use super::{Pending, Workspace};
mod checkout;
mod git;
pub(super) mod handoff;
mod harness;
mod lifetime;
mod link;
mod models;
mod native;
mod removal;
mod stats;
#[cfg(test)]
mod tests;
mod threads;
mod tools;
use crate::{
    application::{Command, Reply},
    remote::{Connection, Credential, Operation, Outcome},
};

pub(super) enum Lane {
    Connection,
    Models,
    OpenProject,
    Picker,
    Handoff,
    ConfirmHandoff(Box<(crate::remote::Source, handoff::VmDestination)>),
    Checkout(String),
    MatchCheckout(String, String),
    Delete(String, crate::remote::MachineId),
    Feed(String),
    Tool,
    Git(String),
    Stats(String),
    Lifetime(String),
    Interrupt(String, crate::remote::Run),
}

impl Workspace {
    pub fn connect_boat(&mut self, key: String) {
        self.connection_request(Operation::Connect(Credential::new(key)));
    }

    pub fn list_machines(&mut self) {
        self.connection_request(Operation::Inventory);
    }

    pub fn disconnect_boat(&mut self) {
        self.connection_request(Operation::Disconnect);
    }

    fn connection_request(&mut self, op: Operation) {
        if matches!(self.remote.connection, Connection::Loading(_)) {
            return;
        }
        let request = self.request(Command::Remote(op), Pending::Remote(Lane::Connection));
        self.remote.deleted_during_inventory.clear();
        self.remote.connection = Connection::Loading(request);
    }

    pub(super) fn complete_remote(
        &mut self,
        request: u64,
        lane: Lane,
        result: Result<Reply, String>,
    ) {
        if let Lane::OpenProject = lane {
            self.complete_remote_project(request, result);
            return;
        }
        if let Lane::Handoff = lane {
            self.complete_handoff_chat(request, result);
            return;
        }
        let result = result.and_then(|reply| match reply {
            Reply::Remote(value) => Ok(value),
            _ => Err("Unexpected remote machine response".into()),
        });
        match lane {
            Lane::Models => self.complete_vm_models(request, result),
            Lane::Delete(id, key) => self.complete_deletion(&id, request, &key, result),
            Lane::Picker => self.complete_remote_picker(request, result),
            Lane::ConfirmHandoff(plan) => {
                self.complete_handoff_confirmation(request, plan.0, plan.1, result)
            }
            Lane::Feed(id) => self.complete_feed(&id, request, result),
            Lane::Tool => self.complete_vm_tool(request, result),
            Lane::Git(id) => self.complete_git(&id, request, result),
            Lane::Stats(id) => self.complete_stats(&id, request, result),
            Lane::Lifetime(id) => self.complete_lifetime(&id, request, result),
            Lane::Interrupt(id, run) => self.complete_interrupt(&id, request, &run, result),
            Lane::Checkout(id) => self.complete_checkout(&id, request, result),
            Lane::MatchCheckout(id, project) => {
                if matches!(self.remote.jobs.get(&id), Some(crate::remote::Job::Busy(active)) if *active==request)
                    && self.remote.branch_checks.get(&id) == Some(&project)
                    && matches!(result, Ok(Outcome::Prepared { .. }))
                {
                    self.remote.branch_checks.remove(&id);
                }
                self.complete_checkout(&id, request, result);
            }
            Lane::Connection if matches!(self.remote.connection, Connection::Loading(id) if id == request) =>
            {
                self.remote.connection = match result {
                    Ok(Outcome::Inventory(mut inventory)) => {
                        if self.remote.tool.as_ref().is_some_and(|p| {
                            self.remote
                                .saved
                                .checkouts
                                .get(&p.checkout)
                                .is_none_or(|c| c.source.account != inventory.account)
                        }) {
                            self.remote.tool = None;
                        }
                        inventory
                            .machines
                            .retain(|m| !self.remote.deleted_during_inventory.contains(&m.key));
                        self.remote.inventory = Some(inventory);
                        if self.remote.picker.is_none() {
                            if let Some(h) = self.vm_handoff() {
                                self.discover_remote_source(h.project.clone(), true);
                            }
                        }
                        Connection::Idle
                    }
                    Ok(Outcome::Disconnected) => {
                        self.remote.inventory = None;
                        self.remote.tool = None;
                        Connection::SignedOut
                    }
                    Err(error) => Connection::Failed(error),
                    _ => Connection::Failed("Unexpected connection response".into()),
                };
            }
            _ => {}
        }
    }
}
