use super::{Pending, Workspace};
use crate::{
    application::{Command, Event},
    remote::native::{checkout_id, Binding},
};
use std::collections::BTreeSet;

impl Workspace {
    pub(in crate::application::workspace) fn track_native_request(
        &mut self,
        id: u64,
        command: &mut Command,
        pending: &Pending,
    ) {
        let project = match pending {
            // Boat checks the base before the VM provider starts a thread.
            Pending::CheckHandoff(i, _) => Some(self.sessions[*i].scope.project.as_str()),
            _ => Binding::owner(command).map(|p| p.as_str()),
        };
        if let Some(project) = project.filter(|p| checkout_id(p).is_some()) {
            self.remote.native_requests.insert(id, project.to_owned());
        }
    }

    /// End this attachment's requests, including effects still held behind a local save.
    /// History and the new broker snapshots decide what actually happened on the VM.
    #[must_use = "deliver the retired request completions before applying reconnect events"]
    pub fn native_connection_lost(&mut self, project: &str) -> Vec<Event> {
        for session in self
            .sessions
            .iter_mut()
            .filter(|s| s.scope.project == project)
        {
            session.revision += 1;
            session.queue_paused = true;
            session.disconnected = session.remote_id.is_some();
            session.conversation.working = false;
        }
        self.deferred_lists
            .retain(|scope, _| scope.project != project);
        self.detail_queue
            .retain(|i| self.sessions[*i].scope.project != project);
        self.dirty_checkouts.remove(project);
        self.refresh_after.retain(|_, t| t.project() != project);
        let ids: BTreeSet<_> = self
            .remote
            .native_requests
            .iter()
            .filter_map(|(&id, owner)| (owner == project).then_some(id))
            .collect();
        if ids.is_empty() {
            return Vec::new();
        }
        let keep = |(id, _): &(u64, Command)| !ids.contains(id);
        self.outgoing.retain(keep);
        self.ready_outgoing.retain(keep);
        self.durable_outgoing.retain(keep);
        for pending in self.pending.values_mut() {
            if let Pending::Save(_, commands) = pending {
                commands.retain(keep);
            }
        }
        let error = "VM connection closed. Delivery is unconfirmed; check the session history before retrying.";
        ids.into_iter()
            .map(|r| Event::Completed(r, Err(error.into())))
            .collect()
    }
}
