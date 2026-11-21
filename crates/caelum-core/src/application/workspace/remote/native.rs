mod defaults;
mod requests;

use super::{Pending, Workspace};
use crate::{
    application::{Command, Event, Provider, Reply, Scope},
    remote::{
        native::{checkout_id, project_key},
        Job, Operation, Outcome, Phase, VmLink,
    },
};

impl Workspace {
    pub fn checkout_directory<'a>(&'a self, project: &'a str) -> &'a str {
        self.native_checkout(project)
            .and_then(|c| c.root.as_deref())
            .unwrap_or(project)
    }
    pub fn native_checkout(&self, project: &str) -> Option<&crate::remote::Checkout> {
        self.remote.saved.checkouts.get(checkout_id(project)?)
    }

    pub fn can_open_vm_session(&self, id: &str) -> bool {
        self.remote.saved.checkouts.get(id).is_some_and(|c| {
            matches!(&c.phase, Phase::Ready { machine, .. } if matches!(machine.state.as_str(),"idle"|"ready"|"running"))
                && c.root.is_some()
                && !matches!(self.remote.jobs.get(id),Some(Job::Deleting(_)))
        })
    }

    /// A VM conversation is a normal stable session slot. The URI is local state;
    /// the provider receives the verified physical directory on its VM.
    pub fn open_vm_session(&mut self, id: &str, new: Option<Provider>) -> Option<usize> {
        let checkout = self.remote.saved.checkouts.get(id)?;
        if matches!(self.remote.jobs.get(id), Some(Job::Deleting(_)))
            || self.remote.branch_checks.contains_key(id)
        {
            return None;
        }
        let ready = self.can_open_vm_session(id);
        let project = project_key(id);
        if new.is_none() && checkout.thread.awaiting_native {
            if !ready || self.remote.jobs.contains_key(id) {
                return None;
            }
            let c = self.remote.saved.checkouts.get_mut(id)?;
            if c.thread.conversation.is_some() || c.thread.continued.is_none() {
                return None;
            }
            let previous = c.other_threads.pop().unwrap_or_default();
            let thread = std::mem::replace(&mut c.thread, previous);
            let h = thread.continued?;
            let i = self.new_session(Scope {
                project,
                provider: thread.provider,
            });
            let s = &mut self.sessions[i];
            s.title.clone_from(&h.title);
            s.draft = thread.draft;
            s.conversation.handoff(&h.chat);
            s.native_handoff = Some(h);
            return Some(i);
        }
        let eligible = |index: usize| {
            let s = &self.sessions[index];
            s.scope.project == project
                && !s.forgotten
                && !s.archived
                && new.is_none_or(|p| {
                    ready
                        && s.scope.provider == p
                        && !s.has_context()
                        && !s.options_dirty
                        && !s.working()
                        && s.attaching == 0
                })
        };
        let existing = self
            .remembered_checkouts
            .get(&project)
            .copied()
            .filter(|&i| eligible(i))
            .or_else(|| (0..self.sessions.len()).rev().find(|&i| eligible(i)));
        if let Some(i) = existing {
            self.select(i);
            return Some(i);
        }
        if !ready {
            return None;
        }
        let provider = new.unwrap_or_else(|| self.default_provider_for(Some(&project)));
        let i = self.new_session(Scope { project, provider });
        Some(i)
    }

    pub(super) fn inline_link(&self, link: &VmLink) -> Option<usize> {
        self.sessions.iter().position(|s| {
            !s.forgotten
                && !s.archived
                && checkout_id(&s.scope.project) == Some(&link.checkout)
                && s.native_handoff
                    .as_ref()
                    .is_some_and(|h| h.id == link.thread)
        })
    }

    pub(in crate::application::workspace) fn check_native_start(&mut self, i: usize, text: String) {
        let s = &self.sessions[i];
        let target = self
            .native_checkout(&s.scope.project)
            .and_then(|c| c.target());
        if let (Some(mut target), Some(base)) = (
            target,
            s.native_handoff.as_ref().and_then(|h| h.base.clone()),
        ) {
            target.required_base = Some(base.clone());
            self.request(
                Command::Remote(Operation::Prepare(target, base.branch, None)),
                Pending::CheckHandoff(i, text),
            );
        } else {
            self.complete(
                0,
                Pending::Start(i, text),
                Err("The handoff checkout is no longer available.".into()),
            );
        }
    }

    pub(in crate::application::workspace) fn checked_native_start(
        &mut self,
        i: usize,
        text: &str,
        result: Result<Reply, String>,
    ) -> Result<(), String> {
        let s = &self.sessions[i];
        let c = self
            .native_checkout(&s.scope.project)
            .ok_or("The handoff VM was removed.")?;
        if s.forgotten
            || s.stopping
            || !matches!(result?, Reply::Remote(Outcome::Prepared {machine,root,..})
            if c.machine().is_some_and(|m|m.key==machine.key) && c.root.as_ref()==Some(&root))
        {
            return Err("The handoff was stopped or the verified VM checkout is not ready.".into());
        }
        self.request(
            Command::Start(s.scope.clone(), s.inflight_options.clone()),
            Pending::Start(i, text.to_owned()),
        );
        Ok(())
    }

    pub fn native_connected(&mut self, project: &str) {
        for provider in Provider::ALL {
            let scope = Scope {
                project: project.to_owned(),
                provider,
            };
            self.discover_sessions(scope.clone());
            self.refresh_choices_for(scope);
        }
        if let Some(i) = self
            .selected
            .filter(|&i| self.sessions[i].scope.project == project)
        {
            self.refresh(i);
            if let Some(key) = self.sessions[i].key() {
                self.request(Command::Details(key), Pending::Details(i));
            }
        }
        if self.active_project.as_deref() == Some(project) {
            self.refresh_project();
            self.refresh_files();
        }
    }

    /// Brokers publish complete snapshots of their own requests, not those of
    /// the laptop and every other connected VM.
    pub fn merge_broker_event(&self, project: Option<&str>, event: &mut Event) {
        let other = |path: &str| project.map_or_else(|| checkout_id(path).is_some(), |p| p != path);
        match event {
            Event::Approvals(items) => {
                items.retain(|p| !other(&p.project_path));
                items.extend(
                    self.approvals
                        .iter()
                        .filter(|p| other(&p.project_path))
                        .cloned(),
                );
            }
            Event::Questions(items) => {
                items.retain(|p| !other(&p.project_path));
                items.extend(
                    self.questions
                        .iter()
                        .filter(|p| other(&p.project_path))
                        .cloned(),
                );
            }
            _ => {}
        }
    }

    pub(super) fn forget_native_sessions(&mut self, id: &str) {
        let project = project_key(id);
        for s in self
            .sessions
            .iter_mut()
            .filter(|s| s.scope.project == project)
        {
            s.forgotten = true;
            s.queue_paused = true;
        }
        if self.active_project.as_ref() == Some(&project) {
            self.leave_conversation();
            self.active_project = None;
        }
        for event in self.native_connection_lost(&project) {
            self.apply(event);
        }
        self.remembered.remove(&project);
        self.remembered_checkouts.remove(&project);
        self.services.checkouts.remove(&project);
        self.services.reviews.retain(|(p, _), _| p != &project);
        self.services.images.retain(|(p, _), _| p != &project);
        self.choices.retain(|scope, _| scope.project != project);
        self.notices.retain(|scope, _| scope.project != project);
        self.provider_commands
            .retain(|scope, _| scope.project != project);
        self.preferences.files.remove(&project);
        self.approvals.retain(|p| p.project_path != project);
        self.questions.retain(|p| p.project_path != project);
    }
}
