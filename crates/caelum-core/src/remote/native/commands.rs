use crate::application::{Command, Task};

/// Commands owned by this exact VM checkout.
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
