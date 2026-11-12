//! Session discovery order is independent of navigation and streamed token arrival.
use super::{Session, Task, Workspace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionScope {
    #[default]
    AllProjects,
    CurrentProject,
}

impl Workspace {
    pub fn discover_open_sessions(&mut self) {
        let scopes: Vec<_> = self
            .projects
            .iter()
            .flat_map(|project| {
                super::Provider::ALL.map(|provider| super::Scope {
                    project: project.path.clone(),
                    provider,
                })
            })
            .collect();
        for scope in scopes {
            self.discover_sessions(scope);
        }
    }

    /// Shared scope for the session feed and its pull requests, including linked checkouts.
    pub fn feed_includes_project(&self, path: &str, project: Option<&str>) -> bool {
        let project = project.or_else(|| {
            (self.preferences.session_scope == SessionScope::CurrentProject)
                .then_some(self.active_project.as_deref())
                .flatten()
        });
        self.project_is_open(path)
            && match project {
                Some(project) => self.group_path(path) == self.group_path(project),
                None => self.preferences.session_scope == SessionScope::AllProjects,
            }
    }

    /// Refresh even when Git's branch is unchanged, without changing session ownership.
    pub fn refresh_feed_pull_requests(&mut self, project: Option<&str>) {
        let paths: Vec<_> = self
            .projects
            .iter()
            .filter(|p| self.feed_includes_project(&p.path, project))
            .map(|p| p.path.clone())
            .collect();
        for path in paths {
            let mut branches: std::collections::BTreeSet<_> =
                self.services.pull_request_branches(&path).collect();
            if let Some(branch) = self
                .services
                .checkouts
                .get(&path)
                .and_then(|c| c.git.as_ref())
                .and_then(|g| g.branch.clone())
            {
                branches.insert(branch);
            }
            for session in self
                .sessions
                .iter()
                .filter(|s| !s.forgotten && s.scope.project == path)
            {
                if let Some(branch) = self.session_branch(session) {
                    branches.insert(branch.to_owned());
                }
                if let Some(details) = &session.details {
                    branches.extend(details.branches.iter().cloned());
                }
            }
            self.task(Task::Git {
                project: path.clone(),
            });
            for branch in branches {
                self.task(Task::Discover {
                    project: path.clone(),
                    branch,
                });
            }
        }
    }

    /// Return stable local identities, including linked checkouts of each project.
    /// A temporary project filter overrides the saved scope without changing it.
    pub fn session_feed(&self, project: Option<&str>, query: &str, archived: bool) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        let mut indices: Vec<_> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                !session.forgotten
                    && session.archived == archived
                    && self.feed_includes_project(&session.scope.project, project)
                    && (query.is_empty()
                        || format!(
                            "{} {} {} {} {} {}",
                            session.title,
                            session.scope.provider.as_str(),
                            session.scope.project,
                            self.projects
                                .iter()
                                .find(|p| p.path == self.group_path(&session.scope.project))
                                .map_or("", |p| &p.name),
                            self.session_branch(session).unwrap_or_default(),
                            self.session_worktree(session).unwrap_or_default(),
                        )
                        .to_lowercase()
                        .contains(&query))
            })
            .map(|(index, _)| index)
            .collect();
        indices
            .sort_by_key(|&index| (std::cmp::Reverse(self.sessions[index].last_activity), index));
        indices
    }

    pub fn session_branch<'a>(&'a self, session: &'a Session) -> Option<&'a str> {
        session
            .details
            .as_ref()
            .and_then(|d| d.branch.as_deref())
            .or_else(|| {
                self.services
                    .checkouts
                    .get(&session.scope.project)?
                    .git
                    .as_ref()?
                    .branch
                    .as_deref()
            })
            .or_else(|| {
                self.services
                    .worktrees(&session.scope.project)?
                    .worktrees
                    .iter()
                    .find(|tree| tree.project.path == session.scope.project)?
                    .branch
                    .as_deref()
            })
    }

    pub fn session_worktree<'a>(&'a self, session: &'a Session) -> Option<&'a str> {
        let path = &session.scope.project;
        if self.group_path(path) == path {
            return None;
        }
        self.services
            .worktrees(path)
            .and_then(|trees| trees.worktrees.iter().find(|t| t.project.path == *path))
            .and_then(|tree| tree.label.as_deref())
            .or_else(|| path.rsplit('/').next())
    }
}

#[cfg(test)]
mod tests;
