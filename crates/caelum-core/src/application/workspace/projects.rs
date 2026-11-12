use super::{Pending, Workspace};
use crate::application::{Command, Provider, Scope, Task};

impl Workspace {
    pub fn open_project(&mut self, path: String) {
        self.opening = true;
        self.error = None;
        self.latest_open = self.request(Command::Open(path), Pending::Open(None, self.navigation));
    }

    pub fn launch_project(&mut self, path: String, provider: Provider) {
        self.opening = true;
        self.error = None;
        self.latest_open = self.request(
            Command::Open(path),
            Pending::Open(Some(provider), self.navigation),
        );
    }

    pub fn select_project(&mut self, path: &str) {
        if !self.projects.iter().any(|project| project.path == path) {
            return;
        }
        self.active_project = Some(path.into());
        if let Some(index) = self
            .remembered
            .get(path)
            .copied()
            .filter(|i| !self.sessions[*i].archived && !self.sessions[*i].forgotten)
        {
            self.select(index);
        } else {
            self.new_session(Scope {
                project: path.into(),
                provider: self.preferences.default_provider,
            });
        }
    }

    pub(super) fn select_open_session_or_new(&mut self, preferred_project: &str, fallback: Scope) {
        let preferred_group = self.group_path(preferred_project).to_owned();
        let selected = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                !session.archived
                    && !session.forgotten
                    && !session.archiving
                    && !session.deleting
                    && self.project_is_open(&session.scope.project)
            })
            .max_by_key(|(index, session)| {
                (
                    session.has_context(),
                    self.group_path(&session.scope.project) == preferred_group,
                    session.scope.project == preferred_project,
                    *index,
                )
            })
            .map(|(index, _)| index);
        if let Some(index) = selected {
            self.select(index);
        } else {
            self.new_session(fallback);
        }
    }

    /// Worktrees created by Caelum, scoped to their original project. Git's full
    /// catalogue is still used for branch choices and checkout availability.
    pub fn managed_worktrees<'a>(
        &'a self,
        project: &'a str,
    ) -> impl Iterator<Item = &'a caelum_protocol::GitWorktree> {
        self.services
            .worktrees(project)
            .into_iter()
            .flat_map(|listing| &listing.worktrees)
            .filter(move |tree| {
                !tree.main
                    && self.managed_worktrees.contains(&tree.path)
                    && self.group_path(&tree.project.path) == project
            })
    }

    /// Switch to this exact checkout, including the main checkout. A project
    /// group's remembered session may belong to a different linked worktree.
    pub fn select_checkout(&mut self, path: &str) -> bool {
        if !self.projects.iter().any(|project| project.path == path)
            || self
                .services
                .worktrees(self.group_path(path))
                .is_some_and(|listing| {
                    !listing
                        .worktrees
                        .iter()
                        .any(|tree| tree.project.path == path && tree.available)
                })
        {
            return false;
        }
        let usable = |i: usize| {
            let session = &self.sessions[i];
            session.scope.project == path && !session.archived && !session.forgotten
        };
        let index = self
            .remembered_checkouts
            .get(path)
            .or(self.remembered.get(path))
            .copied()
            .filter(|&i| usable(i))
            .or_else(|| (0..self.sessions.len()).rev().find(|&i| usable(i)));
        if let Some(index) = index {
            self.select(index);
        } else {
            self.new_session(Scope {
                project: path.into(),
                provider: self.preferences.default_provider,
            });
        }
        true
    }

    pub fn project_is_open(&self, path: &str) -> bool {
        let root = self.group_path(path);
        self.projects
            .iter()
            .any(|p| self.group_path(&p.path) == root)
    }

    /// Close a project group in navigation, retaining sessions, drafts and work.
    pub fn close_project(&mut self, path: &str) {
        let root = self.group_path(path).to_owned();
        let Some(position) = self
            .projects
            .iter()
            .position(|p| self.group_path(&p.path) == root)
        else {
            return;
        };
        let closed: std::collections::HashSet<_> = self
            .projects
            .iter()
            .filter(|p| self.group_path(&p.path) == root)
            .map(|p| p.path.clone())
            .collect();
        let active = self
            .active_project
            .as_ref()
            .is_some_and(|p| self.group_path(p) == root);
        self.closed_projects.insert(root.clone(), self.next_request);
        self.projects.retain(|p| !closed.contains(&p.path));
        if let Some(index) = self
            .last_archived
            .filter(|i| self.group_path(&self.sessions[*i].scope.project) == root)
        {
            self.dismiss_archive(index);
        }
        if active {
            if let Some(index) = self.selected {
                self.dismiss_side_question(index);
            }
            self.navigation += 1;
            self.active_project = None;
            self.selected = None;
            if let Some(project) = self
                .projects
                .get(position.min(self.projects.len().saturating_sub(1)))
            {
                let path = project.path.clone();
                self.select_project(&path);
            }
        }
    }

    pub(super) fn closed_since(&self, project: &str, request: u64) -> bool {
        self.closed_projects
            .get(self.group_path(project))
            .is_some_and(|closed| request <= *closed)
    }

    pub fn group_path<'a>(&'a self, path: &'a str) -> &'a str {
        self.project_roots
            .get(path)
            .map(String::as_str)
            .unwrap_or(path)
    }

    pub fn refresh_project(&mut self) {
        if let Some(path) = self.active_project.clone() {
            self.task(Task::Git {
                project: path.clone(),
            });
            self.task(Task::Worktrees {
                project: path.clone(),
                directory: self.preferences.worktree_directory.clone(),
            });
            if let Some(branch) = self
                .services
                .checkouts
                .get(&path)
                .and_then(|c| c.git.as_ref())
                .and_then(|g| g.branch.clone())
            {
                self.task(Task::Discover {
                    project: path.clone(),
                    branch,
                });
            }
            for provider in Provider::ALL {
                let scope = Scope {
                    project: path.clone(),
                    provider,
                };
                // A repeated refresh does not accumulate duplicate network calls.
                if self
                    .pending
                    .values()
                    .any(|pending| matches!(pending, Pending::List(current) if *current == scope))
                {
                    continue;
                }
                self.notices.remove(&scope);
                self.request(Command::List(scope.clone()), Pending::List(scope.clone()));
                self.request(
                    Command::Choices(scope.clone()),
                    Pending::Choices(scope.clone()),
                );
                if provider == Provider::OpenCode {
                    self.request(Command::Commands(scope.clone()), Pending::Commands(scope));
                }
            }
        }
    }
}
