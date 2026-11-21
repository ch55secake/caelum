use super::{Machine, MachineId};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Instant};

/// GitHub origin and published branch from a local checkout; no local files are transferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub project: String,
    pub account: String,
    pub repository: String,
    pub base: String,
    /// The published tip captured from the session's exact worktree for a handoff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_tip: Option<String>,
    pub environment: Option<String>,
}

impl Source {
    /// Standalone creates use a new name. Only a confirmed handoff can reuse the base name.
    pub(crate) fn valid_plan_branch(&self, branch: &str) -> bool {
        valid_branch(&self.base)
            && valid_branch(branch)
            && self
                .published_tip
                .as_deref()
                .map_or(self.base != branch, valid_tip)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Size {
    Small,
    #[default]
    Standard,
    Large,
    Xlarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub token: String,
    pub branch: String,
    pub size: Size,
    #[serde(default, rename = "minutes")]
    pub lifetime: super::Lifetime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Create(Plan),
    Prepare {
        machine: Machine,
        new_branch: Option<String>,
    },
    Ready {
        machine: Machine,
        branch: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkout {
    pub source: Source,
    pub phase: Phase,
    /// Canonical Git worktree on this VM. Older snapshots resolve it on refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default)]
    pub thread: super::Thread,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_threads: Vec<super::Thread>,
    #[serde(default)]
    pub pinned_stats: bool,
}

impl Checkout {
    pub(crate) fn open_thread(&mut self, mut thread: super::Thread) {
        thread.record_activity();
        let prior = std::mem::replace(&mut self.thread, thread);
        if !prior.is_placeholder() {
            self.other_threads.push(prior);
        }
    }

    pub(crate) fn threads(&self) -> impl Iterator<Item = &super::Thread> {
        std::iter::once(&self.thread).chain(&self.other_threads)
    }

    pub fn machine(&self) -> Option<&Machine> {
        match &self.phase {
            Phase::Create(_) => None,
            Phase::Prepare { machine, .. } | Phase::Ready { machine, .. } => Some(machine),
        }
    }
    pub fn title(&self) -> &str {
        self.machine().map_or("VM setup", |m| &m.name)
    }
    pub fn branch(&self) -> Option<&str> {
        match &self.phase {
            Phase::Create(p) => Some(&p.branch),
            Phase::Prepare { new_branch, .. } => new_branch.as_deref(),
            Phase::Ready { branch, .. } => Some(branch),
        }
    }
    pub fn target(&self) -> Option<Target> {
        Some(Target {
            machine: self.machine()?.key.clone(),
            repository: self.source.repository.clone(),
            root: self.root.clone(),
            required_base: self
                .thread
                .continued
                .as_ref()
                .filter(|_| self.thread.conversation.is_none())
                .and_then(|c| c.base.clone()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub machine: MachineId,
    pub repository: String,
    pub root: Option<String>,
    pub required_base: Option<PublishedBase>,
}

pub(super) fn valid_root(path: &str) -> bool {
    path.len() <= 4096
        && !path.chars().any(char::is_control)
        && path
            .strip_prefix('/')
            .is_some_and(|p| p.split('/').all(|part| !matches!(part, "" | "." | "..")))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedBase {
    pub branch: String,
    pub tip: String,
}

impl PublishedBase {
    pub(super) fn valid(&self) -> bool {
        valid_branch(&self.branch) && valid_tip(&self.tip)
    }
}

fn valid_tip(tip: &str) -> bool {
    matches!(tip.len(), 40 | 64) && tip.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VmLink {
    pub checkout: String,
    pub thread: String,
}

impl VmLink {
    pub fn resolve<'a>(&self, saved: &'a Saved) -> Option<(&'a Checkout, &'a super::Thread)> {
        let c = saved.checkouts.get(&self.checkout)?;
        let t = c
            .threads()
            .find(|t| t.editor_key(&self.checkout) == self.thread)?;
        Some((c, t))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub checkouts: BTreeMap<String, Checkout>,
    pub selected: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expanded: BTreeMap<String, bool>,
}

#[derive(Debug)]
pub enum Job {
    Busy(u64),
    Deleting(u64),
    Wait(Instant),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum Picker {
    Loading { request: u64, project: String },
    Ready(Source),
    Failed { project: String, error: String },
}

pub fn valid_branch(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value != "HEAD"
        && !value.starts_with('-')
        && !value.contains("..")
        && !value.ends_with('.')
        && value
            .split('/')
            .all(|p| !p.is_empty() && !p.starts_with('.') && !p.ends_with(".lock"))
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
}

pub(super) fn repo_name(slug: &str) -> Result<&str, String> {
    let (owner, repo) = slug
        .split_once('/')
        .ok_or("Expected a GitHub owner/repository")?;
    for part in [owner, repo] {
        if part.is_empty()
            || part.starts_with('.')
            || !part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err("Invalid GitHub repository name".into());
        }
    }
    Ok(repo)
}

pub(crate) fn github_slug(url: &str) -> Option<String> {
    let path = if let Some(path) = url.trim().strip_prefix("git@github.com:") {
        path.to_owned()
    } else {
        let url = reqwest::Url::parse(url.trim()).ok()?;
        if url.host_str()? != "github.com" || !matches!(url.scheme(), "ssh" | "https") {
            return None;
        }
        url.path().trim_start_matches('/').to_owned()
    };
    let path = path.trim_end_matches('/');
    let slug = path.strip_suffix(".git").unwrap_or(path);
    repo_name(slug).ok()?;
    Some(slug.to_owned())
}
