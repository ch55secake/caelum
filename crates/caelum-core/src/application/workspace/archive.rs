use super::{Pending, Session, Workspace};
use crate::application::Command;

impl Workspace {
    pub fn archive(&mut self, index: usize, archived: bool) {
        self.set_archived(index, archived, true);
    }

    pub fn delete_archived_session(&mut self, index: usize) -> bool {
        let Some(session) = self.sessions.get(index) else {
            return false;
        };
        if !session.archived
            || session.forgotten
            || session.archiving
            || session.deleting
            || session.working()
            || !session.queue.is_empty()
            || !session.inbox.items.is_empty()
            || self.approvals.iter().any(|prompt| {
                session.remote_id.is_some()
                    && session.remote_id.as_ref() == prompt.session_id.as_ref()
                    && session.scope.project == prompt.project_path
                    && session.scope.provider.as_str() == prompt.provider
            })
            || self.questions.iter().any(|prompt| {
                session.remote_id.as_deref() == Some(prompt.session_id.as_str())
                    && session.scope.project == prompt.project_path
                    && session.scope.provider.as_str() == prompt.provider
            })
        {
            return false;
        }
        let key = session.key();
        self.sessions[index].error = None;
        if let Some(key) = key {
            self.sessions[index].deleting = true;
            self.request(Command::DeleteSession(key), Pending::DeleteSession(index));
        } else {
            self.forget_deleted_session(index);
        }
        true
    }

    pub(super) fn forget_deleted_session(&mut self, index: usize) {
        if self.sessions[index].forgotten {
            return;
        }
        let selected = self.selected == Some(index);
        self.sessions[index].forgotten = true;
        self.sessions[index].deleting = false;
        self.sessions[index].error = None;
        self.remembered.retain(|_, remembered| *remembered != index);
        self.remembered_checkouts
            .retain(|_, remembered| *remembered != index);
        self.dismiss_archive(index);
        if self.archive_replacement == Some(index) {
            self.archive_replacement = None;
        }
        if selected {
            self.select_archive_replacement(index);
        }
    }

    pub fn undo_archive(&mut self) {
        if let Some(index) = self.last_archived {
            let select =
                self.archive_replacement.is_some() && self.selected == self.archive_replacement;
            self.set_archived(index, false, select);
        }
    }

    /// Presentation lifetime starts at successful archival, and pauses during interaction.
    pub fn tick_archive_notice(&mut self, now: u64, engaged: bool) -> bool {
        let Some(index) = self.last_archived else {
            return false;
        };
        let Some((last, remaining)) = self.archive_clock.as_mut() else {
            return false;
        };
        if !engaged && !self.sessions[index].archiving {
            *remaining = remaining.saturating_sub(now.saturating_sub(*last));
        }
        *last = now;
        if *remaining == 0 {
            self.dismiss_archive(index);
            return true;
        }
        false
    }

    pub fn dismiss_archive(&mut self, index: usize) {
        if self.last_archived == Some(index) {
            self.last_archived = None;
            self.archive_clock = None;
        }
    }

    pub(super) fn archived_notice(&mut self, index: usize, revision: u64) -> bool {
        let project = &self.sessions[index].scope.project;
        if self.closed_projects.contains_key(self.group_path(project))
            && !self.project_is_open(project)
        {
            return false;
        }
        if revision < self.archive_notice_revision {
            return false;
        }
        self.archive_notice_revision = revision;
        self.last_archived = Some(index);
        self.archive_clock = Some((crate::application::presentation::now(), 8_000));
        self.archive_replacement = None;
        true
    }

    pub(super) fn select_archive_replacement(&mut self, index: usize) -> usize {
        let scope = self.sessions[index].scope.clone();
        let group = self.group_path(&scope.project).to_owned();
        let replacement = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(candidate, session)| {
                *candidate != index
                    && !session.archived
                    && !session.forgotten
                    && !session.archiving
                    && self.project_is_open(&session.scope.project)
                    && self.group_path(&session.scope.project) == group
            })
            .max_by_key(|(candidate, session)| {
                (
                    session.has_context(),
                    session.scope.project == scope.project,
                    *candidate,
                )
            })
            .map(|(candidate, _)| candidate);
        let replacement = replacement.unwrap_or_else(|| self.new_session(scope));
        self.select(replacement);
        replacement
    }

    fn set_archived(&mut self, index: usize, archived: bool, select: bool) {
        if self.sessions[index].forgotten {
            return;
        }
        let needs_answer = self.approvals.iter().any(|p| {
            self.sessions[index].remote_id == p.session_id
                && self.sessions[index].scope.project == p.project_path
                && self.sessions[index].scope.provider.as_str() == p.provider
        }) || self.questions.iter().any(|p| {
            self.sessions[index].remote_id.as_ref() == Some(&p.session_id)
                && self.sessions[index].scope.project == p.project_path
                && self.sessions[index].scope.provider.as_str() == p.provider
        });
        let session = &mut self.sessions[index];
        if needs_answer
            || session.working()
            || session.archiving
            || !session.queue.is_empty()
            || !session.inbox.items.is_empty()
        {
            return;
        }
        session.error = None;
        self.archive_revision += 1;
        if let Some(key) = session.key() {
            session.archiving = true;
            self.request(
                Command::Archive(key, archived),
                Pending::Archive(
                    index,
                    archived,
                    select.then(|| (self.selected.unwrap_or(index), self.navigation)),
                    self.archive_revision,
                ),
            );
        } else {
            session.archived = archived;
            session.archive_barrier = self.next_request;
            if archived {
                let owns_notice = self.archived_notice(index, self.archive_revision);
                if self.selected == Some(index) {
                    let replacement = self.select_archive_replacement(index);
                    if owns_notice {
                        self.archive_replacement = Some(replacement);
                    }
                }
            } else {
                self.dismiss_archive(index);
                if select {
                    self.select(index);
                }
            }
        }
    }

    pub(super) fn forget_stale_opencode_session(&mut self, index: usize) {
        let selected = self.selected == Some(index);
        let (scope, options, draft, attachments) = {
            let session = &mut self.sessions[index];
            session.forgotten = true;
            session.archiving = false;
            session.error = None;
            (
                session.scope.clone(),
                session.options.clone(),
                std::mem::take(&mut session.draft),
                std::mem::take(&mut session.attachments),
            )
        };
        self.remembered.retain(|_, remembered| *remembered != index);
        let message = "This OpenCode session no longer belongs to this project. It was removed from Caelum, but was not archived in OpenCode.";
        if selected {
            let mut replacement_session = Session::new(scope);
            replacement_session.options = options;
            replacement_session.draft = draft;
            replacement_session.attachments = attachments;
            replacement_session.error = Some(message.into());
            let replacement = self.sessions.len();
            self.sessions.push(replacement_session);
            self.select(replacement);
        } else {
            self.error = Some(message.into());
        }
    }
}
