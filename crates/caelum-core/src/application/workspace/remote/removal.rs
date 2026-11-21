use super::{Lane, Pending, Workspace};
use crate::{
    application::Command,
    remote::{Job, MachineId, Monitor, Operation, Outcome, Turn},
};

impl Workspace {
    pub fn can_remove_remote(&self, id: &str) -> bool {
        self.remote.saved.checkouts.get(id).is_some_and(|c| {
            !(c.thread.turn == Turn::Submitting
                && matches!(self.remote.feeds.get(id), Some(Job::Busy(_))))
        }) && !matches!(
            self.remote.jobs.get(id),
            Some(Job::Busy(_) | Job::Deleting(_))
        ) && !matches!(self.remote.lifetimes.get(id), Some(Job::Busy(_)))
            && !self.remote.interrupts.contains_key(id)
            && !self.publishing(id)
    }

    /// Only delete the machine named in the confirmation. An unallocated setup
    /// has no known Boat ID; discarding it must never provision a VM to delete it.
    pub fn remove_remote(&mut self, id: &str, confirmed: Option<&MachineId>) -> bool {
        if !self.can_remove_remote(id)
            || self.remote.saved.checkouts[id].machine().map(|m| &m.key) != confirmed
        {
            return false;
        }
        self.remote.delete_errors.remove(id);
        if let Some(key) = confirmed {
            let req = self.request(
                Command::Remote(Operation::Delete(key.clone())),
                Pending::Remote(Lane::Delete(id.into(), key.clone())),
            );
            self.remote.jobs.insert(id.into(), Job::Deleting(req));
            if self.remote.monitor.owns(id) {
                self.remote.monitor = Monitor::Off;
            }
        } else {
            self.forget_remote(id);
        }
        true
    }

    pub(super) fn complete_deletion(
        &mut self,
        id: &str,
        req: u64,
        key: &MachineId,
        result: Result<Outcome, String>,
    ) {
        if !matches!(self.remote.jobs.get(id),Some(Job::Deleting(r)) if *r==req)
            || !self
                .remote
                .saved
                .checkouts
                .get(id)
                .and_then(|c| c.machine())
                .is_some_and(|m| &m.key == key)
        {
            return;
        }
        self.remote.jobs.remove(id);
        match result {
            Ok(Outcome::Deleted(deleted)) if &deleted == key => {
                if let Some(inventory) = &mut self.remote.inventory {
                    inventory.machines.retain(|m| &m.key != key);
                }
                // An inventory read started before deletion must not restore this row.
                if matches!(
                    self.remote.connection,
                    crate::remote::Connection::Loading(_)
                ) {
                    // Leave connection mutations alone; filter their older inventories at delivery.
                    self.remote.deleted_during_inventory.push(key.clone());
                }
                self.forget_remote(id);
            }
            other => {
                self.remote.delete_errors.insert(
                    id.into(),
                    other
                        .err()
                        .unwrap_or_else(|| "Boat did not confirm deletion of this machine.".into()),
                );
            }
        }
    }

    fn forget_remote(&mut self, id: &str) {
        let Some(checkout) = self.remote.saved.checkouts.remove(id) else {
            return;
        };
        let selected_inline = self
            .active_project
            .as_deref()
            .and_then(crate::remote::native::checkout_id)
            == Some(id);
        let handoff_id = self
            .selected
            .and_then(|i| self.sessions[i].native_handoff.as_ref())
            .map(|h| h.id.clone());
        let origin = (self.remote.saved.selected.as_deref() == Some(id) || selected_inline)
            .then(|| {
                self.sessions.iter().position(|s| {
                    !s.archived
                        && !s.forgotten
                        && self.project_is_open(&s.scope.project)
                        && s.vm_link.as_ref().is_some_and(|v| {
                            v.checkout == id
                                && v.thread
                                    == handoff_id
                                        .as_deref()
                                        .unwrap_or(checkout.thread.editor_key(id))
                        })
                })
            })
            .flatten();
        self.forget_native_sessions(id);
        self.remote.saved.imported_profiles.remove(id);
        let group = self.group_path(&checkout.source.project).to_owned();
        if !self
            .remote
            .saved
            .checkouts
            .values()
            .any(|c| self.group_path(&c.source.project) == group)
        {
            self.remote.saved.expanded.remove(&group);
        }
        if self.remote.saved.selected.as_deref() == Some(id) {
            self.remote.saved.selected = None;
        }
        for s in &mut self.sessions {
            if s.vm_link.as_ref().is_some_and(|link| link.checkout == id) {
                s.vm_link = None;
            }
        }
        self.remote.jobs.remove(id);
        self.remote.feeds.remove(id);
        self.remote.branch_checks.remove(id);
        if self.remote.tool.as_ref().is_some_and(|p| p.checkout == id) {
            self.remote.tool = None;
        }
        self.remote.interrupts.remove(id);
        self.remote.changes.remove(id);
        self.remote.lifetimes.remove(id);
        if self.remote.monitor.owns(id) {
            self.remote.monitor = Monitor::Off;
        }
        self.remote.delete_errors.remove(id);
        self.remote.removed.push(id.into());
        if let Some(i) = origin {
            self.select(i);
        }
    }
}
