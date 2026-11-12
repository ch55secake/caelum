use super::{Pending, Workspace};
use crate::application::{presentation::now, Command, Provider};

const TURN_INTERVAL: u8 = 3;
const MANUAL_COOLDOWN_MS: u64 = 60_000;

pub(super) enum Refresh {
    Automatic,
    Manual,
}

#[derive(Default)]
pub(super) struct TitleState {
    /// Successful executions since the last request, capped to coalesce work.
    pub(super) turns: u8,
    /// Snapshots requested before a live rename must not restore an old title.
    pub(super) barrier: u64,
    /// Last request or successful refresh, in Unix milliseconds.
    pub(super) refreshed_at: Option<u64>,
}

impl TitleState {
    pub(super) fn completed_turn(&mut self) {
        self.turns = self.turns.saturating_add(1).min(TURN_INTERVAL);
    }

    pub(super) fn refreshed(&mut self, at: u64) {
        self.refreshed_at = Some(self.refreshed_at.unwrap_or_default().max(at));
    }
}

impl Workspace {
    pub(super) fn refresh_title(&mut self, index: usize, refresh: Refresh) {
        let now = now();
        let session = &mut self.sessions[index];
        let due = match refresh {
            Refresh::Automatic => session.title_state.turns >= TURN_INTERVAL,
            Refresh::Manual => session
                .title_state
                .refreshed_at
                .is_none_or(|at| now.saturating_sub(at) >= MANUAL_COOLDOWN_MS),
        };
        if session.scope.provider != Provider::OpenCode
            || !due
            || session.archived
            || session.archiving
            || session.forgotten
            || session.deleting
            || session.disconnected
        {
            return;
        }
        let Some(key) = session.key() else {
            return;
        };
        if self
            .pending
            .values()
            .any(|pending| matches!(pending, Pending::RegenerateTitle(_, owner) if *owner == key))
        {
            return;
        }
        // Failures wait for another interval rather than repeatedly charging
        // for retries. Turns completed in flight accrue toward one follow-up.
        session.title_state.turns = 0;
        session.title_state.refreshed(now);
        self.request(
            Command::RegenerateTitle(key.clone()),
            Pending::RegenerateTitle(index, key),
        );
    }
}
