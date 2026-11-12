use super::{titles::Refresh, update_summary, Pending, Session, Workspace};
use crate::application::{Command, Provider, Scope, Summary};

impl Workspace {
    pub fn refresh_open_sessions(&mut self) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                !session.archived
                    && !session.forgotten
                    && self.project_is_open(&session.scope.project)
            })
            .map(|(index, _)| index)
            .collect();
        for index in sessions {
            self.refresh_choices_for(self.sessions[index].scope.clone());
            self.refresh_session(index);
        }
    }

    /// Explicit user refresh of history and, unless recently refreshed, the title.
    pub fn refresh_session(&mut self, index: usize) {
        self.refresh(index);
        self.refresh_title(index, Refresh::Manual);
    }

    /// Refresh both active and archived provider sessions for a picker.
    pub fn discover_sessions(&mut self, scope: Scope) {
        self.notices.remove(&scope);
        if !self
            .pending
            .values()
            .any(|pending| matches!(pending, Pending::List(current) if current == &scope))
        {
            self.request(Command::List(scope.clone()), Pending::List(scope.clone()));
        }
        if !self
            .pending
            .values()
            .any(|pending| matches!(pending, Pending::ListArchived(current) if current == &scope))
        {
            self.request(
                Command::ListArchived(scope.clone()),
                Pending::ListArchived(scope),
            );
        }
    }

    pub fn refresh_choices(&mut self, provider: Provider) {
        let Some(project) = self.active_project.clone() else {
            return;
        };
        self.refresh_choices_for(Scope { project, provider });
    }

    pub fn refresh_choices_for(&mut self, scope: Scope) {
        if self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Choices(s) if s == &scope))
        {
            return;
        }
        self.notices.remove(&scope);
        self.request(
            Command::Choices(scope.clone()),
            Pending::Choices(scope.clone()),
        );
        if scope.provider == Provider::OpenCode {
            self.request(Command::Commands(scope.clone()), Pending::Commands(scope));
        }
    }

    pub fn test_sound(&mut self) {
        self.request(
            Command::Sound(self.preferences.sound_volume),
            Pending::Notify,
        );
    }

    pub fn new_session(&mut self, scope: Scope) -> usize {
        let mut session = Session::new(scope.clone());
        session.record_activity(crate::application::presentation::now());
        session.options = self
            .preferences
            .providers
            .get(scope.provider.as_str())
            .cloned()
            .unwrap_or_default();
        if let Some(choices) = self.choices.get(&scope) {
            if session.options.model.is_none() {
                session.options.model = choices.default_model.clone();
            }
        }
        let index = self.sessions.len();
        self.sessions.push(session);
        self.select(index);
        index
    }

    pub fn project_draft(&mut self, scope: Scope) -> usize {
        if let Some(i) = self.sessions.iter().rposition(|s| {
            s.scope == scope && s.remote_id.is_none() && !s.archived && !s.forgotten && !s.working()
        }) {
            self.select(i);
            i
        } else {
            self.new_session(scope)
        }
    }

    pub fn acknowledge(&mut self) -> bool {
        if let Some(i) = self.selected {
            if !self.window_background
                && !self.conversation_hidden
                && self.sessions[i].loaded
                && !self.sessions[i].loading
                && self.sessions[i].reading < 0.0
            {
                return std::mem::take(&mut self.sessions[i].unread);
            }
        }
        false
    }

    pub fn select(&mut self, index: usize) {
        self.notifications.cancel();
        if index >= self.sessions.len() || self.sessions[index].forgotten {
            return;
        }
        if self.selected != Some(index) {
            if let Some(previous) = self.selected {
                self.dismiss_side_question(previous);
            }
            self.navigation += 1;
        }
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        self.active_project = Some(session.scope.project.clone());
        self.remembered.insert(session.scope.project.clone(), index);
        self.remembered_checkouts
            .insert(session.scope.project.clone(), index);
        if let Some(root) = self.project_roots.get(&session.scope.project) {
            self.remembered.insert(root.clone(), index);
        }
        self.selected = Some(index);
        if !session.loaded {
            self.refresh(index);
        }
    }

    pub fn refresh(&mut self, index: usize) {
        let session = &mut self.sessions[index];
        if session.loading || session.submitting {
            return;
        }
        let Some(key) = session.key() else {
            return;
        };
        session.loading = true;
        session.error = None;
        let revision = session.revision;
        self.request(Command::Resume(key), Pending::Resume(index, revision));
    }

    pub fn older(&mut self, index: usize) {
        let session = &mut self.sessions[index];
        if session.loading {
            return;
        }
        let (Some(key), Some(cursor)) = (session.key(), session.next_cursor.clone()) else {
            return;
        };
        session.loading = true;
        let revision = session.revision;
        self.request(
            Command::History(key, Some(cursor)),
            Pending::History(index, revision, true),
        );
    }

    pub(super) fn creating(&self, scope: &Scope) -> bool {
        self.pending.values().any(|pending| matches!(pending, Pending::Start(index, _) if self.sessions[*index].scope == *scope))
    }

    pub(super) fn apply_summaries(
        &mut self,
        request: u64,
        scope: &Scope,
        summaries: Vec<Summary>,
        archived: bool,
    ) {
        for summary in summaries
            .into_iter()
            .filter(|summary| summary.key.scope == *scope)
        {
            if let Some(session) = self
                .sessions
                .iter_mut()
                .find(|session| session.key().as_ref() == Some(&summary.key))
            {
                if session.forgotten {
                    continue;
                }
                if !session.archiving && request > session.archive_barrier {
                    session.archived = archived;
                }
                if session.revision == 0 {
                    session.conversation.working = summary.working;
                    update_summary(session, summary, request);
                }
            } else {
                let mut session = Session::new(scope.clone());
                session.remote_id = Some(summary.key.id.clone());
                session.loaded = false;
                session.conversation.working = summary.working;
                session.archived = archived;
                update_summary(&mut session, summary, request);
                self.sessions.push(session);
                self.detail_queue.push(self.sessions.len() - 1);
            }
        }
        self.schedule_details();
    }

    pub(super) fn schedule_details(&mut self) {
        let count = self
            .pending
            .values()
            .filter(|p| matches!(p, Pending::Details(_)))
            .count();
        for _ in count..6 {
            let Some(index) = self.detail_queue.pop() else {
                break;
            };
            if let Some(key) = self.sessions[index].key() {
                self.request(Command::Details(key), Pending::Details(index));
            }
        }
    }
}
