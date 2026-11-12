use super::{Pending, Workspace};
use crate::application::{Command, Provider, Reply, Scope, Task, TaskReply};

impl Workspace {
    pub fn task(&mut self, task: Task) {
        if let Task::RemoveWorktree { path, project } = &task {
            if self.sessions.iter().any(|s| {
                std::path::Path::new(&s.scope.project).starts_with(path)
                    && !s.forgotten
                    && (s.working()
                        || s.attaching > 0
                        || s.generating
                        || s.archiving
                        || !s.queue.is_empty()
                        || !s.draft.is_empty()
                        || !s.attachments.is_empty())
            }) {
                self.services
                    .checkouts
                    .entry(project.clone())
                    .or_default()
                    .error = Some(
                    "Finish sessions and clear or move drafts before removing this checkout".into(),
                );
                return;
            }
        }
        if self.services.busy(&task)
            && !matches!(
                task,
                Task::Search { .. } | Task::Completions { .. } | Task::Stats { .. }
            )
        {
            if matches!(
                task,
                Task::Diff { .. } | Task::File { .. } | Task::Directory { .. }
            ) {
                self.refresh_after.insert(task.slot(), task);
            }
            return;
        }
        let id = self.request(Command::Task(task.clone()), Pending::Task(task.clone()));
        if matches!(task, Task::CreateWorktree { .. }) {
            let provider = self
                .selected
                .map(|i| self.sessions[i].scope.provider)
                .unwrap_or(self.preferences.default_provider);
            self.launches.insert(id, (provider, self.navigation));
        }
        self.services.start(&task, id);
    }

    /// Retain a follow-up invalidation while Git is in flight. A tool ending or
    /// an external edit must not be lost behind an older snapshot.
    pub fn checkout_changed(&mut self, project: &str) {
        if !self.projects.iter().any(|p| p.path == project) {
            return;
        }
        let git = Task::Git {
            project: project.into(),
        };
        if self.services.busy(&git) {
            self.dirty_checkouts.insert(project.into());
            return;
        }
        self.dirty_checkouts.remove(project);
        self.task(git);
        let Some(checkout) = self.services.checkouts.get(project) else {
            return;
        };
        let mut tasks = vec![Task::Directory {
            project: project.into(),
            path: String::new(),
            offset: 0,
        }];
        for path in checkout.diffs.keys() {
            tasks.push(Task::Diff {
                project: project.into(),
                path: path.clone(),
            });
        }
        for path in &checkout.tabs {
            tasks.push(Task::File {
                project: project.into(),
                path: path.clone(),
            });
        }
        for path in checkout.directories.keys().filter(|p| !p.is_empty()) {
            tasks.push(Task::Directory {
                project: project.into(),
                path: path.clone(),
                offset: 0,
            });
        }
        for task in tasks {
            self.task(task);
        }
    }

    pub fn launch_worktree(&mut self, task: Task, provider: Provider) {
        let previous = self.next_request;
        self.task(task);
        if self.next_request != previous && self.launches.contains_key(&self.next_request) {
            self.launches
                .insert(self.next_request, (provider, self.navigation));
        }
    }

    pub fn open_file(&mut self, project: String, path: String) {
        let checkout = self.services.checkouts.entry(project.clone()).or_default();
        if !checkout.tabs.contains(&path) {
            checkout.tabs.push(path.clone());
        }
        checkout.selected_file = Some(path.clone());
        if !checkout.files.contains_key(&path) {
            self.task(Task::File { project, path });
        }
    }

    pub fn refresh_files(&mut self) {
        if let Some(project) = self.active_project.clone() {
            if let Some(path) = self
                .services
                .checkouts
                .get(&project)
                .and_then(|c| c.selected_file.clone())
            {
                self.task(Task::File {
                    project: project.clone(),
                    path,
                });
            }
            self.task(Task::Directory {
                project,
                path: String::new(),
                offset: 0,
            });
        }
    }

    pub(super) fn complete_task(
        &mut self,
        request: u64,
        task: Task,
        result: Result<Reply, String>,
    ) {
        let launch = self.launches.remove(&request);
        if !self.services.current(request, &task) {
            self.services
                .complete(request, &task, Err("Superseded request".into()));
            return;
        }
        let result = result.and_then(|r| {
            if let Reply::Task(r) = r {
                Ok(r)
            } else {
                Err("Unexpected workspace response".into())
            }
        });
        if let Ok(TaskReply::Preferences(settings)) = &result {
            self.preferences = settings.clone();
        }
        if let (
            Task::Shell {
                project,
                session,
                id,
                stop: true,
            },
            Ok(TaskReply::Shell(output)),
        ) = (&task, &result)
        {
            let output = if output.is_empty() {
                self.services
                    .checkouts
                    .get(project)
                    .and_then(|c| c.shell_output.get(&(session.clone(), id.clone())))
                    .unwrap_or(output)
            } else {
                output
            };
            for owner in self.sessions.iter_mut().filter(|s| {
                s.scope.provider == Provider::OpenCode
                    && s.scope.project == *project
                    && s.remote_id.as_deref() == Some(session)
            }) {
                if let Some(shell) = owner.shells.iter().find(|s| s.id == *id) {
                    owner.conversation.apply(Provider::OpenCode, "session.shell.ended", &serde_json::json!({
                        "shell": {"id":id,"command":shell.command,"status":"killed","time":{"started":shell.started_at,"completed":crate::application::presentation::now()}},
                        "output":{"output":output}
                    }));
                }
                owner.shells.retain(|s| s.id != *id);
                owner.shell_revision += 1;
                owner.revision += 1;
            }
        }
        if let Ok(TaskReply::Git(_, updates)) = &result {
            for update in updates {
                self.branch_update(update.clone());
            }
        }
        if let Ok(TaskReply::Worktree(worktree)) = &result {
            if let Task::CreateWorktree { project, .. } = &task {
                self.managed_worktrees.insert(worktree.path.clone());
                // Record ownership before opening: closing the group must also
                // reject an in-flight Open for this newly created checkout.
                let root = self.group_path(project).to_owned();
                self.project_roots
                    .insert(worktree.project.path.clone(), root);
                if let Some((provider, _)) = launch.filter(|(_, navigation)| {
                    *navigation == self.navigation && !self.closed_since(project, request)
                }) {
                    self.launch_project(worktree.project.path.clone(), provider);
                }
            }
            if matches!(task, Task::RemoveWorktree { .. }) {
                self.managed_worktrees.remove(&worktree.path);
                let root = self.project_roots.get(&worktree.project.path).cloned();
                self.projects.retain(|p| p.path != worktree.project.path);
                for session in &mut self.sessions {
                    if session.scope.project == worktree.project.path {
                        session.archived = true;
                        session.archive_barrier = self.next_request;
                    }
                }
                self.remembered
                    .retain(|_, i| self.sessions[*i].scope.project != worktree.project.path);
                self.remembered_checkouts
                    .retain(|_, i| self.sessions[*i].scope.project != worktree.project.path);
                self.services.checkouts.remove(&worktree.project.path);
                if self.active_project.as_ref() == Some(&worktree.project.path) {
                    let destination = root
                        .filter(|root| self.projects.iter().any(|p| &p.path == root))
                        .or_else(|| self.projects.first().map(|p| p.path.clone()));
                    self.active_project = None;
                    self.selected = None;
                    if let Some(path) = destination {
                        self.select_open_session_or_new(
                            &path,
                            Scope {
                                project: path.clone(),
                                provider: self.preferences.default_provider,
                            },
                        );
                    }
                }
            }
        }
        if let Ok(TaskReply::Worktrees(Some(listing))) = &result {
            let root = self.group_path(task.project()).to_owned();
            if !self.closed_since(&root, request) && self.project_is_open(&root) {
                // Older versions already created worktrees in this repository's
                // dedicated Caelum storage folder. Remember them across directory
                // preference changes, alongside explicitly recorded creations.
                for worktree in &listing.worktrees {
                    if !worktree.main
                        && std::path::Path::new(&worktree.path).parent()
                            == Some(std::path::Path::new(&listing.storage_directory))
                    {
                        self.managed_worktrees.insert(worktree.path.clone());
                    }
                }
                for worktree in &listing.worktrees {
                    if worktree.project.path != root
                        && (worktree.main || !self.managed_worktrees.contains(&worktree.path))
                    {
                        continue;
                    }
                    let owner = self
                        .project_roots
                        .entry(worktree.project.path.clone())
                        .or_insert_with(|| root.clone());
                    if *owner != root {
                        continue;
                    }
                    if worktree.available && worktree.project.path != root {
                        let mut project_info = worktree.project.clone();
                        project_info.name = worktree.label.clone().unwrap_or_else(|| {
                            std::path::Path::new(&worktree.path)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                        });
                        if let Some(project) = self
                            .projects
                            .iter_mut()
                            .find(|p| p.path == worktree.project.path)
                        {
                            project.name = project_info.name;
                        } else {
                            self.projects.push(project_info);
                            for provider in Provider::ALL {
                                let scope = Scope {
                                    project: worktree.project.path.clone(),
                                    provider,
                                };
                                self.request(
                                    Command::List(scope.clone()),
                                    Pending::List(scope.clone()),
                                );
                                self.refresh_choices_for(scope);
                            }
                        }
                    }
                    if let Some(branch) = &worktree.branch {
                        self.task(Task::Discover {
                            project: worktree.project.path.clone(),
                            branch: branch.clone(),
                        });
                    }
                }
            }
        }
        for mut next in self.services.complete(request, &task, result) {
            if matches!(task, Task::RemoveWorktree { .. }) {
                if let Task::Worktrees { project, directory } = &mut next {
                    if !self.projects.iter().any(|p| p.path == *project) {
                        if let Some(root) = self
                            .project_roots
                            .get(project)
                            .or(self.active_project.as_ref())
                        {
                            *project = root.clone();
                        }
                    }
                    directory.clone_from(&self.preferences.worktree_directory);
                }
            }
            self.task(next);
        }
        if let Task::Shell {
            project,
            session,
            id,
            stop: false,
        } = &task
        {
            let running = self.sessions.iter().any(|owner| {
                owner.scope.provider == Provider::OpenCode
                    && owner.scope.project == *project
                    && owner.remote_id.as_ref() == Some(session)
                    && owner.shells.iter().any(|shell| shell.id == *id)
            });
            if !running {
                if let Some(checkout) = self.services.checkouts.get_mut(project) {
                    checkout.shell_output.remove(&(session.clone(), id.clone()));
                }
            }
        }
        if let Some(next) = self.refresh_after.remove(&task.slot()) {
            self.task(next);
        }
        if let Task::Git { project } = &task {
            if self.dirty_checkouts.remove(project) {
                self.checkout_changed(project);
            }
        }
    }
}
