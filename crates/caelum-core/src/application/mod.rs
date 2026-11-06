//! Framework-independent application boundary. Blocking provider work belongs to
//! `Runtime`; `Workspace` owns scoped requests and conversation reconciliation.

mod conversation;
mod runtime;
mod workspace;

pub use conversation::{Conversation, Entry, EntryKind};
pub use runtime::{Command, Event, ProviderRuntime, Reply, Runtime};
pub use workspace::{Session, Workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Scope {
    pub project: String,
    pub provider: Provider,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionKey {
    pub scope: Scope,
    pub id: String,
}

#[derive(Debug, Clone, Default)]
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
