//! Dispatch checkout reads on the connection that verified the root. Only
//! provider API operations may continue to the shared provider adapters.
mod files;
mod git;
use super::{commands, Access};
use crate::application::{Command, Reply, Task, TaskReply};
use caelum_protocol::ProjectSearchMode;

impl Access {
    pub(crate) fn checkout_command(
        &self,
        cmd: &Command,
        branches: &std::sync::Mutex<crate::session_branches::SessionBranches>,
    ) -> Option<Result<Reply, String>> {
        if !self.live.load(std::sync::atomic::Ordering::Acquire)
            || !commands::admitted(cmd, &self.root)
        {
            return Some(Err("Action outside the current VM connection.".into()));
        }
        let task = match cmd {
            Command::Open(p) => {
                return Some(
                    crate::projects::Project::for_provider(p, Some(self))
                        .map(|p| Reply::Project(p.info()))
                        .map_err(|e| e.to_string()),
                )
            }
            Command::Task(t) => t,
            _ => return None,
        };
        let reply = match task {
            Task::File { path, .. } => self.io.block_on(self.file(path)).map(TaskReply::File),
            Task::Directory { path, offset, .. } => self
                .io
                .block_on(self.directory(path, *offset))
                .map(TaskReply::Directory),
            Task::Image { source, .. }
                if source.starts_with("opencode-media:") || source.starts_with("data:image/") =>
            {
                return None
            }
            Task::Image { source, .. } => {
                self.io.block_on(self.image(source)).map(TaskReply::Image)
            }
            Task::Search { query, mode, .. } => self
                .io
                .block_on(self.search(query, *mode))
                .map(TaskReply::Search),
            Task::Completions { query, .. } => self
                .io
                .block_on(self.search(query, ProjectSearchMode::Files))
                .map(TaskReply::Search),
            Task::Git { .. } => self.io.block_on(self.status()).map(|s| {
                let updates = branches
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .refresh_checkout(&s);
                TaskReply::Git(s, updates)
            }),
            Task::Diff { path, .. } => self.io.block_on(self.diff(path)).map(TaskReply::Diff),
            Task::Worktrees { .. } => Ok(TaskReply::Worktrees(None)),
            Task::Usage { .. }
            | Task::Stats { .. }
            | Task::Shell { .. }
            | Task::EncodeImage { .. } => return None,
            _ => self.github_task(task),
        };
        Some(reply.map(Reply::Task))
    }
}
