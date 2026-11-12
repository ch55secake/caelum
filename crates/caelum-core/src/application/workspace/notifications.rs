use super::Workspace;
use crate::application::SessionKey;

#[derive(Default)]
pub(super) struct State {
    pending: Option<(SessionKey, u64)>,
}

impl State {
    pub(super) fn take(&mut self, navigation: u64) -> Option<SessionKey> {
        self.pending
            .take()
            .filter(|(_, generation)| *generation == navigation)
            .map(|(target, _)| target)
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

impl Workspace {
    /// Focus only the exact, still-open session. Old notifications never recreate
    /// deleted/archived sessions or redirect to a provider with the same remote ID.
    pub fn activate_notification(&mut self, target: SessionKey) {
        if self.store.is_some() && !self.store_ready {
            if self.persistence_error.is_none() {
                self.notifications.pending = Some((target, self.navigation));
            }
            return;
        }
        if let Some(index) = self.sessions.iter().position(|session| {
            session.key().as_ref() == Some(&target)
                && !session.archived
                && !session.forgotten
                && !session.archiving
                && !session.deleting
                && self.project_is_open(&session.scope.project)
        }) {
            self.select(index);
        }
    }
}
