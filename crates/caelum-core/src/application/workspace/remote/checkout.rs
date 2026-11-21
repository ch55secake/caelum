use super::{Lane, Pending, Workspace};
use crate::{
    application::{Command, Provider, Reply},
    remote::*,
};
use std::time::{Duration, Instant};

impl Workspace {
    pub fn remote_picker(&mut self, project: String) {
        if let Some(h) = self.vm_handoff() {
            if h.project == project {
                self.discover_remote_source(project, true);
                self.list_machines();
            }
            return;
        }
        let project = self
            .native_checkout(&project)
            .map(|c| c.source.project.clone())
            .unwrap_or(project);
        if self.projects.iter().any(|p| p.path == project) {
            self.discover_remote_source(project, false);
        } else {
            // A folder selected in the VM dialog goes through the normal project
            // validation worker without opening a conversation on the laptop.
            let request = self.request(
                Command::Open(project.clone()),
                Pending::Remote(Lane::OpenProject),
            );
            self.remote.picker = Some(Picker::Loading { request, project });
        }
        self.list_machines();
    }

    pub(super) fn complete_remote_project(&mut self, request: u64, result: Result<Reply, String>) {
        let Some(Picker::Loading {
            request: active,
            project: requested,
        }) = &self.remote.picker
        else {
            return;
        };
        if request != *active {
            return;
        }
        match result {
            Ok(Reply::Project(project)) if !self.closed_since(&project.path, request) => {
                let path = project.path.clone();
                if !self.projects.iter().any(|p| p.path == project.path) {
                    self.projects.push(project);
                }
                self.discover_remote_source(path, false);
            }
            other => {
                self.remote.picker = Some(Picker::Failed {
                    project: requested.clone(),
                    error: other
                        .err()
                        .unwrap_or_else(|| "The selected project is no longer available".into()),
                })
            }
        }
    }

    pub(super) fn discover_remote_source(&mut self, project: String, handoff: bool) {
        let op = if handoff {
            Operation::DiscoverHandoff(project.clone())
        } else {
            Operation::Discover(project.clone())
        };
        let request = self.request(Command::Remote(op), Pending::Remote(Lane::Picker));
        self.remote.picker = Some(Picker::Loading { request, project });
    }

    pub(super) fn complete_remote_picker(&mut self, request: u64, result: Result<Outcome, String>) {
        let Some(Picker::Loading {
            request: active,
            project,
        }) = &self.remote.picker
        else {
            return;
        };
        if request != *active {
            return;
        }
        self.remote.picker = Some(match result {
            Ok(Outcome::Source(s)) if &s.project == project => Picker::Ready(s),
            other => Picker::Failed {
                project: project.clone(),
                error: other
                    .err()
                    .unwrap_or_else(|| "Unexpected project source".into()),
            },
        });
    }

    pub fn create_remote(
        &mut self,
        size: Size,
        lifetime: Lifetime,
        provider: Provider,
    ) -> Option<String> {
        let Some(Picker::Ready(source)) = self.remote.picker.clone() else {
            return None;
        };
        source.published_tip.as_ref()?;
        let branch = source.base.clone();
        let id = self.register_vm(source, &branch, size, lifetime)?;
        self.remote.saved.checkouts.get_mut(&id)?.thread.provider = provider;
        self.select_remote(&id);
        Some(id)
    }

    pub(super) fn register_vm(
        &mut self,
        source: Source,
        branch: &str,
        size: Size,
        lifetime: Lifetime,
    ) -> Option<String> {
        if !source.valid_plan_branch(branch) {
            return None;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let plan = Plan {
            token: id.clone(),
            branch: branch.into(),
            size,
            lifetime,
        };
        self.remote.saved.checkouts.insert(
            id.clone(),
            Checkout {
                defaults: None,
                source,
                phase: Phase::Create(plan),
                root: None,
                thread: Thread::for_checkout(),
                other_threads: Vec::new(),
                pinned_stats: false,
            },
        );
        self.remote.picker = None;
        Some(id)
    }

    pub(super) fn registered_checkout(&self, key: &MachineId) -> Option<(&String, &Checkout)> {
        self.remote
            .saved
            .checkouts
            .iter()
            .find(|(_, c)| c.machine().is_some_and(|m| m.key == *key))
    }

    /// The launcher asks for a new session; an ordinary attachment reopens the saved one.
    pub fn attach_remote(
        &mut self,
        machine: Machine,
        new_session: Option<Provider>,
    ) -> Option<String> {
        if !self
            .remote
            .inventory
            .as_ref()
            .is_some_and(|i| i.machines.iter().any(|m| m.key == machine.key))
        {
            return None;
        }
        if new_session.is_some() && self.vm_session_machine_blocker(&machine).is_some() {
            return None;
        }
        let existing = self.registered_checkout(&machine.key).is_some();
        let project = match &self.remote.picker {
            Some(Picker::Ready(s)) => s.project.clone(),
            _ => return None,
        };
        let id = self.register_machine(machine)?;
        if existing && new_session.is_some() {
            self.check_vm_destination(&id, project);
            return Some(id);
        }
        if let Some(provider) = new_session {
            self.remote.saved.checkouts.get_mut(&id)?.thread.provider = provider;
        }
        self.select_remote(&id);
        Some(id)
    }

    pub(super) fn register_machine(&mut self, machine: Machine) -> Option<String> {
        let Some(Picker::Ready(source)) = self.remote.picker.clone() else {
            return None;
        };
        if machine.key.account != source.account {
            return None;
        }
        let existing = self.registered_checkout(&machine.key);
        let id = if let Some((id, c)) = existing {
            if matches!(self.remote.jobs.get(id), Some(Job::Deleting(_)))
                || !c.source.repository.eq_ignore_ascii_case(&source.repository)
            {
                return None;
            }
            id.clone()
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            self.remote.saved.checkouts.insert(
                id.clone(),
                Checkout {
                    defaults: None,
                    source,
                    root: None,
                    thread: Thread::for_checkout(),
                    other_threads: Vec::new(),
                    pinned_stats: false,
                    phase: Phase::Prepare {
                        machine,
                        new_branch: None,
                    },
                },
            );
            id
        };
        self.remote.picker = None;
        Some(id)
    }

    pub fn select_remote(&mut self, id: &str) {
        if !self.remote.saved.checkouts.contains_key(id) {
            return;
        }
        self.leave_conversation();
        self.active_project = None;
        self.remote.saved.selected = Some(id.to_owned());
        self.refresh_remote(id);
    }

    pub fn check_vm_destination(&mut self, id: &str, project: String) {
        self.dismiss_vm_destination(id);
        self.remote.branch_checks.insert(id.into(), project);
        self.select_remote(id);
    }

    /// A branch check belongs to one launch, not to every session already on
    /// this VM. A cancelled read cannot admit a later launch from another worktree.
    pub fn dismiss_vm_destination(&mut self, id: &str) {
        if self.remote.branch_checks.remove(id).is_none() {
            return;
        }
        match self.remote.jobs.get(id) {
            Some(Job::Busy(r)) if matches!(self.pending.get(r),Some(Pending::Remote(Lane::MatchCheckout(owner,_))) if owner==id) =>
            {
                let r = *r;
                self.pending.remove(&r);
                self.remote.jobs.remove(id);
            }
            Some(Job::Failed(_)) => {
                self.remote.jobs.remove(id);
            }
            _ => {}
        }
    }

    pub fn refresh_remote(&mut self, id: &str) {
        let Some(c) = self.remote.saved.checkouts.get(id) else {
            return;
        };
        let op = match &c.phase {
            Phase::Create(plan) => Operation::Create(c.source.clone(), plan.clone()),
            Phase::Ready { .. }
            | Phase::Prepare {
                new_branch: None, ..
            } if self.remote.branch_checks.contains_key(id) => {
                Operation::MatchCheckout(self.remote.branch_checks[id].clone(), c.target().unwrap())
            }
            Phase::Prepare { new_branch, .. } => Operation::Prepare(
                c.target().unwrap(),
                c.source.base.clone(),
                new_branch.clone(),
            ),
            Phase::Ready { .. } => {
                Operation::Prepare(c.target().unwrap(), c.source.base.clone(), None)
            }
        };
        self.checkout_request(id, op);
    }

    pub fn power_remote(&mut self, id: &str, resume: bool) {
        if self.publishing(id) {
            return;
        }
        if let Some(machine) = self
            .remote
            .saved
            .checkouts
            .get(id)
            .and_then(Checkout::machine)
        {
            self.checkout_request(id, Operation::Power(machine.key.clone(), resume));
        }
    }

    fn checkout_request(&mut self, id: &str, op: Operation) {
        if matches!(
            self.remote.jobs.get(id),
            Some(Job::Busy(_) | Job::Deleting(_))
        ) || matches!(self.remote.lifetimes.get(id), Some(Job::Busy(_)))
        {
            return;
        }
        if self.remote.monitor.owns(id) {
            self.remote.monitor = Monitor::Off;
        }
        let lane = match &op {
            Operation::MatchCheckout(project, _) => Lane::MatchCheckout(id.into(), project.clone()),
            _ => Lane::Checkout(id.into()),
        };
        let request = self.request(Command::Remote(op), Pending::Remote(lane));
        self.remote.jobs.insert(id.into(), Job::Busy(request));
    }

    pub(super) fn complete_checkout(
        &mut self,
        id: &str,
        request: u64,
        result: Result<Outcome, String>,
    ) {
        if !matches!(self.remote.jobs.get(id), Some(Job::Busy(r)) if *r == request) {
            return;
        }
        self.remote.jobs.remove(id);
        let Some(c) = self.remote.saved.checkouts.get_mut(id) else {
            return;
        };
        let valid = |m: &Machine| {
            m.key.account == c.source.account && c.machine().is_none_or(|old| old.key == m.key)
        };
        let new_branch = match &c.phase {
            Phase::Create(p) => Some(p.branch.clone()),
            Phase::Prepare { new_branch, .. } => new_branch.clone(),
            Phase::Ready { .. } => None,
        };
        match result {
            Ok(Outcome::Prepared {
                machine,
                branch,
                root,
            }) if valid(&machine) && c.root.as_ref().is_none_or(|old| old == &root) => {
                c.root = Some(root);
                c.phase = Phase::Ready { machine, branch };
                if self.remote.branch_checks.contains_key(id) {
                    self.refresh_remote(id);
                    return;
                }
                self.follow_remote(id);
            }
            Ok(Outcome::Machine(machine)) if valid(&machine) => {
                let terminal = matches!(
                    machine.state.as_str(),
                    "archived" | "stopped" | "error" | "failed" | "cancelled"
                );
                c.phase = Phase::Prepare {
                    machine,
                    new_branch,
                };
                if !terminal {
                    self.refresh_remote(id);
                }
            }
            Ok(Outcome::Waiting(machine)) if valid(&machine) => {
                c.phase = Phase::Prepare {
                    machine,
                    new_branch,
                };
                self.remote.jobs.insert(
                    id.into(),
                    Job::Wait(Instant::now() + Duration::from_secs(2)),
                );
            }
            other => {
                self.remote.jobs.insert(
                    id.into(),
                    Job::Failed(
                        other
                            .err()
                            .unwrap_or_else(|| "Unexpected machine response".into()),
                    ),
                );
            }
        }
    }

    /// Only an in-progress provisioning/stop requests another wake.
    pub fn remote_deadline(&mut self, now: Instant) -> Option<Instant> {
        let due: Vec<_> = self
            .remote
            .jobs
            .iter()
            .filter(|(_, job)| matches!(job, Job::Wait(at) if *at <= now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            self.refresh_remote(&id);
        }
        self.remote
            .jobs
            .values()
            .filter_map(|job| {
                if let Job::Wait(at) = job {
                    Some(*at)
                } else {
                    None
                }
            })
            .min()
    }
}
