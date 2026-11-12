use super::{Pending, Workspace};
use crate::application::{Command, Provider, QueuedPrompt};
use caelum_protocol::{ApprovalDecision, ApprovalPrompt, QuestionPrompt};

impl Workspace {
    /// Ordinary messages wait behind active work, history/inbox reads and earlier prompts.
    pub fn will_queue(&self, index: usize) -> bool {
        let session = &self.sessions[index];
        session.working()
            || session.loading
            || !session.queue.is_empty()
            || !session.inbox.items.is_empty()
            || self
                .pending
                .values()
                .any(|p| matches!(p, Pending::Inbox(i, _) if *i == index))
    }

    pub fn send(&mut self, index: usize, prompt: String) -> bool {
        if prompt.trim().is_empty() && self.sessions[index].attachments.is_empty() {
            return false;
        }
        let queued = self.will_queue(index);
        let session = &mut self.sessions[index];
        if !session.can_submit() {
            return false;
        }
        let native_command = prompt
            .trim()
            .strip_prefix('/')
            .filter(|_| !prompt.trim().starts_with("//"));
        if let Some(command) = native_command {
            let name = command.split_whitespace().next().unwrap_or_default();
            if !self
                .provider_commands
                .get(&session.scope)
                .is_some_and(|commands| commands.iter().any(|c| c.name == name))
            {
                session.error =
                    Some(
                        if self.pending.values().any(
                            |p| matches!(p, Pending::Commands(scope) if scope == &session.scope),
                        ) {
                            "Commands are loading. Try again when discovery finishes.".into()
                        } else {
                            format!("Unknown command /{name}")
                        },
                    );
                return false;
            }
        }
        let prompt = crate::application::composer::with_context(&prompt, &session.attachments);
        if let Some(key) = session.key().filter(|_| native_command.is_some()) {
            session.commanding = true;
            session.error = None;
            session.attachments.clear();
            session.draft.clear();
            session.options_dirty = false;
            let command = Command::Send(key, prompt.clone(), session.options.clone());
            self.request(command, Pending::Control(index, prompt));
            return true;
        }
        if session.working()
            && session.scope.provider == Provider::OpenCode
            && !session.options_dirty
            && session.queue.is_empty()
            && !session.queue_paused
        {
            if let Some(key) = session.key() {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let id = format!("msg_caelum_{stamp}_{}", self.next_request);
                session.inbox.items.push(crate::application::InboxPrompt {
                    id: id.clone(),
                    text: prompt.clone(),
                    pending: true,
                    removing: false,
                    error: None,
                });
                session.inbox.revision += 1;
                session.attachments.clear();
                session.draft.clear();
                self.request(
                    Command::Enqueue(key, id.clone(), prompt),
                    Pending::Enqueue(index, id),
                );
                return true;
            }
        }
        if queued {
            session.queue.push(QueuedPrompt {
                text: prompt,
                options: session.options.clone(),
            });
            session.attachments.clear();
            session.draft.clear();
            return true;
        }
        let accepted = self.send_raw(index, prompt);
        if accepted {
            self.sessions[index].attachments.clear();
            self.sessions[index].draft.clear();
        }
        accepted
    }

    pub fn review_blocker(&self, index: usize, project: &str) -> Option<&'static str> {
        let session = self.sessions.get(index)?;
        if session.scope.project != project {
            return Some("Choose a session in this checkout");
        }
        if session.working() {
            return Some("Wait for the current turn to finish");
        }
        if self.approvals.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && (p.session_id.is_none() || p.session_id == session.remote_id)
        }) || self.questions.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && Some(&p.session_id) == session.remote_id.as_ref()
        }) {
            return Some("Answer the session’s pending request first");
        }
        if !session.can_send() || session.remote_id.is_none() {
            return Some("Connect an active session to address feedback");
        }
        if !session.queue.is_empty() || !session.inbox.items.is_empty() {
            return Some("Finish queued messages first");
        }
        None
    }

    /// Send review feedback to its captured destination, retaining the independent composer draft.
    pub fn address_review(&mut self, index: usize, project: &str, url: &str) -> bool {
        if index >= self.sessions.len() || self.review_blocker(index, project).is_some() {
            return false;
        }
        let Some(prompt) = self
            .services
            .reviews
            .get(&(project.into(), url.into()))
            .and_then(|r| r.address_prompt())
        else {
            return false;
        };
        self.send_raw(index, prompt)
    }

    pub(super) fn send_raw(&mut self, index: usize, prompt: String) -> bool {
        let session = &mut self.sessions[index];
        if prompt.trim().is_empty() || !session.can_send() {
            return false;
        }
        session.submitting = true;
        session.error = None;
        session.inflight_prompt = Some(prompt.clone());
        session.inflight_options = session.options.clone();
        session.inflight_queued = false;
        session.options_dirty = false;
        session.revision += 1;
        session.conversation.begin_turn(
            session.inflight_options.agent.clone(),
            crate::application::presentation::now(),
        );
        session
            .conversation
            .optimistic(format!("local-{}", self.next_request + 1), prompt.clone());
        if session.remote_id.is_none() {
            session.title = prompt
                .lines()
                .next()
                .unwrap_or(&prompt)
                .chars()
                .take(60)
                .collect();
            let command = Command::Start(session.scope.clone(), session.options.clone());
            self.request(command, Pending::Start(index, prompt));
        } else {
            self.send_existing(index, prompt);
        }
        true
    }

    pub(super) fn send_existing(&mut self, index: usize, prompt: String) {
        let session = &self.sessions[index];
        let key = session.key().expect("created session");
        self.request(
            Command::Send(key, prompt.clone(), session.inflight_options.clone()),
            Pending::Send(index, prompt, session.revision),
        );
    }

    pub fn cancel(&mut self, index: usize) {
        let session = &mut self.sessions[index];
        if !session.working() || session.stopping {
            return;
        }
        session.stopping = true;
        session.queue_paused = true;
        session.revision += 1;
        // First-send startup/admission can be in flight. Wait for acceptance
        // before cancelling, or cancel creation before submitting the prompt.
        if !session.submitting {
            if let Some(key) = session.key() {
                self.request(Command::Cancel(key), Pending::Cancel(index));
            }
        }
    }

    pub fn approve(&mut self, prompt: ApprovalPrompt, decision: ApprovalDecision) {
        let owner = Pending::Request {
            provider: prompt.provider.clone(),
            project: prompt.project_path.clone(),
            session: prompt.session_id.clone(),
        };
        self.request(Command::Approve(prompt, decision), owner);
    }

    pub fn answer(&mut self, prompt: QuestionPrompt, answer: Option<crate::questions::Answer>) {
        if let Some(answer) = &answer {
            let checked = crate::application::forms::validate(&prompt, answer);
            if !checked.errors.is_empty() {
                return;
            }
        }
        let owner = Pending::Request {
            provider: prompt.provider.clone(),
            project: prompt.project_path.clone(),
            session: Some(prompt.session_id.clone()),
        };
        self.request(Command::Answer(prompt, answer), owner);
    }
}
