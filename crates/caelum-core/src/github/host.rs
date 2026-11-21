use super::*;
use crate::{
    application::{Task, TaskReply},
    remote::native::Access,
};

/// The same head-pinned API requests run `gh` in the checkout that owns them.
pub(super) trait Gh {
    fn program(&self) -> &Path;
    fn project(&self, path: &Path) -> Result<Project, ProjectError> {
        Project::open(path)
    }
    fn output(&self, cmd: Command, timeout: Duration) -> Result<Vec<u8>, GitHubError> {
        checked_output(cmd, timeout).map(|o| o.stdout)
    }
}
impl Gh for Path {
    fn program(&self) -> &Path {
        self
    }
}
impl Gh for PathBuf {
    fn program(&self) -> &Path {
        self
    }
}
impl Gh for Access {
    fn program(&self) -> &Path {
        Path::new("gh")
    }
    fn project(&self, path: &Path) -> Result<Project, ProjectError> {
        Project::for_provider(path, Some(self))
    }
    fn output(&self, cmd: Command, timeout: Duration) -> Result<Vec<u8>, GitHubError> {
        self.gh(cmd.get_args(), timeout)
            .map_err(GitHubError::Command)
    }
}

impl Access {
    pub(crate) fn github_task(&self, t: &Task) -> Result<TaskReply, String> {
        let root = Path::new(&self.root);
        let read = Duration::from_secs(10);
        let write = Duration::from_secs(30);
        match t {
            Task::Discover { branch, .. } => {
                query_pull_request(self, root, branch, read).map(TaskReply::Discovered)
            }
            Task::Details { url, .. } | Task::AttachPullRequest { url, .. } => {
                let observed = cache::timestamp();
                query_details(self, root, url, read)
                    .inspect(|details| {
                        if let Some(m) = &self.metadata {
                            m.review(|c| c.save_details(details, observed));
                        }
                    })
                    .map(Box::new)
                    .map(TaskReply::Details)
            }
            Task::ReviewCache { url, .. } => Ok(TaskReply::ReviewCache(
                self.metadata
                    .as_ref()
                    .and_then(|m| m.review(|c| c.load(url)))
                    .flatten()
                    .map(Box::new),
            )),
            Task::ReviewFiles {
                url,
                head,
                base,
                page,
                ..
            } => {
                if let Some(files) = self
                    .metadata
                    .as_ref()
                    .and_then(|m| m.review(|c| c.page(url, head, base, *page)))
                    .flatten()
                {
                    return Ok(TaskReply::ReviewFiles(files));
                }
                review::files(self, root, url, head, base, *page)
                    .inspect(|files| {
                        if let Some(m) = &self.metadata {
                            m.review(|c| c.save_page(url, head, *page, files));
                        }
                    })
                    .map(TaskReply::ReviewFiles)
            }
            Task::Feedback {
                url,
                head,
                kind,
                cursor,
                thread,
                ..
            } => review::feedback(
                self,
                root,
                url,
                head,
                *kind,
                cursor.as_deref(),
                thread.as_deref(),
            )
            .map(TaskReply::Feedback),
            Task::Merge {
                url, head, method, ..
            } => merge_request(self, root, url, *method, head, write).map(|()| TaskReply::Done),
            Task::UpdateBranch { url, head, .. } => {
                update_branch_request(self, root, url, head, write).map(|()| TaskReply::Done)
            }
            Task::SetAutoMerge {
                url, head, method, ..
            } => {
                auto_merge::set_auto_merge(self, root, url, head, *method).map(TaskReply::AutoMerge)
            }
            _ => return Err("Unsupported VM checkout operation.".into()),
        }
        .map_err(|e| e.to_string())
    }
}
