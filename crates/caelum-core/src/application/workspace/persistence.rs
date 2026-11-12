mod compare;

use super::{Pending, Workspace};
use crate::application::{Command, QueuedPrompt, SavedSession, Snapshot, Store, Task};
use std::collections::HashMap;

impl Workspace {
    pub fn commands(&mut self) -> Vec<(u64, Command)> {
        let Some(store) = self.store.as_ref() else {
            return std::mem::take(&mut self.outgoing);
        };
        let mut immediate = std::mem::take(&mut self.ready_outgoing);
        for request in std::mem::take(&mut self.outgoing) {
            if matches!(
                request.1,
                Command::Load(_)
                    | Command::Cancel(_)
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
            ) {
                immediate.push(request);
            } else {
                self.durable_outgoing.push(request);
            }
        }
        if !self.store_ready || self.saving || self.persistence_error.is_some() {
            return immediate;
        }
        if self
            .saved
            .as_ref()
            .is_none_or(|saved| !self.matches_snapshot(saved))
        {
            let snapshot = self.snapshot();
            self.saving = true;
            let commands = std::mem::take(&mut self.durable_outgoing);
            self.request(
                Command::Save(store.clone(), Box::new(snapshot.clone())),
                Pending::Save(Box::new(snapshot), commands),
            );
            immediate.append(&mut self.outgoing);
        } else {
            immediate.append(&mut self.durable_outgoing);
        }
        immediate
    }

    pub fn enable_store(&mut self, path: std::path::PathBuf) {
        let store = Store(path);
        self.store = Some(store.clone());
        self.request(Command::Load(store), Pending::Load);
    }

    pub fn retry_save(&mut self) {
        if self.store_ready {
            self.persistence_error = None;
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
                    last_activity: s.last_activity,
                    scope: s.scope.clone(),
                    remote_id: s.remote_id.clone(),
                    title: s.title.clone(),
                    title_refresh_turns: s.title_state.turns,
                    title_refreshed_at: s.title_state.refreshed_at,
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
        if self.store_ready {
            if let Some(store) = &self.store {
                return store.save(&self.snapshot());
            }
        }
        Ok(())
    }
}
