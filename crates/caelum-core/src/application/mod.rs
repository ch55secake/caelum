//! Framework-independent application boundary. Blocking provider work belongs to
//! `Runtime`; `Workspace` owns scoped requests and conversation reconciliation.

mod composer;
mod conversation;
pub mod forms;
mod persistence;
mod runtime;
mod services;
mod workspace;

pub use composer::{split_context, Attachment, QueuedPrompt};
pub use conversation::{Conversation, Entry, EntryKind};
pub use persistence::{Preferences, SavedSession, Snapshot, Store};
pub use runtime::{Command, Event, ProviderRuntime, Reply, Runtime};
pub use services::{Checkout, Review, Services, Task, TaskReply};
pub use workspace::{Session, Workspace};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Codex,
    OpenCode,
}

impl Provider {
    pub const ALL: [Self; 2] = [Self::Codex, Self::OpenCode];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scope {
    pub project: String,
    pub provider: Provider,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionKey {
    pub scope: Scope,
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub agent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub key: SessionKey,
    pub title: String,
    pub working: bool,
    pub options: Options,
}

#[cfg(test)]
mod tests;
