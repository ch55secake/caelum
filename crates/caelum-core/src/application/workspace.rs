use std::collections::HashMap;

use caelum_protocol::{
    ApprovalDecision, ApprovalPrompt, ProjectInfo, ProviderChoices, QuestionPrompt,
};
use serde_json::Value;

use super::{
    Attachment, Command, Conversation, Event, Options, Preferences, Provider, QueuedPrompt, Reply,
    SavedSession, Scope, Services, SessionKey, Snapshot, Store, Summary, Task, TaskReply,
};

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
    pub draft: String,
    pub attachments: Vec<Attachment>,
    pub attaching: usize,
    pub queue: Vec<QueuedPrompt>,
    pub queue_paused: bool,
    pub archived: bool,
    pub archiving: bool,
    pub reading: f32,
    pub shells: Vec<caelum_protocol::OpenCodeBackgroundShell>,
    pub details: Option<caelum_protocol::SessionDetails>,
    pub side_question: Option<(String, String)>,
    pub generating: bool,
    pub disconnected: bool,
    inflight_prompt: Option<String>,
    inflight_options: Options,
    options_dirty: bool,
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
            draft: String::new(),
            attachments: Vec::new(),
            attaching: 0,
            queue: Vec::new(),
            queue_paused: false,
            archived: false,
            archiving: false,
            reading: -1.0,
            shells: Vec::new(),
            details: None,
            side_question: None,
            generating: false,
            disconnected: false,
            inflight_prompt: None,
            inflight_options: Options::default(),
            options_dirty: false,
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
        !self.working()
            && self.loaded
            && !self.loading
            && !self.disconnected
            && !self.stopping
            && !self.archived
            && !self.archiving
            && self.attaching == 0
    }
}

enum Pending {
    Generate(usize, String),
    Notify,
    Load,
    Save(Snapshot, Vec<(u64, Command)>),
    Task(Task),
    Commands(Scope),
    Shells(usize),
    Archive(usize, bool),
    Attachment(usize),
    Details(usize),
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
    pub project_roots: std::collections::BTreeMap<String, String>,
    pub sessions: Vec<Session>,
    pub selected: Option<usize>,
    pub active_project: Option<String>,
    pub choices: HashMap<Scope, ProviderChoices>,
    pub notices: HashMap<Scope, String>,
    pub approvals: Vec<ApprovalPrompt>,
    pub questions: Vec<QuestionPrompt>,
    pub error: Option<String>,
    pub opening: bool,
    pub services: Services,
    pub preferences: Preferences,
    pub provider_commands: HashMap<Scope, Vec<caelum_protocol::OpenCodeCommandInfo>>,
    pub persistence_error: Option<String>,
    pub last_archived: Option<usize>,
    store: Option<Store>,
    store_ready: bool,
    saving: bool,
    saved: Option<Snapshot>,
    durable_outgoing: Vec<(u64, Command)>,
    ready_outgoing: Vec<(u64, Command)>,
    detail_queue: Vec<usize>,
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
        let Some(store) = self.store.clone() else {
            return std::mem::take(&mut self.outgoing);
        };
        let mut immediate = std::mem::take(&mut self.ready_outgoing);
        for request in std::mem::take(&mut self.outgoing) {
            if matches!(
                request.1,
                Command::Load(_) | Command::Cancel(_) | Command::Approve(..) | Command::Answer(..)
            ) {
                immediate.push(request);
            } else {
                self.durable_outgoing.push(request);
            }
        }
        if !self.store_ready || self.saving || self.persistence_error.is_some() {
            return immediate;
        }
        let snapshot = self.snapshot();
        if self.saved.as_ref() != Some(&snapshot) {
            self.saving = true;
            let commands = std::mem::take(&mut self.durable_outgoing);
            self.request(
                Command::Save(store, snapshot.clone()),
                Pending::Save(snapshot, commands),
            );
            immediate.append(&mut self.outgoing);
        } else {
            immediate.append(&mut self.durable_outgoing);
        }
        immediate
    }

    pub fn enable_store(&mut self, path: std::path::PathBuf) {
        let store = Store(path);
        self.store = Some(store.clone());
        self.request(Command::Load(store), Pending::Load);
    }

    pub fn retry_save(&mut self) {
        if self.store_ready {
            self.persistence_error = None;
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: 1,
            projects: self.projects.clone(),
            selected: self.selected,
            preferences: self.preferences.clone(),
            remembered: self
                .remembered
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            project_roots: self.project_roots.clone(),
            sessions: self
                .sessions
                .iter()
                .map(|s| SavedSession {
                    scope: s.scope.clone(),
                    remote_id: s.remote_id.clone(),
                    title: s.title.clone(),
                    options: s.options.clone(),
                    draft: s.draft.clone(),
                    attachments: s.attachments.clone(),
                    failed_prompt: s.failed_prompt.clone(),
                    pending_submission: s.inflight_prompt.as_ref().map(|text| QueuedPrompt {
                        text: text.clone(),
                        options: s.inflight_options.clone(),
                    }),
                    queue: s.queue.clone(),
                    archived: s.archived,
                    reading: s.reading,
                })
                .collect(),
        }
    }

    /// Called after background workers have stopped, so an older write cannot
    /// replace the final editor snapshot during normal window shutdown.
    pub fn flush(&self) -> Result<(), String> {
        if self.store_ready {
            if let Some(store) = &self.store {
                return store.save(&self.snapshot());
            }
        }
        Ok(())
    }

    pub fn task(&mut self, task: Task) {
        if let Task::RemoveWorktree { path, project } = &task {
            if self.sessions.iter().any(|s| {
                std::path::Path::new(&s.scope.project).starts_with(path)
                    && (s.working()
                        || s.attaching > 0
                        || s.generating
                        || s.archiving
                        || !s.queue.is_empty()
                        || !s.draft.is_empty()
                        || !s.attachments.is_empty())
            }) {
                self.services
                    .checkouts
                    .entry(project.clone())
                    .or_default()
                    .error = Some(
                    "Finish sessions and clear or move drafts before removing this checkout".into(),
                );
                return;
            }
        }
        if self.services.busy(&task)
            && !matches!(task, Task::Search { .. } | Task::Completions { .. })
        {
            return;
        }
        let id = self.request(Command::Task(task.clone()), Pending::Task(task.clone()));
        self.services.start(&task, id);
    }

    pub fn open_file(&mut self, project: String, path: String) {
        let checkout = self.services.checkouts.entry(project.clone()).or_default();
        if !checkout.tabs.contains(&path) {
            checkout.tabs.push(path.clone());
        }
        checkout.selected_file = Some(path.clone());
        if !checkout.files.contains_key(&path) {
            self.task(Task::File { project, path });
        }
    }

    pub fn archive(&mut self, index: usize, archived: bool) {
        let session = &mut self.sessions[index];
        if session.working() || session.archiving || !session.queue.is_empty() {
            return;
        }
        if let Some(key) = session.key() {
            session.archiving = true;
            self.request(
                Command::Archive(key, archived),
                Pending::Archive(index, archived),
            );
        } else {
            session.archived = archived;
            self.last_archived = archived.then_some(index);
            if archived && self.selected == Some(index) {
                self.new_session(self.sessions[index].scope.clone());
            } else if !archived {
                self.select(index);
            }
        }
    }

    pub fn resume_queue(&mut self, index: usize) {
        self.sessions[index].queue_paused = false;
        self.sessions[index].error = None;
        if !self.sessions[index].loaded || self.sessions[index].disconnected {
            self.refresh(index);
        }
        self.advance_queue(index);
    }

    fn advance_queue(&mut self, index: usize) {
        let session = &mut self.sessions[index];
        if !session.queue_paused
            && session.can_send()
            && !session.queue.is_empty()
            && session.error.is_none()
        {
            let queued = session.queue.remove(0);
            let options = std::mem::replace(&mut session.options, queued.options);
            let dirty = session.options_dirty;
            self.send_raw(index, queued.text);
            self.sessions[index].options = options;
            self.sessions[index].options_dirty =
                dirty || self.sessions[index].options != self.sessions[index].inflight_options;
        }
    }

    pub fn options_changed(&mut self, index: usize) {
        self.sessions[index].options_dirty = true;
    }

    pub fn attach(&mut self, index: usize, attachment: Attachment) -> bool {
        if self.sessions[index].attachments.len() >= 16 {
            self.sessions[index].error = Some("Attach at most 16 files per message".into());
            return false;
        }
        self.sessions[index].attachments.push(attachment);
        true
    }

    pub fn attach_file(&mut self, index: usize, path: String) {
        if self.sessions[index].attaching + self.sessions[index].attachments.len() >= 16 {
            self.sessions[index].error = Some("Attach at most 16 files per message".into());
            return;
        }
        self.sessions[index].attaching += 1;
        let task = Task::File {
            project: self.sessions[index].scope.project.clone(),
            path,
        };
        self.request(Command::Task(task), Pending::Attachment(index));
    }

    pub fn ask(&mut self, index: usize, prompt: String) -> bool {
        let session = &mut self.sessions[index];
        if session.scope.provider != Provider::OpenCode
            || session.generating
            || prompt.trim().is_empty()
        {
            return false;
        }
        let Some(key) = session.key() else {
            session.error = Some("Start an OpenCode session before asking a side question".into());
            return false;
        };
        session.generating = true;
        session.side_question = Some((prompt.clone(), String::new()));
        self.request(
            Command::Generate(key, prompt.clone()),
            Pending::Generate(index, prompt),
        );
        true
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
        if let Some(index) = self
            .remembered
            .get(path)
            .copied()
            .filter(|i| !self.sessions[*i].archived)
        {
            self.select(index);
        } else {
            self.new_session(Scope {
                project: path.into(),
                provider: self.preferences.default_provider,
            });
        }
    }

    pub fn group_path<'a>(&'a self, path: &'a str) -> &'a str {
        self.project_roots
            .get(path)
            .map(String::as_str)
            .unwrap_or(path)
    }

    pub fn refresh_project(&mut self) {
        if let Some(path) = self.active_project.clone() {
            self.task(Task::Git {
                project: path.clone(),
            });
            self.task(Task::Worktrees {
                project: path.clone(),
                directory: self.preferences.worktree_directory.clone(),
            });
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
                self.request(
                    Command::Choices(scope.clone()),
                    Pending::Choices(scope.clone()),
                );
                if provider == Provider::OpenCode {
                    self.request(Command::Commands(scope.clone()), Pending::Commands(scope));
                }
            }
        }
    }

    pub fn new_session(&mut self, scope: Scope) -> usize {
        let mut session = Session::new(scope.clone());
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

    pub fn select(&mut self, index: usize) {
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        self.active_project = Some(session.scope.project.clone());
        self.remembered.insert(session.scope.project.clone(), index);
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

    pub fn send(&mut self, index: usize, prompt: String) -> bool {
        if prompt.trim().is_empty() && self.sessions[index].attachments.is_empty() {
            return false;
        }
        let session = &mut self.sessions[index];
        if session.archived || session.stopping || session.archiving || session.attaching > 0 {
            return false;
        }
        let prompt = super::composer::with_context(&prompt, &session.attachments);
        if session.working() || session.loading {
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

    fn send_raw(&mut self, index: usize, prompt: String) -> bool {
        let session = &mut self.sessions[index];
        if prompt.trim().is_empty() || !session.can_send() {
            return false;
        }
        session.submitting = true;
        session.error = None;
        session.inflight_prompt = Some(prompt.clone());
        session.inflight_options = session.options.clone();
        session.options_dirty = false;
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
            let checked = super::forms::validate(&prompt, answer);
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

    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Branch(update) => self.branch_update(update),
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
                    for index in 0..self.sessions.len() {
                        self.advance_queue(index);
                    }
                    self.schedule_details();
                }
            }
            Event::Approvals(prompts) => {
                if self.preferences.notifications
                    && prompts.iter().any(|p| {
                        !self.approvals.contains(p)
                            && !self.visible_request(
                                &p.provider,
                                &p.project_path,
                                p.session_id.as_deref(),
                            )
                    })
                {
                    self.request(
                        Command::Notify(
                            "Caelum · Approval needed".into(),
                            "A session needs your permission".into(),
                        ),
                        Pending::Notify,
                    );
                }
                self.approvals = prompts;
            }
            Event::Questions(prompts) => {
                if self.preferences.notifications
                    && prompts.iter().any(|p| {
                        !self.questions.contains(p)
                            && !self.visible_request(
                                &p.provider,
                                &p.project_path,
                                Some(&p.session_id),
                            )
                    })
                {
                    self.request(
                        Command::Notify(
                            "Caelum · Question".into(),
                            "A session needs your answer".into(),
                        ),
                        Pending::Notify,
                    );
                }
                self.questions = prompts;
            }
            Event::Codex(event) => self.provider_event(
                &event.project_path,
                Provider::Codex,
                &event.method,
                &event.params,
            ),
            Event::OpenCode(mut event) => {
                if let Some(data) = event.data.as_object_mut() {
                    if let Some(id) = event.id {
                        data.insert("_event_id".into(), id.into());
                    }
                    if let Some(created) = event.created {
                        data.insert("_created".into(), created.into());
                    }
                }
                self.provider_event(
                    &event.project_path,
                    Provider::OpenCode,
                    &event.event_type,
                    &event.data,
                )
            }
        }
    }

    fn complete(&mut self, request: u64, pending: Pending, result: Result<Reply, String>) {
        if let Pending::Task(task) = pending {
            if !self.services.current(request, &task) {
                self.services
                    .complete(request, &task, Err("Superseded request".into()));
                return;
            }
            let result = result.and_then(|r| {
                if let Reply::Task(r) = r {
                    Ok(r)
                } else {
                    Err("Unexpected workspace response".into())
                }
            });
            if let Ok(TaskReply::Preferences(settings)) = &result {
                self.preferences = settings.clone();
            }
            if let Ok(TaskReply::Git(_, updates)) = &result {
                for update in updates {
                    self.branch_update(update.clone());
                }
            }
            if let Ok(TaskReply::Worktree(worktree)) = &result {
                if matches!(task, Task::CreateWorktree { .. }) {
                    self.open_project(worktree.project.path.clone());
                }
                if matches!(task, Task::RemoveWorktree { .. }) {
                    let root = self.project_roots.get(&worktree.project.path).cloned();
                    self.projects.retain(|p| p.path != worktree.project.path);
                    for session in &mut self.sessions {
                        if session.scope.project == worktree.project.path {
                            session.archived = true;
                        }
                    }
                    self.remembered
                        .retain(|_, i| self.sessions[*i].scope.project != worktree.project.path);
                    self.services.checkouts.remove(&worktree.project.path);
                    if self.active_project.as_ref() == Some(&worktree.project.path) {
                        let destination = root
                            .filter(|root| self.projects.iter().any(|p| &p.path == root))
                            .or_else(|| self.projects.first().map(|p| p.path.clone()));
                        self.active_project = None;
                        self.selected = None;
                        if let Some(path) = destination {
                            self.select_project(&path);
                        }
                    }
                }
            }
            if let Ok(TaskReply::Worktrees(Some(listing))) = &result {
                let root = listing
                    .worktrees
                    .iter()
                    .find(|tree| tree.main)
                    .map(|tree| tree.project.path.clone())
                    .unwrap_or_else(|| listing.project.path.clone());
                for worktree in &listing.worktrees {
                    self.project_roots
                        .insert(worktree.project.path.clone(), root.clone());
                    if worktree.available
                        && !self
                            .projects
                            .iter()
                            .any(|p| p.path == worktree.project.path)
                    {
                        self.projects.push(worktree.project.clone());
                        for provider in Provider::ALL {
                            let scope = Scope {
                                project: worktree.project.path.clone(),
                                provider,
                            };
                            self.request(Command::List(scope.clone()), Pending::List(scope));
                        }
                    }
                    if let Some(branch) = &worktree.branch {
                        self.task(Task::Discover {
                            project: worktree.project.path.clone(),
                            branch: branch.clone(),
                        });
                    }
                }
            }
            for mut next in self.services.complete(request, &task, result) {
                if matches!(task, Task::RemoveWorktree { .. }) {
                    if let Task::Worktrees { project, directory } = &mut next {
                        if !self.projects.iter().any(|p| p.path == *project) {
                            if let Some(root) = self
                                .project_roots
                                .get(project)
                                .or(self.active_project.as_ref())
                            {
                                *project = root.clone();
                            }
                        }
                        directory.clone_from(&self.preferences.worktree_directory);
                    }
                }
                self.task(next);
            }
            return;
        }
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                match pending {
                    Pending::Generate(index, prompt) => {
                        self.sessions[index].generating = false;
                        self.sessions[index].side_question =
                            Some((prompt, format!("Request failed: {error}")));
                    }
                    Pending::Notify => {}
                    Pending::Load => self.persistence_error = Some(error),
                    Pending::Save(_, mut commands) => {
                        self.saving = false;
                        commands.append(&mut self.durable_outgoing);
                        self.durable_outgoing = commands;
                        self.persistence_error = Some(error);
                    }
                    Pending::Task(_) => unreachable!(),
                    Pending::Commands(scope) => {
                        self.notices.insert(scope, error);
                    }
                    Pending::Shells(index) => self.sessions[index].error = Some(error),
                    Pending::Archive(index, _) => {
                        self.sessions[index].archiving = false;
                        self.sessions[index].error = Some(error);
                    }
                    Pending::Attachment(index) => {
                        self.sessions[index].attaching =
                            self.sessions[index].attaching.saturating_sub(1);
                        self.sessions[index].error = Some(error);
                    }
                    Pending::Details(_) => {}
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
                        session.inflight_prompt = None;
                        session.queue_paused = true;
                        session.submitting = false;
                        session.stopping = false;
                        session.conversation.working = false;
                        session.conversation.remove_optimistic();
                        session.error = Some(error);
                        session.failed_prompt = Some(prompt);
                    }
                    Pending::Send(index, prompt, revision) => {
                        let session = &mut self.sessions[index];
                        session.inflight_prompt = None;
                        session.queue_paused = true;
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
            (Pending::Generate(index, prompt), Reply::Generated(text)) => {
                self.sessions[index].generating = false;
                self.sessions[index].side_question = Some((prompt, text));
            }
            (Pending::Notify, Reply::Done) => {}
            (Pending::Details(index), Reply::Details(details)) => {
                let project = self.sessions[index].scope.project.clone();
                for branch in &details.branches {
                    self.task(Task::Discover {
                        project: project.clone(),
                        branch: branch.clone(),
                    });
                }
                self.sessions[index].details = Some(details);
            }
            (Pending::Attachment(index), Reply::Task(TaskReply::File(file))) => {
                self.sessions[index].attaching = self.sessions[index].attaching.saturating_sub(1);
                self.attach(
                    index,
                    Attachment::new(file.path, file.content, None, file.truncated),
                );
            }
            (Pending::Load, Reply::Snapshot(snapshot)) => {
                self.store_ready = true;
                if let Some(snapshot) = snapshot {
                    self.saved = Some(snapshot.clone());
                    self.preferences = snapshot.preferences;
                    self.project_roots = snapshot.project_roots;
                    if self.sessions.is_empty() {
                        self.projects = snapshot.projects;
                        self.sessions = snapshot
                            .sessions
                            .into_iter()
                            .map(|saved| {
                                let mut session = Session::new(saved.scope);
                                session.loaded = saved.remote_id.is_none();
                                session.remote_id = saved.remote_id;
                                session.title = saved.title;
                                session.options = saved.options;
                                session.options_dirty = true;
                                session.draft = saved.draft;
                                session.attachments = saved.attachments;
                                session.failed_prompt = saved.failed_prompt;
                                session.queue = saved.queue;
                                if let Some(pending) = saved.pending_submission {
                                    session.queue.insert(0, pending);
                                }
                                session.queue_paused = true;
                                session.archived = saved.archived;
                                session.reading = saved.reading;
                                session
                            })
                            .collect();
                        for (i, s) in self
                            .sessions
                            .iter()
                            .enumerate()
                            .filter(|(_, s)| !s.archived)
                        {
                            self.remembered.insert(s.scope.project.clone(), i);
                        }
                        self.remembered
                            .extend(snapshot.remembered.into_iter().filter(|(_, i)| {
                                *i < self.sessions.len() && !self.sessions[*i].archived
                            }));
                        self.detail_queue.extend(
                            self.sessions
                                .iter()
                                .enumerate()
                                .filter_map(|(i, s)| s.remote_id.as_ref().map(|_| i)),
                        );
                        if let Some(index) = snapshot.selected.filter(|i| *i < self.sessions.len())
                        {
                            self.select(index);
                            self.refresh_project();
                        }
                    }
                }
            }
            (Pending::Save(snapshot, mut commands), Reply::Done) => {
                self.saved = Some(snapshot);
                self.saving = false;
                self.ready_outgoing.append(&mut commands);
            }
            (Pending::Commands(scope), Reply::Commands(commands)) => {
                self.provider_commands.insert(scope, commands);
            }
            (Pending::Shells(index), Reply::Shells(shells)) => self.sessions[index].shells = shells,
            (Pending::Archive(index, archived), Reply::Done) => {
                self.sessions[index].archiving = false;
                self.sessions[index].archived = archived;
                if archived {
                    self.last_archived = Some(index);
                    if self.selected == Some(index) {
                        self.new_session(self.sessions[index].scope.clone());
                    }
                } else {
                    self.last_archived = None;
                    self.select(index);
                }
            }
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
                if key.scope.provider == Provider::OpenCode {
                    self.request(Command::Shells(key.clone()), Pending::Shells(index));
                }
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
                    if page.model.is_some() && !session.options_dirty {
                        session.options.model = page.model;
                        session.options.effort = page.effort;
                    }
                    if page.agent.is_some() && !session.options_dirty {
                        session.options.agent = page.agent;
                    }
                }
                if older || !session.loaded {
                    session.next_cursor = page.next_cursor;
                }
                session.loaded = true;
                session.loading = false;
                session.disconnected = false;
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
                    session.inflight_prompt = None;
                } else {
                    self.send_existing(index, prompt);
                }
            }
            (Pending::Send(index, _, _), Reply::Done) => {
                let session = &mut self.sessions[index];
                session.inflight_prompt = None;
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
                self.detail_queue.push(self.sessions.len() - 1);
            }
        }
        self.schedule_details();
    }

    fn schedule_details(&mut self) {
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

    fn branch_update(&mut self, update: caelum_protocol::SessionBranchUpdate) {
        if let Some(session) = self.sessions.iter_mut().find(|s| {
            s.scope.project == update.project_path
                && s.scope.provider.as_str() == update.provider
                && s.remote_id.as_ref() == Some(&update.session_id)
        }) {
            session.details = Some(caelum_protocol::SessionDetails {
                branch: update.branch,
                branches: update.branches.clone(),
                branch_is_current: false,
                changes: None,
            });
        }
        for branch in update.branches {
            self.task(Task::Discover {
                project: update.project_path.clone(),
                branch,
            });
        }
    }

    fn visible_request(&self, provider: &str, project: &str, id: Option<&str>) -> bool {
        self.selected.is_some_and(|i| {
            let session = &self.sessions[i];
            session.scope.project == project
                && session.scope.provider.as_str() == provider
                && id.map_or(true, |id| session.remote_id.as_deref() == Some(id))
        })
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
                let recover =
                    !session.disconnected && (session.working() || self.selected == Some(index));
                session.revision += 1;
                session.conversation.working = false;
                session.queue_paused = true;
                session.disconnected = true;
                session.error = Some(
                    "Provider disconnected. Refresh this session to reconnect and recover history."
                        .into(),
                );
                if recover && !session.submitting && !session.loading {
                    completed.push(index);
                }
                continue;
            }
            if id.is_none() || session.remote_id.as_deref() != id {
                continue;
            }
            let was_working = session.working();
            session.revision += 1;
            session.conversation.apply(provider, method, data);
            if matches!(
                method,
                "session.execution.failed" | "session.execution.interrupted"
            ) || (method == "turn/completed" && !data["turn"]["error"].is_null())
            {
                session.queue_paused = true;
            }
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
            if self.preferences.notifications
                && self.selected != Some(index)
                && !self.sessions[index].disconnected
            {
                self.request(
                    Command::Notify(
                        "Caelum · Session finished".into(),
                        self.sessions[index].title.clone(),
                    ),
                    Pending::Notify,
                );
            }
            self.refresh(index);
        }
    }
}

fn update_summary(session: &mut Session, summary: Summary) {
    if !summary.title.trim().is_empty() {
        session.title = summary.title;
    }
    if summary.options.model.is_some() && !session.options_dirty {
        session.options.model = summary.options.model;
        session.options.effort = summary.options.effort;
    }
    if summary.options.agent.is_some() && !session.options_dirty {
        session.options.agent = summary.options.agent;
    }
}
