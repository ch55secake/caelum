//! Session discovery order is independent of navigation and streamed token arrival.
use super::{Session, Workspace};
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

    /// Return stable local identities, including linked checkouts of each project.
    /// A temporary project filter overrides the saved scope without changing it.
    pub fn session_feed(&self, project: Option<&str>, query: &str, archived: bool) -> Vec<usize> {
        let project = project.or_else(|| {
            (self.preferences.session_scope == SessionScope::CurrentProject)
                .then_some(self.active_project.as_deref())
                .flatten()
        });
        if project.is_none() && self.preferences.session_scope == SessionScope::CurrentProject {
            return Vec::new();
        }
        let query = query.trim().to_lowercase();
        let mut indices: Vec<_> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                !session.forgotten
                    && session.archived == archived
                    && self.project_is_open(&session.scope.project)
                    && project.is_none_or(|p| {
                        self.group_path(&session.scope.project) == self.group_path(p)
                    })
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
