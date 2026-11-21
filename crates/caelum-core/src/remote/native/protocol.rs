use crate::application::{Command, Event, Task};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{self, Read, Write};

pub(super) const VERSION: u32 = 1;
pub(super) const LIMIT: usize = 32 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub(super) struct CommandFrame(pub u64, pub Command);

#[derive(Serialize, Deserialize)]
pub(super) enum ServerFrame {
    Ready {
        version: u32,
        root: String,
        repository: String,
    },
    Event(Box<Event>),
    Error(String),
}

impl From<Event> for ServerFrame {
    fn from(event: Event) -> Self {
        Self::Event(Box::new(event))
    }
}

pub(super) fn read_frame<R: Read, T: DeserializeOwned>(r: &mut R) -> io::Result<T> {
    let mut header = [0; 4];
    r.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid native workspace frame size",
        ));
    }
    let mut bytes = vec![0; length];
    r.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

pub(super) fn write_frame<W: Write, T: Serialize>(w: &mut W, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.len() > LIMIT {
        return Err(io::Error::other("Native workspace response exceeds 32 MiB"));
    }
    w.write_all(&(bytes.len() as u32).to_be_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}

/// Explicit allowlist on both ends: no local persistence, desktop actions or Boat management.
/// Until worktrees have separate verified bindings, only the exact VM checkout may be opened.
pub(super) fn admitted(command: &Command, root: &str) -> bool {
    let project = match command {
        Command::Open(path) => path,
        Command::Commands(s)
        | Command::List(s)
        | Command::ListArchived(s)
        | Command::Choices(s)
        | Command::Start(s, _) => &s.project,
        Command::Inbox(k)
        | Command::Generate(k, _)
        | Command::RegenerateTitle(k)
        | Command::Rename(k, _)
        | Command::CheckpointFiles(k, ..)
        | Command::Rewind { key: k, .. }
        | Command::Shells(k)
        | Command::Archive(k, _)
        | Command::DeleteSession(k)
        | Command::Details(k)
        | Command::Fork(k, _)
        | Command::EditBranch(k, ..)
        | Command::Resume(k)
        | Command::History(k, _)
        | Command::CodexControlHistory(k, _)
        | Command::Enqueue(k, ..)
        | Command::RemoveQueued(k, _)
        | Command::Send(k, ..)
        | Command::Cancel(k) => &k.scope.project,
        Command::Approve(p, _) => &p.project_path,
        Command::Answer(p, _) => &p.project_path,
        Command::Task(
            t @ (Task::Shell { .. }
            | Task::Image { .. }
            | Task::EncodeImage { .. }
            | Task::Usage { .. }
            | Task::Stats { .. }
            | Task::Completions { .. }
            | Task::Directory { .. }
            | Task::File { .. }
            | Task::Search { .. }
            | Task::Git { .. }
            | Task::Diff { .. }
            | Task::Discover { .. }
            | Task::Details { .. }
            | Task::ReviewCache { .. }
            | Task::ReviewFiles { .. }
            | Task::Feedback { .. }
            | Task::AttachPullRequest { .. }
            | Task::Merge { .. }
            | Task::SetAutoMerge { .. }
            | Task::UpdateBranch { .. }
            | Task::Worktrees { .. }),
        ) => t.project(),
        _ => return false,
    };
    project == root
}
