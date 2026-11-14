//! Scoped files, checkout and head-pinned review coordination. Renderers never
//! own subprocesses, pagination or response admission.
mod completion;
mod review_cache;

use super::{Preferences, ProviderRuntime};
use crate::{files, git::GitClient, github};
use caelum_protocol::*;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum Task {
    Shell {
        project: String,
        session: String,
        id: String,
        stop: bool,
    },
    Reveal {
        project: String,
        path: String,
    },
    EncodeImage {
        project: String,
        bytes: Vec<u8>,
    },
    Updates,
    Image {
        project: String,
        source: String,
    },
    Usage {
        project: String,
    },
    Stats {
        project: String,
        filter: OpenCodeStatsFilter,
    },
    AttachPullRequest {
        project: String,
        url: String,
    },
    Completions {
        project: String,
        query: String,
    },
    Directory {
        project: String,
        path: String,
        offset: usize,
    },
    File {
        project: String,
        path: String,
    },
    Search {
        project: String,
        query: String,
        mode: ProjectSearchMode,
    },
    Git {
        project: String,
    },
    Diff {
        project: String,
        path: String,
    },
    Worktrees {
        project: String,
        directory: String,
    },
    CreateWorktree {
        project: String,
        branch: String,
        base: String,
        directory: String,
    },
    RemoveWorktree {
        project: String,
        path: String,
    },
    RenameWorktree {
        project: String,
        path: String,
        name: String,
        directory: String,
    },
    Discover {
        project: String,
        branch: String,
    },
    Details {
        project: String,
        url: String,
    },
    ReviewCache {
        project: String,
        url: String,
    },
    ReviewFiles {
        project: String,
        url: String,
        head: String,
        base: String,
        page: u32,
    },
    Feedback {
        project: String,
        url: String,
        head: String,
        kind: GitHubFeedbackKind,
        cursor: Option<String>,
        thread: Option<String>,
    },
    Merge {
        project: String,
        url: String,
        head: String,
        method: GitHubMergeMethod,
    },
    SetAutoMerge {
        project: String,
        url: String,
        head: String,
        /// `Some(method)` enables auto-merge; `None` disables it.
        method: Option<GitHubMergeMethod>,
    },
    UpdateBranch {
        project: String,
        url: String,
        head: String,
    },
    ImportSettings {
        path: String,
    },
}

#[derive(Debug)]
pub enum TaskReply {
    Shell(String),
    Updates(UpdateCheck),
    Image(String),
    Usage(serde_json::Value),
    Stats(OpenCodeStats, Vec<OpenCodeStatsProject>),
    Directory(ProjectDirectoryListing),
    File(ProjectFileContent),
    Search(ProjectSearchResults),
    Git(GitRepositoryStatus, Vec<SessionBranchUpdate>),
    Diff(GitFileDiff),
    Worktrees(Option<GitWorktrees>),
    Worktree(GitWorktree),
    Discovered(Option<GitHubPullRequest>),
    Details(Box<GitHubPullRequestDetails>),
    ReviewCache(Option<Box<github::CachedReview>>),
    ReviewFiles(GitHubPullRequestFiles),
    Feedback(GitHubPullRequestFeedback),
    AutoMerge(GitHubAutoMergeResult),
    Preferences(Preferences),
    Done,
}

impl Task {
    fn mutates_review(&self) -> bool {
        matches!(
            self,
            Self::Merge { .. } | Self::SetAutoMerge { .. } | Self::UpdateBranch { .. }
        )
    }
    pub fn project(&self) -> &str {
        match self {
            Self::Shell { project, .. } => project,
            Self::Completions { project, .. }
            | Self::Image { project, .. }
            | Self::EncodeImage { project, .. }
            | Self::Reveal { project, .. } => project,
            Self::Usage { project }
            | Self::Stats { project, .. }
            | Self::AttachPullRequest { project, .. } => project,
            Self::Directory { project, .. }
            | Self::File { project, .. }
            | Self::Search { project, .. }
            | Self::Git { project }
            | Self::Diff { project, .. }
            | Self::Worktrees { project, .. }
            | Self::CreateWorktree { project, .. }
            | Self::RemoveWorktree { project, .. }
            | Self::RenameWorktree { project, .. }
            | Self::Discover { project, .. }
            | Self::Details { project, .. }
            | Self::ReviewCache { project, .. }
            | Self::ReviewFiles { project, .. }
            | Self::Feedback { project, .. }
            | Self::Merge { project, .. }
            | Self::SetAutoMerge { project, .. }
            | Self::UpdateBranch { project, .. } => project,
            Self::ImportSettings { .. } | Self::Updates => "",
        }
    }
    pub fn review(&self) -> Option<(&str, Option<&str>)> {
        match self {
            Self::Details { url, .. } | Self::ReviewCache { url, .. } => Some((url, None)),
            Self::ReviewFiles { url, head, .. }
            | Self::Feedback { url, head, .. }
            | Self::Merge { url, head, .. }
            | Self::SetAutoMerge { url, head, .. }
            | Self::UpdateBranch { url, head, .. } => Some((url, Some(head))),
            _ => None,
        }
    }
    pub(super) fn slot(&self) -> String {
        // JSON tuple encoding keeps delimiters inside paths unambiguous.
        let part = match self {
            Self::Shell {
                session, id, stop, ..
            } => format!("shell:{session}:{id}:{stop}"),
            Self::Updates => "updates".into(),
            Self::Reveal { path, .. } => format!("reveal:{path}"),
            Self::EncodeImage { .. } => "encode-image".into(),
            Self::Image { source, .. } => format!("image:{source}"),
            Self::Usage { .. } => "usage".into(),
            Self::Stats { .. } => "stats".into(),
            Self::AttachPullRequest { url, .. } => format!("attach-pr:{url}"),
            Self::Completions { .. } => "completions".into(),
            Self::Directory { path, .. } => format!("directory:{path}"),
            Self::File { path, .. } => format!("file:{path}"),
            Self::Search { .. } => "search".into(),
            Self::Git { .. } => "git".into(),
            Self::Diff { path, .. } => format!("diff:{path}"),
            Self::Worktrees { .. } => "worktrees".into(),
            Self::CreateWorktree { .. }
            | Self::RemoveWorktree { .. }
            | Self::RenameWorktree { .. } => "worktree-mutation".into(),
            Self::Discover { branch, .. } => format!("discover:{branch}"),
            Self::Details { url, .. } => format!("details:{url}"),
            Self::ReviewCache { url, .. } => format!("review-cache:{url}"),
            Self::ReviewFiles { url, .. } => format!("files:{url}"),
            Self::Feedback {
                url, kind, thread, ..
            } => format!("feedback:{url}:{kind:?}:{thread:?}"),
            Self::Merge { url, .. }
            | Self::SetAutoMerge { url, .. }
            | Self::UpdateBranch { url, .. } => format!("mutation:{url}"),
            Self::ImportSettings { .. } => "settings".into(),
        };
        serde_json::json!([self.project(), part]).to_string()
    }

    pub(super) fn execute(&self, runtime: &ProviderRuntime) -> Result<TaskReply, String> {
        let worktrees = &runtime.worktrees;
        let git = GitClient::new();
        macro_rules! reply {
            ($call:expr, $variant:ident) => {
                $call.map(TaskReply::$variant).map_err(|e| e.to_string())
            };
        }
        match self {
            Self::Shell {
                project,
                session,
                id,
                stop,
            } => reply!(
                runtime.opencode.shell_action(project, session, id, *stop),
                Shell
            ),
            Self::Reveal { project, path } => {
                let project = crate::projects::Project::open(project).map_err(|e| e.to_string())?;
                let path = project
                    .resolve_scoped_path(path)
                    .map_err(|e| e.to_string())?;
                let status = std::process::Command::new(if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                })
                .args(if cfg!(target_os = "macos") {
                    vec!["-R".as_ref(), path.as_os_str()]
                } else {
                    vec![path.as_os_str()]
                })
                .status()
                .map_err(|e| e.to_string())?;
                if !status.success() {
                    return Err("Unable to reveal this path".into());
                }
                Ok(TaskReply::Done)
            }
            Self::EncodeImage { bytes, .. } => {
                crate::images::data_url(bytes.as_slice()).map(TaskReply::Image)
            }
            Self::Image { project, source } => {
                let data = if source.starts_with("data:image/") {
                    source.clone()
                } else if let Some(hash) = source.strip_prefix("opencode-media:") {
                    runtime
                        .opencode
                        .read_image(project, hash)
                        .map_err(|e| e.to_string())?
                } else if source.starts_with("https://") || source.starts_with("http://") {
                    let response = reqwest::blocking::Client::builder()
                        .timeout(std::time::Duration::from_secs(20))
                        .build()
                        .map_err(|e| e.to_string())?
                        .get(source)
                        .send()
                        .and_then(|r| r.error_for_status())
                        .map_err(|e| e.to_string())?;
                    crate::images::data_url(response)?
                } else {
                    files::read_image(project, source)?
                };
                Ok(TaskReply::Image(data))
            }
            Self::Updates => reply!(
                crate::updates::check_for_updates(env!("CARGO_PKG_VERSION")),
                Updates
            ),
            Self::Usage { project } => reply!(runtime.opencode.usage_limits(project), Usage),
            Self::Stats { project, filter } => Ok(TaskReply::Stats(
                runtime
                    .opencode
                    .session_stats(project, filter)
                    .map_err(|e| e.to_string())?,
                runtime
                    .opencode
                    .stats_projects(project)
                    .map_err(|e| e.to_string())?,
            )),
            Self::AttachPullRequest { project, url } => {
                reply!(
                    github::pull_request_details(project, url).map(Box::new),
                    Details
                )
            }
            Self::Completions { project, query } => reply!(
                files::search_project(project, query, ProjectSearchMode::Files, false),
                Search
            ),
            Self::Directory {
                project,
                path,
                offset,
            } => reply!(
                files::list_directory(project, path, *offset, 200),
                Directory
            ),
            Self::File { project, path } => reply!(files::read_text_file(project, path), File),
            Self::Search {
                project,
                query,
                mode,
            } => reply!(files::search_project(project, query, *mode, false), Search),
            Self::Git { project } => {
                let status = git.repository_status(project).map_err(|e| e.to_string())?;
                let branches = runtime
                    .branches
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .refresh_checkout(&status);
                Ok(TaskReply::Git(status, branches))
            }
            Self::Diff { project, path } => reply!(git.file_diff(project, path), Diff),
            Self::Worktrees { project, directory } => {
                reply!(worktrees.list(project, directory), Worktrees)
            }
            Self::CreateWorktree {
                project,
                branch,
                base,
                directory,
            } => reply!(worktrees.create(project, branch, base, directory), Worktree),
            Self::RemoveWorktree { project, path } => {
                runtime
                    .codex
                    .ensure_checkout_idle(std::path::Path::new(path))
                    .map_err(|e| e.to_string())?;
                runtime
                    .opencode
                    .ensure_checkout_idle(std::path::Path::new(path))
                    .map_err(|e| e.to_string())?;
                let removed = worktrees.remove(project, path).map_err(|e| e.to_string())?;
                runtime.codex.release_checkout(std::path::Path::new(path));
                runtime
                    .opencode
                    .release_checkout(std::path::Path::new(path));
                Ok(TaskReply::Worktree(removed))
            }
            Self::RenameWorktree {
                project,
                path,
                name,
                ..
            } => reply!(worktrees.rename(project, path, name), Worktree),
            Self::Discover { project, branch } => {
                reply!(github::open_pull_request(project, branch), Discovered)
            }
            Self::Details { project, url } => review_cache::details(runtime, project, url)
                .map(Box::new)
                .map(TaskReply::Details),
            Self::ReviewCache { url, .. } => Ok(TaskReply::ReviewCache(
                review_cache::cache(runtime)
                    .and_then(|cache| cache.load(url))
                    .map(Box::new),
            )),
            Self::ReviewFiles {
                project,
                url,
                head,
                base,
                page,
            } => review_cache::files(runtime, project, url, head, base, *page)
                .map(TaskReply::ReviewFiles),
            Self::Feedback {
                project,
                url,
                head,
                kind,
                cursor,
                thread,
            } => reply!(
                github::pull_request_feedback(
                    project,
                    url,
                    head,
                    *kind,
                    cursor.as_deref(),
                    thread.as_deref()
                ),
                Feedback
            ),
            Self::Merge {
                project,
                url,
                head,
                method,
            } => github::merge_pull_request(project, url, *method, head)
                .map(|()| TaskReply::Done)
                .map_err(|e| e.to_string()),
            Self::SetAutoMerge {
                project,
                url,
                head,
                method,
            } => reply!(
                github::set_pull_request_auto_merge(project, url, head, *method),
                AutoMerge
            ),
            Self::UpdateBranch { project, url, head } => {
                github::update_pull_request_branch(project, url, head)
                    .map(|()| TaskReply::Done)
                    .map_err(|e| e.to_string())
            }
            Self::ImportSettings { path } => Preferences::import_tauri(
                &std::fs::read_to_string(path).map_err(|e| e.to_string())?,
            )
            .map(TaskReply::Preferences),
        }
    }
}

#[derive(Default)]
pub struct Checkout {
    pub shell_output: BTreeMap<(String, String), String>,
    pub usage: Option<serde_json::Value>,
    pub stats: Option<OpenCodeStats>,
    pub stats_projects: Vec<OpenCodeStatsProject>,
    pub stats_filter: String,
    pub completions: Option<ProjectSearchResults>,
    pub completion_query: String,
    pub directories: BTreeMap<String, ProjectDirectoryListing>,
    pub files: BTreeMap<String, ProjectFileContent>,
    pub tabs: Vec<String>,
    pub selected_file: Option<String>,
    pub search: Option<ProjectSearchResults>,
    pub search_query: String,
    pub git: Option<GitRepositoryStatus>,
    pub diffs: BTreeMap<String, GitFileDiff>,
    pub worktrees: Option<GitWorktrees>,
    worktrees_revision: u64,
    pub pull_requests: Vec<GitHubPullRequest>,
    pub error: Option<String>,
}

#[derive(Default, PartialEq, Eq)]
enum ReviewSource {
    #[default]
    GitHub,
    Cache,
}

#[derive(Default)]
pub struct Review {
    generation: u64,
    source: ReviewSource,
    pub details: Option<GitHubPullRequestDetails>,
    pub files: Vec<GitHubPullRequestFile>,
    pub files_loaded: bool,
    pub base: Option<String>,
    pub next_page: Option<u32>,
    pub discussion: Vec<GitHubReviewComment>,
    pub reviews: Vec<GitHubReviewComment>,
    pub threads: Vec<GitHubReviewThread>,
    pub cursors: BTreeMap<String, Option<String>>,
    pub selected_comments: HashSet<String>,
    pub error: Option<String>,
    pub notice: Option<String>,
}

impl Review {
    pub fn cached(&self) -> bool {
        self.source == ReviewSource::Cache
    }
    pub fn expected_base(&self) -> Option<&str> {
        self.details.as_ref().map(|d| d.base_ref_oid.as_str())
    }
    pub fn head(&self) -> Option<&str> {
        self.details.as_ref().map(|d| d.head_ref_oid.as_str())
    }
    pub fn address_prompt(&self) -> Option<String> {
        let details = self.details.as_ref()?;
        let mut comments = Vec::new();
        for c in self.discussion.iter().chain(&self.reviews) {
            if self.selected_comments.contains(&c.id) {
                comments.push(format!("{}\n{}", c.url, c.body));
            }
        }
        for thread in &self.threads {
            for c in &thread.comments {
                if self.selected_comments.contains(&c.id) {
                    comments.push(format!(
                        "{}:{} ({}, {}{})\n{}\n{}\n{}",
                        thread.path,
                        thread.line.or(thread.original_line).unwrap_or(0),
                        thread.diff_side,
                        if thread.is_resolved {
                            "resolved"
                        } else {
                            "open"
                        },
                        if thread.is_outdated { ", outdated" } else { "" },
                        c.url,
                        c.diff_hunk.as_deref().unwrap_or_default(),
                        c.body
                    ));
                }
            }
        }
        if comments.is_empty() {
            return None;
        }
        Some(format!("Address these selected review comments for {} at head {}. Verify each against the current checkout before changing code.\n\n{}", details.pull_request.url, details.head_ref_oid, comments.join("\n\n---\n\n")))
    }
}

#[derive(Default)]
pub struct Services {
    pub images: BTreeMap<(String, String), String>,
    pub revision: u64,
    errors: HashMap<String, String>,
    pub update: Option<UpdateCheck>,
    pub checkouts: BTreeMap<String, Checkout>,
    pub reviews: BTreeMap<(String, String), Review>,
    latest: HashMap<String, u64>,
    loading: HashSet<String>,
    discovered: HashMap<(String, String), Option<String>>,
    merged_pull_requests: HashSet<String>,
    review_owners: HashMap<String, (String, String)>,
    generations: HashMap<u64, u64>,
}

impl Services {
    pub(super) fn pull_request_branches<'a>(
        &'a self,
        project: &'a str,
    ) -> impl Iterator<Item = String> + 'a {
        self.discovered
            .keys()
            .filter(move |(path, _)| path == project)
            .map(|(_, branch)| branch.clone())
    }

    /// A listing read from any checkout describes the same repository. Prefer
    /// the newest successful request, even if replies finish out of order.
    pub fn worktrees(&self, project: &str) -> Option<&GitWorktrees> {
        self.checkouts
            .values()
            .filter_map(|checkout| {
                let listing = checkout.worktrees.as_ref()?;
                (listing.project.path == project
                    || listing
                        .worktrees
                        .iter()
                        .any(|tree| tree.project.path == project))
                .then_some((checkout.worktrees_revision, listing))
            })
            .max_by_key(|(revision, _)| *revision)
            .map(|(_, listing)| listing)
    }

    pub fn refresh_review(&mut self, project: &str, url: &str) {
        self.revision += 1;
        let key = (project.into(), url.into());
        let review = self.reviews.entry(key.clone()).or_default();
        review.generation += 1;
        review.files.clear();
        review.base = None;
        review.files_loaded = false;
        review.next_page = None;
        review.cursors.clear();
        review.discussion.clear();
        review.reviews.clear();
        review.threads.clear();
        review.selected_comments.clear();
        let mutation = Task::UpdateBranch {
            project: project.into(),
            url: url.into(),
            head: String::new(),
        }
        .slot();
        for (slot, owner) in &self.review_owners {
            if *owner == key && *slot != mutation {
                self.latest.remove(slot);
                self.loading.remove(slot);
            }
        }
    }
    pub(super) fn current(&self, id: u64, task: &Task) -> bool {
        if self.latest.get(&task.slot()) != Some(&id) {
            return false;
        }
        if task.mutates_review() {
            return true;
        }
        if let Some((url, head)) = task.review() {
            let Some(review) = self.reviews.get(&(task.project().into(), url.into())) else {
                return false;
            };
            if self.generations.get(&id) != Some(&review.generation)
                || head.is_some_and(|head| review.head() != Some(head))
                || matches!(task, Task::ReviewFiles { base, .. } if review.expected_base() != Some(base))
            {
                return false;
            }
        }
        true
    }
    pub fn busy(&self, task: &Task) -> bool {
        self.loading.contains(&task.slot())
    }
    pub fn error(&self, task: &Task) -> Option<&str> {
        self.errors.get(&task.slot()).map(String::as_str)
    }
    pub(super) fn start(&mut self, task: &Task, id: u64) {
        self.errors.remove(&task.slot());
        if let Task::SetAutoMerge { project, url, .. } = task {
            self.invalidate_details(project, url);
            self.reviews
                .entry((project.clone(), url.clone()))
                .or_default()
                .notice = None;
        }
        if let Task::Stats { filter, .. } = task {
            let checkout = self.checkouts.entry(task.project().into()).or_default();
            let key = serde_json::to_string(filter).unwrap_or_default();
            if checkout.stats_filter != key {
                checkout.stats = None;
                checkout.stats_filter = key;
            }
        }
        self.latest.insert(task.slot(), id);
        self.loading.insert(task.slot());
        self.checkouts
            .entry(task.project().into())
            .or_default()
            .error = None;
        if let Some((url, _)) = task.review() {
            let key = (task.project().into(), url.into());
            let review = self.reviews.entry(key.clone()).or_default();
            review.error = None;
            self.generations.insert(id, review.generation);
            self.review_owners.insert(task.slot(), key);
        }
    }

    fn invalidate_details(&mut self, project: &str, url: &str) {
        let slot = Task::Details {
            project: project.into(),
            url: url.into(),
        }
        .slot();
        self.latest.remove(&slot);
        self.loading.remove(&slot);
    }
}
