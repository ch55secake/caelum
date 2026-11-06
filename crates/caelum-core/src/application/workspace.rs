use std::collections::HashMap;

use caelum_protocol::{
    ApprovalDecision, ApprovalPrompt, ProjectInfo, ProviderChoices, QuestionPrompt,
};
use serde_json::Value;

use super::{Command, Conversation, Event, Options, Provider, Reply, Scope, SessionKey, Summary};

pub struct Session {
    pub scope: Scope,
    pub remote_id: Option<String>,
    pub title: String,
    pub conversation: Conversation,
    pub options: Options,
    pub loading: bool,
    pub loaded: bool,
    pub submitting: bool,
    pub stopping: bool,
    pub next_cursor: Option<String>,
    pub error: Option<String>,
    /// A failed or cancelled submission stays available without replacing a
    /// newer editor draft. The UI can explicitly restore/copy it.
    pub failed_prompt: Option<String>,
    revision: u64,
}

impl Session {
    fn new(scope: Scope) -> Self {
        Self {
            scope,
            remote_id: None,
            title: "New session".into(),
            conversation: Conversation::default(),
            options: Options::default(),
            loading: false,
            loaded: true,
            submitting: false,
            stopping: false,
            next_cursor: None,
            error: None,
            failed_prompt: None,
            revision: 0,
        }
    }

    pub fn key(&self) -> Option<SessionKey> {
        self.remote_id.as_ref().map(|id| SessionKey {
            scope: self.scope.clone(),
            id: id.clone(),
        })
    }

    pub fn working(&self) -> bool {
        self.submitting || self.conversation.working
    }

    pub fn can_send(&self) -> bool {
        !self.working() && !self.loading && !self.stopping
    }
}

enum Pending {
    Open,
    List(Scope),
    Choices(Scope),
    Resume(usize, u64),
    History(usize, u64, bool),
    Start(usize, String),
    Send(usize, String, u64),
    Cancel(usize),
    Request {
        provider: String,
        project: String,
        session: Option<String>,
    },
}

/// View-independent workspace coordinator. Stable local session indices survive
/// first-send creation; every pending request carries its original owner.
#[derive(Default)]
pub struct Workspace {
    pub projects: Vec<ProjectInfo>,
    pub sessions: Vec<Session>,
    pub selected: Option<usize>,
    pub active_project: Option<String>,
    pub choices: HashMap<Scope, ProviderChoices>,
    pub notices: HashMap<Scope, String>,
    pub approvals: Vec<ApprovalPrompt>,
    pub questions: Vec<QuestionPrompt>,
    pub error: Option<String>,
    pub opening: bool,
    pending: HashMap<u64, Pending>,
    outgoing: Vec<(u64, Command)>,
    next_request: u64,
    latest_open: u64,
    remembered: HashMap<String, usize>,
    deferred_lists: HashMap<Scope, Vec<Summary>>,
}

impl Workspace {
    fn request(&mut self, command: Command, pending: Pending) -> u64 {
        self.next_request += 1;
        let id = self.next_request;
        self.pending.insert(id, pending);
        self.outgoing.push((id, command));
        id
    }

    pub fn commands(&mut self) -> Vec<(u64, Command)> {
        std::mem::take(&mut self.outgoing)
    }

    pub fn open_project(&mut self, path: String) {
        self.opening = true;
        self.error = None;
        self.latest_open = self.request(Command::Open(path), Pending::Open);
    }

    pub fn select_project(&mut self, path: &str) {
        if !self.projects.iter().any(|project| project.path == path) {
            return;
        }
        self.active_project = Some(path.into());
        if let Some(index) = self.remembered.get(path).copied() {
            self.select(index);
        } else {
            self.new_session(Scope {
                project: path.into(),
                provider: Provider::Codex,
            });
        }
    }

    pub fn refresh_project(&mut self) {
        if let Some(path) = self.active_project.clone() {
            for provider in Provider::ALL {
                let scope = Scope {
                    project: path.clone(),
                    provider,
                };
                // A repeated refresh does not accumulate duplicate network calls.
                if self
                    .pending
                    .values()
                    .any(|pending| matches!(pending, Pending::List(current) if *current == scope))
                {
                    continue;
                }
                self.notices.remove(&scope);
                self.request(Command::List(scope.clone()), Pending::List(scope.clone()));
                self.request(Command::Choices(scope.clone()), Pending::Choices(scope));
            }
        }
    }

    pub fn new_session(&mut self, scope: Scope) -> usize {
        let mut session = Session::new(scope.clone());
        if let Some(choices) = self.choices.get(&scope) {
            session.options.model = choices.default_model.clone();
        }
        let index = self.sessions.len();
        self.sessions.push(session);
        self.select(index);
        index
    }

    pub fn select(&mut self, index: usize) {
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        self.active_project = Some(session.scope.project.clone());
        self.remembered.insert(session.scope.project.clone(), index);
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

    pub fn send(&mut self, index: usize, prompt: String) -> bool {
        let session = &mut self.sessions[index];
        if prompt.trim().is_empty() || !session.can_send() {
            return false;
        }
        session.submitting = true;
        session.error = None;
        session.failed_prompt = None;
        session.revision += 1;
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

    fn send_existing(&mut self, index: usize, prompt: String) {
        let session = &self.sessions[index];
        let key = session.key().expect("created session");
        self.request(
            Command::Send(key, prompt.clone(), session.options.clone()),
            Pending::Send(index, prompt, session.revision),
        );
    }

    pub fn cancel(&mut self, index: usize) {
        let session = &mut self.sessions[index];
        if !session.working() || session.stopping {
            return;
        }
        session.stopping = true;
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
        let owner = Pending::Request {
            provider: prompt.provider.clone(),
            project: prompt.project_path.clone(),
            session: Some(prompt.session_id.clone()),
        };
        self.request(Command::Answer(prompt, answer), owner);
    }

    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Completed(id, result) => {
                if let Some(pending) = self.pending.remove(&id) {
                    self.complete(id, pending, result);
                    let ready: Vec<_> = self
                        .deferred_lists
                        .keys()
                        .filter(|scope| !self.creating(scope))
                        .cloned()
                        .collect();
                    for scope in ready {
                        if let Some(summaries) = self.deferred_lists.remove(&scope) {
                            self.apply_summaries(&scope, summaries);
                        }
                    }
                }
            }
            Event::Approvals(prompts) => self.approvals = prompts,
            Event::Questions(prompts) => self.questions = prompts,
            Event::Codex(event) => self.provider_event(
                &event.project_path,
                Provider::Codex,
                &event.method,
                &event.params,
            ),
            Event::OpenCode(event) => self.provider_event(
                &event.project_path,
                Provider::OpenCode,
                &event.event_type,
                &event.data,
            ),
        }
    }

    fn complete(&mut self, request: u64, pending: Pending, result: Result<Reply, String>) {
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                match pending {
                    Pending::Open => {
                        if request == self.latest_open {
                            self.opening = false;
                            self.error = Some(error);
                        }
                    }
                    Pending::List(scope) | Pending::Choices(scope) => {
                        self.notices.insert(scope, error);
                    }
                    Pending::Resume(index, _) | Pending::History(index, _, _) => {
                        self.sessions[index].loading = false;
                        self.sessions[index].error = Some(error);
                    }
                    Pending::Start(index, prompt) => {
                        let session = &mut self.sessions[index];
                        session.submitting = false;
                        session.stopping = false;
                        session.conversation.working = false;
                        session.conversation.remove_optimistic();
                        session.error = Some(error);
                        session.failed_prompt = Some(prompt);
                    }
                    Pending::Send(index, prompt, revision) => {
                        let session = &mut self.sessions[index];
                        session.submitting = false;
                        session.stopping = false;
                        if session.revision == revision {
                            session.conversation.working = false;
                        }
                        if session
                            .conversation
                            .entries
                            .iter()
                            .any(|entry| entry.optimistic)
                        {
                            session.conversation.remove_optimistic();
                            session.failed_prompt = Some(prompt);
                        }
                        session.error = Some(error);
                    }
                    Pending::Cancel(index) => {
                        self.sessions[index].stopping = false;
                        self.sessions[index].error = Some(error);
                    }
                    Pending::Request {
                        provider,
                        project,
                        session,
                    } => {
                        for target in self.sessions.iter_mut().filter(|target| {
                            target.scope.project == project
                                && target.scope.provider.as_str() == provider
                                && session
                                    .as_ref()
                                    .map_or(true, |id| target.remote_id.as_ref() == Some(id))
                        }) {
                            target.error = Some(error.clone());
                        }
                        if let Some(provider) =
                            Provider::ALL.into_iter().find(|p| p.as_str() == provider)
                        {
                            self.notices.insert(Scope { project, provider }, error);
                        }
                    }
                }
                return;
            }
        };
        match (pending, reply) {
            (Pending::Open, Reply::Project(project)) => {
                if !self.projects.iter().any(|p| p.path == project.path) {
                    self.projects.push(project.clone());
                }
                if request == self.latest_open {
                    self.opening = false;
                    self.select_project(&project.path);
                    self.refresh_project();
                }
            }
            (Pending::List(scope), Reply::Sessions(summaries)) => {
                // Discovery can see a newly created remote session before the
                // creation reply supplies its ID. Adopt it into its stable local
                // slot first, then reconcile discovery without duplicate rows.
                if self.creating(&scope) {
                    self.deferred_lists.insert(scope, summaries);
                } else {
                    self.apply_summaries(&scope, summaries);
                }
            }
            (Pending::Choices(scope), Reply::Choices(choices)) => {
                for session in self
                    .sessions
                    .iter_mut()
                    .filter(|s| s.scope == scope && s.remote_id.is_none() && !s.submitting)
                {
                    if session.options.model.is_none() {
                        session.options.model = choices.default_model.clone();
                    }
                }
                self.choices.insert(scope, choices);
            }
            (Pending::Resume(index, revision), Reply::Session(summary)) => {
                let session = &mut self.sessions[index];
                if session.key().as_ref() != Some(&summary.key) {
                    session.loading = false;
                    session.error = Some("Provider returned a different session".into());
                    return;
                }
                if session.revision == revision {
                    session.conversation.working = summary.working;
                }
                update_summary(session, summary);
                let key = session.key().expect("resumed session");
                self.request(
                    Command::History(key, None),
                    Pending::History(index, revision, false),
                );
            }
            (Pending::History(index, revision, older), Reply::History(page)) => {
                let session = &mut self.sessions[index];
                session.conversation.history(
                    session.scope.provider,
                    &page.data,
                    older || !session.loaded,
                );
                if !older && session.revision == revision {
                    if let Some(working) = page.is_working {
                        session.conversation.working = working;
                    }
                    if page.model.is_some() {
                        session.options.model = page.model;
                        session.options.effort = page.effort;
                    }
                    if page.agent.is_some() {
                        session.options.agent = page.agent;
                    }
                }
                if older || !session.loaded {
                    session.next_cursor = page.next_cursor;
                }
                session.loaded = true;
                session.loading = false;
            }
            (Pending::Start(index, prompt), Reply::Session(summary)) => {
                let session = &mut self.sessions[index];
                if session.scope != summary.key.scope {
                    session.submitting = false;
                    session.conversation.working = false;
                    session.failed_prompt = Some(prompt);
                    session.error =
                        Some("Provider created a session in a different project".into());
                    return;
                }
                session.remote_id = Some(summary.key.id);
                if session.stopping {
                    session.stopping = false;
                    session.submitting = false;
                    session.conversation.settle();
                    session.conversation.remove_optimistic();
                    session.failed_prompt = Some(prompt);
                } else {
                    self.send_existing(index, prompt);
                }
            }
            (Pending::Send(index, _, _), Reply::Done) => {
                let session = &mut self.sessions[index];
                session.submitting = false;
                if session.stopping {
                    let key = session.key().expect("submitted session");
                    self.request(Command::Cancel(key), Pending::Cancel(index));
                } else if !session.working() {
                    self.refresh(index);
                }
            }
            (Pending::Cancel(index), Reply::Done) => {
                self.sessions[index].revision += 1;
                self.sessions[index].stopping = false;
                self.sessions[index].conversation.settle();
                self.refresh(index);
            }
            (Pending::Request { .. }, Reply::Done) => {}
            _ => self.error = Some("Unexpected application response".into()),
        }
    }

    fn creating(&self, scope: &Scope) -> bool {
        self.pending.values().any(|pending| matches!(pending, Pending::Start(index, _) if self.sessions[*index].scope == *scope))
    }

    fn apply_summaries(&mut self, scope: &Scope, summaries: Vec<Summary>) {
        for summary in summaries
            .into_iter()
            .filter(|summary| summary.key.scope == *scope)
        {
            if let Some(session) = self
                .sessions
                .iter_mut()
                .find(|session| session.key().as_ref() == Some(&summary.key))
            {
                if session.revision == 0 {
                    session.conversation.working = summary.working;
                    update_summary(session, summary);
                }
            } else {
                let mut session = Session::new(scope.clone());
                session.remote_id = Some(summary.key.id.clone());
                session.loaded = false;
                session.conversation.working = summary.working;
                update_summary(&mut session, summary);
                self.sessions.push(session);
            }
        }
    }

    fn provider_event(&mut self, path: &str, provider: Provider, method: &str, data: &Value) {
        let disconnected = matches!(method, "codex/processExited" | "server.disconnected");
        let id = match provider {
            Provider::Codex => data["threadId"].as_str().or(data["thread"]["id"].as_str()),
            Provider::OpenCode => data["sessionID"]
                .as_str()
                .or(data["session"]["id"].as_str()),
        };
        let mut completed = Vec::new();
        for (index, session) in self.sessions.iter_mut().enumerate() {
            if session.scope.project != path || session.scope.provider != provider {
                continue;
            }
            if disconnected {
                session.revision += 1;
                session.conversation.working = false;
                session.error = Some(
                    "Provider disconnected. Refresh this session to reconnect and recover history."
                        .into(),
                );
                continue;
            }
            if id.is_none() || session.remote_id.as_deref() != id {
                continue;
            }
            let was_working = session.working();
            session.revision += 1;
            session.conversation.apply(provider, method, data);
            if was_working && !session.working() && session.loaded {
                completed.push(index);
            }
            if method == "error" || method == "permission.reply.failed" {
                session.error = Some(
                    data["error"]["message"]
                        .as_str()
                        .or(data["message"].as_str())
                        .unwrap_or("Provider request failed")
                        .into(),
                );
            }
            let title = data["thread"]["name"]
                .as_str()
                .or(data["session"]["title"].as_str())
                .or_else(|| {
                    if method == "thread/name/updated" {
                        data["threadName"].as_str()
                    } else if matches!(
                        method,
                        "session.title.updated" | "session.updated" | "session.renamed"
                    ) {
                        data["title"].as_str()
                    } else {
                        None
                    }
                });
            if let Some(title) = title.filter(|title| !title.is_empty()) {
                session.title = title.into();
            }
        }
        for index in completed {
            self.refresh(index);
        }
    }
}

fn update_summary(session: &mut Session, summary: Summary) {
    if !summary.title.trim().is_empty() {
        session.title = summary.title;
    }
    if summary.options.model.is_some() {
        session.options.model = summary.options.model;
        session.options.effort = summary.options.effort;
    }
    if summary.options.agent.is_some() {
        session.options.agent = summary.options.agent;
    }
}
