mod compare;

use super::{Pending, Workspace};
use crate::application::{Command, QueuedPrompt, SavedSession, Snapshot, Store, Task};
use std::collections::HashMap;

type Requests = Vec<(u64, Command)>;

#[derive(Default)]
pub(super) enum State {
    #[default]
    Disabled,
    Unavailable(String),
    Loading {
        request: u64,
        store: Store,
    },
    LoadFailed(String),
    Loaded(Loaded),
}

pub(super) struct Loaded {
    store: Store,
    saved: Option<Box<Snapshot>>,
    write: Write,
}

enum Write {
    Ready,
    Saving {
        request: u64,
        snapshot: Box<Snapshot>,
        commands: Requests,
    },
    Failed(String),
}

impl Workspace {
    pub fn commands(&mut self) -> Vec<(u64, Command)> {
        if matches!(self.persistence, State::Disabled | State::Unavailable(_)) {
            return std::mem::take(&mut self.outgoing);
        }
        let mut immediate = std::mem::take(&mut self.ready_outgoing);
        for request in std::mem::take(&mut self.outgoing) {
            if (self.restored()
                && matches!(
                    request.1,
                    Command::Task(
                        Task::ReviewCache { .. }
                            | Task::Details { .. }
                            | Task::ReviewFiles { .. }
                            | Task::Feedback { .. }
                    )
                ))
                || matches!(
                    request.1,
                    Command::Load(_)
                        | Command::Task(Task::Updates | Task::Update(_))
                        | Command::Cancel(_)
                        | Command::Remote(crate::remote::Operation::Interrupt(..))
                        | Command::Task(
                            Task::Shell { .. }
                                | Task::Git { .. }
                                | Task::Diff { .. }
                                | Task::File { .. }
                                | Task::Directory { .. }
                        )
                        | Command::Approve(..)
                        | Command::Answer(..)
                        | Command::Notify(..)
                        | Command::Sound(_)
                )
            {
                immediate.push(request);
            } else {
                self.durable_outgoing.push(request);
            }
        }
        let State::Loaded(Loaded {
            store,
            saved,
            write: Write::Ready,
        }) = &self.persistence
        else {
            return immediate;
        };
        if saved
            .as_ref()
            .is_none_or(|saved| !self.matches_snapshot(saved))
        {
            let snapshot = Box::new(self.snapshot());
            let commands = std::mem::take(&mut self.durable_outgoing);
            let request = self.request(
                Command::Save(store.clone(), snapshot.clone()),
                Pending::Save,
            );
            if let State::Loaded(loaded) = &mut self.persistence {
                loaded.write = Write::Saving {
                    request,
                    snapshot,
                    commands,
                };
            }
            immediate.append(&mut self.outgoing);
        } else {
            immediate.append(&mut self.durable_outgoing);
        }
        immediate
    }

    pub fn enable_store(&mut self, path: std::path::PathBuf) {
        if !matches!(self.persistence, State::Disabled | State::Unavailable(_)) {
            return;
        }
        let store = Store(path);
        let request = self.request(Command::Load(store.clone()), Pending::Load);
        self.persistence = State::Loading { request, store };
    }

    pub fn store_unavailable(&mut self, error: String) {
        if matches!(self.persistence, State::Disabled | State::Unavailable(_)) {
            self.persistence = State::Unavailable(error);
        }
    }

    pub fn persistence_error(&self) -> Option<&String> {
        match &self.persistence {
            State::Unavailable(error) | State::LoadFailed(error) => Some(error),
            State::Loaded(Loaded {
                write: Write::Failed(error),
                ..
            }) => Some(error),
            _ => None,
        }
    }

    pub(super) fn store_loaded(
        &mut self,
        id: u64,
        result: Result<Option<&Snapshot>, String>,
    ) -> bool {
        let State::Loading { request, store } = &self.persistence else {
            return false;
        };
        if *request != id {
            return false;
        }
        self.persistence = match result {
            Ok(saved) => State::Loaded(Loaded {
                store: store.clone(),
                saved: saved.cloned().map(Box::new),
                write: Write::Ready,
            }),
            Err(error) => State::LoadFailed(error),
        };
        true
    }

    pub(super) fn store_saved(&mut self, id: u64, result: Result<(), String>) {
        let State::Loaded(loaded) = &mut self.persistence else {
            return;
        };
        if !matches!(loaded.write, Write::Saving { request, .. } if request == id) {
            return;
        }
        let Write::Saving {
            snapshot,
            mut commands,
            ..
        } = std::mem::replace(&mut loaded.write, Write::Ready)
        else {
            unreachable!();
        };
        match result {
            Ok(()) => {
                loaded.saved = Some(snapshot);
                self.ready_outgoing.append(&mut commands);
            }
            Err(error) => {
                loaded.write = Write::Failed(error);
                commands.append(&mut self.durable_outgoing);
                self.durable_outgoing = commands;
            }
        }
    }

    pub fn retry_save(&mut self) {
        if let State::Loaded(loaded) = &mut self.persistence {
            if matches!(loaded.write, Write::Failed(_)) {
                loaded.write = Write::Ready;
            }
        }
    }

    pub fn restored(&self) -> bool {
        matches!(
            self.persistence,
            State::Disabled | State::Unavailable(_) | State::Loaded(_)
        )
    }

    /// Native restart waits for the exact current editor/queue snapshot.
    pub fn saved_for_restart(&self) -> bool {
        self.home.attaching == 0
            && self.sessions.iter().all(|session| session.attaching == 0)
            && match &self.persistence {
                State::Disabled => true,
                State::Loaded(Loaded {
                    write: Write::Ready,
                    saved: Some(saved),
                    ..
                }) => self.matches_snapshot(saved),
                _ => false,
            }
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut preferences = self.preferences.clone();
        for (project, checkout) in &self.services.checkouts {
            if project.is_empty() {
                continue;
            }
            let files = preferences.files.entry(project.clone()).or_default();
            files.tabs = checkout.tabs.clone();
            files.selected = checkout.selected_file.clone();
        }
        let mut session_indices = HashMap::new();
        let sessions = self
            .sessions
            .iter()
            .enumerate()
            .filter_map(|(index, s)| {
                if s.forgotten {
                    return None;
                }
                let saved_index = session_indices.len();
                session_indices.insert(index, saved_index);
                Some(SavedSession {
                    vm_link: s.vm_link.clone(),
                    last_activity: s.last_activity,
                    scope: s.scope.clone(),
                    remote_id: s.remote_id.clone(),
                    title: s.title.clone(),
                    title_refresh_turns: s.title_state.turns,
                    manual_title: s.title_state.manual,
                    options: s.options.clone(),
                    draft: s.draft.clone(),
                    attachments: s.attachments.clone(),
                    failed_prompt: s.failed_prompt.clone(),
                    pending_submission: s.inflight_prompt.as_ref().map(|text| QueuedPrompt {
                        text: text.clone(),
                        options: s.inflight_options.clone(),
                    }),
                    queue: s.queue.clone(),
                    inbox: s.inbox.items.clone(),
                    archived: s.archived,
                    reading: s.reading,
                    unread: s.unread,
                })
            })
            .collect();
        Snapshot {
            version: 1,
            remote: self.remote.saved.clone(),
            home: self.home.draft.clone(),
            projects: self.projects.clone(),
            selected: self
                .selected
                .and_then(|index| session_indices.get(&index).copied()),
            preferences,
            remembered: self
                .remembered
                .iter()
                .filter_map(|(key, index)| {
                    session_indices
                        .get(index)
                        .map(|index| (key.clone(), *index))
                })
                .collect(),
            project_roots: self.project_roots.clone(),
            managed_worktrees: self.managed_worktrees.clone(),
            remembered_checkouts: self
                .remembered_checkouts
                .iter()
                .filter_map(|(key, index)| {
                    session_indices
                        .get(index)
                        .map(|index| (key.clone(), *index))
                })
                .collect(),
            sessions,
        }
    }

    /// Called after background workers have stopped, so an older write cannot
    /// replace the final editor snapshot during normal window shutdown.
    pub fn flush(&self) -> Result<(), String> {
        if let State::Loaded(Loaded { store, .. }) = &self.persistence {
            return store.save(&self.snapshot());
        }
        Ok(())
    }
}
