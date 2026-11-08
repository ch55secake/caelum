use std::collections::{BTreeSet, HashMap};

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
    pub commanding: bool,
    pub stopping: bool,
    pub next_cursor: Option<String>,
    pub error: Option<String>,
    /// A failed or cancelled submission stays available without replacing a
    /// newer editor draft. The UI can explicitly restore/copy it.
    pub failed_prompt: Option<String>,
    pub draft: String,
    pub attachments: Vec<Attachment>,
    pub attaching: usize,
    pub completion_caret: Option<usize>,
    pub completion_generation: u64,
    pub queue: Vec<QueuedPrompt>,
    pub inbox: super::Inbox,
    pub queue_paused: bool,
    pub archived: bool,
    /// Removed from Caelum because the provider no longer associates it with
    /// this project. Kept in memory only so pending requests retain stable indices.
    pub forgotten: bool,
    pub archiving: bool,
    pub reading: f32,
    pub unread: bool,
    pub shells: Vec<caelum_protocol::OpenCodeBackgroundShell>,
    shell_revision: u64,
    pub details: Option<caelum_protocol::SessionDetails>,
    pub side_question: Option<(String, String)>,
    pub side_draft: String,
    pub side_attachments: Vec<Attachment>,
    pub side_error: Option<String>,
    pub side_generation: u64,
    pub side_completion_caret: Option<usize>,
    pub generating: bool,
    pub disconnected: bool,
    inflight_prompt: Option<String>,
    inflight_options: Options,
    inflight_queued: bool,
    options_dirty: bool,
    revision: u64,
    seen_events: std::collections::VecDeque<String>,
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
            commanding: false,
            stopping: false,
            next_cursor: None,
            error: None,
            failed_prompt: None,
            draft: String::new(),
            attachments: Vec::new(),
            attaching: 0,
            completion_caret: None,
            completion_generation: 0,
            queue: Vec::new(),
            inbox: Default::default(),
            queue_paused: false,
            archived: false,
            forgotten: false,
            archiving: false,
            reading: -1.0,
            unread: false,
            shells: Vec::new(),
            shell_revision: 0,
            details: None,
            side_question: None,
            side_draft: String::new(),
            side_attachments: Vec::new(),
            side_error: None,
            side_generation: 0,
            side_completion_caret: None,
            generating: false,
            disconnected: false,
            inflight_prompt: None,
            inflight_options: Options::default(),
            inflight_queued: false,
            options_dirty: false,
            revision: 0,
            seen_events: Default::default(),
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
            && !self.forgotten
            && !self.archiving
            && self.attaching == 0
    }
}

enum Pending {
    Inbox(usize, u64),
    Enqueue(usize, String),
    RemoveQueued(usize, String),
    Control(usize, String),
    Generate(usize, String, u64),
    Notify,
    Load,
    Save(Box<Snapshot>, Vec<(u64, Command)>),
    Task(Task),
    Commands(Scope),
    Shells(usize, u64),
    Archive(usize, bool, Option<(usize, u64)>, u64),
    Attachment(usize, String),
    Mention {
        index: usize,
        side: bool,
        generation: u64,
        draft: String,
        token: super::Completion,
        replacement: String,
    },
    Details(usize),
    Open(Option<Provider>, u64),
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
    pub window_background: bool,
    pub conversation_hidden: bool,
    pub projects: Vec<ProjectInfo>,
    pub project_roots: std::collections::BTreeMap<String, String>,
    pub sessions: Vec<Session>,
    pub selected: Option<usize>,
    pub active_project: Option<String>,
    pub choices: HashMap<Scope, ProviderChoices>,
    pub notices: HashMap<Scope, String>,
    dirty_checkouts: std::collections::HashSet<String>,
    refresh_after: HashMap<String, Task>,
    pub approvals: Vec<ApprovalPrompt>,
    pub questions: Vec<QuestionPrompt>,
    pub error: Option<String>,
    pub opening: bool,
    pub services: Services,
    pub preferences: Preferences,
    pub provider_commands: HashMap<Scope, Vec<caelum_protocol::OpenCodeCommandInfo>>,
    pub persistence_error: Option<String>,
    pub last_archived: Option<usize>,
    archive_clock: Option<(u64, u64)>,
    archive_revision: u64,
    archive_notice_revision: u64,
    archive_replacement: Option<usize>,
    navigation: u64,
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
    launches: HashMap<u64, (Provider, u64)>,
    remembered: HashMap<String, usize>,
    remembered_checkouts: HashMap<String, usize>,
    managed_worktrees: BTreeSet<String>,
    deferred_lists: HashMap<Scope, Vec<Summary>>,
    /// Closing only removes open navigation. Reject earlier discovery/open replies
    /// without invalidating the stable sessions or their in-flight work.
    closed_projects: HashMap<String, u64>,
}

impl Workspace {
    fn request(&mut self, command: Command, pending: Pending) -> u64 {
        if matches!(command, Command::Notify(..)) && self.preferences.sound_volume > 0 {
            self.request(
                Command::Sound(self.preferences.sound_volume),
                Pending::Notify,
            );
        }
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
                Command::Load(_)
                    | Command::Cancel(_)
                    | Command::Task(
                        Task::Shell { .. }
                            | Task::Git { .. }
                            | Task::Diff { .. }
                            | Task::File { .. }
                            | Task::Directory { .. }
                    )
                    | Command::Approve(..)
                    | Command::Answer(..)
                    | Command::Notify(..)
                    | Command::Sound(_)
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
                Command::Save(store, Box::new(snapshot.clone())),
                Pending::Save(Box::new(snapshot), commands),
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
        let mut preferences = self.preferences.clone();
        for (project, checkout) in &self.services.checkouts {
            if project.is_empty() {
                continue;
            }
            let files = preferences.files.entry(project.clone()).or_default();
            files.tabs = checkout.tabs.clone();
            files.selected = checkout.selected_file.clone();
        }
        let mut session_indices = HashMap::new();
        let sessions = self
            .sessions
            .iter()
            .enumerate()
            .filter_map(|(index, s)| {
                if s.forgotten {
                    return None;
                }
                let saved_index = session_indices.len();
                session_indices.insert(index, saved_index);
                Some(SavedSession {
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
                    inbox: s.inbox.items.clone(),
                    archived: s.archived,
                    reading: s.reading,
                    unread: s.unread,
                })
            })
            .collect();
        Snapshot {
            version: 1,
            projects: self.projects.clone(),
            selected: self
                .selected
                .and_then(|index| session_indices.get(&index).copied()),
            preferences,
            remembered: self
                .remembered
                .iter()
                .filter_map(|(key, index)| {
                    session_indices
                        .get(index)
                        .map(|index| (key.clone(), *index))
                })
                .collect(),
            project_roots: self.project_roots.clone(),
            managed_worktrees: self.managed_worktrees.clone(),
            remembered_checkouts: self
                .remembered_checkouts
                .iter()
                .filter_map(|(key, index)| {
                    session_indices
                        .get(index)
                        .map(|index| (key.clone(), *index))
                })
                .collect(),
            sessions,
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
                    && !s.forgotten
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
            && !matches!(
                task,
                Task::Search { .. } | Task::Completions { .. } | Task::Stats { .. }
            )
        {
            if matches!(
                task,
                Task::Diff { .. } | Task::File { .. } | Task::Directory { .. }
            ) {
                self.refresh_after.insert(task.slot(), task);
            }
            return;
        }
        let id = self.request(Command::Task(task.clone()), Pending::Task(task.clone()));
        if matches!(task, Task::CreateWorktree { .. }) {
            let provider = self
                .selected
                .map(|i| self.sessions[i].scope.provider)
                .unwrap_or(self.preferences.default_provider);
            self.launches.insert(id, (provider, self.navigation));
        }
        self.services.start(&task, id);
    }

    /// Retain a follow-up invalidation while Git is in flight. A tool ending or
    /// an external edit must not be lost behind an older snapshot.
    pub fn checkout_changed(&mut self, project: &str) {
        if !self.projects.iter().any(|p| p.path == project) {
            return;
        }
        let git = Task::Git {
            project: project.into(),
        };
        if self.services.busy(&git) {
            self.dirty_checkouts.insert(project.into());
            return;
        }
        self.dirty_checkouts.remove(project);
        self.task(git);
        let Some(checkout) = self.services.checkouts.get(project) else {
            return;
        };
        let mut tasks = vec![Task::Directory {
            project: project.into(),
            path: String::new(),
            offset: 0,
        }];
        for path in checkout.diffs.keys() {
            tasks.push(Task::Diff {
                project: project.into(),
                path: path.clone(),
            });
        }
        for path in &checkout.tabs {
            tasks.push(Task::File {
                project: project.into(),
                path: path.clone(),
            });
        }
        for path in checkout.directories.keys().filter(|p| !p.is_empty()) {
            tasks.push(Task::Directory {
                project: project.into(),
                path: path.clone(),
                offset: 0,
            });
        }
        for task in tasks {
            self.task(task);
        }
    }

    pub fn launch_worktree(&mut self, task: Task, provider: Provider) {
        let previous = self.next_request;
        self.task(task);
        if self.next_request != previous && self.launches.contains_key(&self.next_request) {
            self.launches
                .insert(self.next_request, (provider, self.navigation));
        }
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

    pub fn refresh_files(&mut self) {
        if let Some(project) = self.active_project.clone() {
            if let Some(path) = self
                .services
                .checkouts
                .get(&project)
                .and_then(|c| c.selected_file.clone())
            {
                self.task(Task::File {
                    project: project.clone(),
                    path,
                });
            }
            self.task(Task::Directory {
                project,
                path: String::new(),
                offset: 0,
            });
        }
    }

    pub fn archive(&mut self, index: usize, archived: bool) {
        self.set_archived(index, archived, true);
    }

    pub fn undo_archive(&mut self) {
        if let Some(index) = self.last_archived {
            let select =
                self.archive_replacement.is_some() && self.selected == self.archive_replacement;
            self.set_archived(index, false, select);
        }
    }

    /// Presentation lifetime starts at successful archival, and pauses during interaction.
    pub fn tick_archive_notice(&mut self, now: u64, engaged: bool) -> bool {
        let Some(index) = self.last_archived else {
            return false;
        };
        let Some((last, remaining)) = self.archive_clock.as_mut() else {
            return false;
        };
        if !engaged && !self.sessions[index].archiving {
            *remaining = remaining.saturating_sub(now.saturating_sub(*last));
        }
        *last = now;
        if *remaining == 0 {
            self.dismiss_archive(index);
            return true;
        }
        false
    }

    pub fn dismiss_archive(&mut self, index: usize) {
        if self.last_archived == Some(index) {
            self.last_archived = None;
            self.archive_clock = None;
        }
    }

    fn archived_notice(&mut self, index: usize, revision: u64) -> bool {
        let project = &self.sessions[index].scope.project;
        if self.closed_projects.contains_key(self.group_path(project))
            && !self.project_is_open(project)
        {
            return false;
        }
        if revision < self.archive_notice_revision {
            return false;
        }
        self.archive_notice_revision = revision;
        self.last_archived = Some(index);
        self.archive_clock = Some((super::presentation::now(), 8_000));
        self.archive_replacement = None;
        true
    }

    fn set_archived(&mut self, index: usize, archived: bool, select: bool) {
        if self.sessions[index].forgotten {
            return;
        }
        let needs_answer = self.approvals.iter().any(|p| {
            self.sessions[index].remote_id == p.session_id
                && self.sessions[index].scope.project == p.project_path
                && self.sessions[index].scope.provider.as_str() == p.provider
        }) || self.questions.iter().any(|p| {
            self.sessions[index].remote_id.as_ref() == Some(&p.session_id)
                && self.sessions[index].scope.project == p.project_path
                && self.sessions[index].scope.provider.as_str() == p.provider
        });
        let session = &mut self.sessions[index];
        if needs_answer
            || session.working()
            || session.archiving
            || !session.queue.is_empty()
            || !session.inbox.items.is_empty()
        {
            return;
        }
        session.error = None;
        self.archive_revision += 1;
        if let Some(key) = session.key() {
            session.archiving = true;
            self.request(
                Command::Archive(key, archived),
                Pending::Archive(
                    index,
                    archived,
                    select.then(|| (self.selected.unwrap_or(index), self.navigation)),
                    self.archive_revision,
                ),
            );
        } else {
            session.archived = archived;
            if archived {
                self.archived_notice(index, self.archive_revision);
            } else {
                self.dismiss_archive(index);
            }
            if archived && self.selected == Some(index) {
                self.new_session(self.sessions[index].scope.clone());
                self.archive_replacement = self.selected;
            } else if !archived && select {
                self.select(index);
            }
        }
    }

    fn forget_stale_opencode_session(&mut self, index: usize) {
        let selected = self.selected == Some(index);
        let (scope, options, draft, attachments) = {
            let session = &mut self.sessions[index];
            session.forgotten = true;
            session.archiving = false;
            session.error = None;
            (
                session.scope.clone(),
                session.options.clone(),
                std::mem::take(&mut session.draft),
                std::mem::take(&mut session.attachments),
            )
        };
        self.remembered.retain(|_, remembered| *remembered != index);
        let message = "This OpenCode session no longer belongs to this project. It was removed from Caelum, but was not archived in OpenCode.";
        if selected {
            let mut replacement_session = Session::new(scope);
            replacement_session.options = options;
            replacement_session.draft = draft;
            replacement_session.attachments = attachments;
            replacement_session.error = Some(message.into());
            let replacement = self.sessions.len();
            self.sessions.push(replacement_session);
            self.select(replacement);
        } else {
            self.error = Some(message.into());
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

    pub fn refresh_inbox(&mut self, index: usize) {
        let session = &self.sessions[index];
        if session.scope.provider != Provider::OpenCode
            || self
                .pending
                .values()
                .any(|p| matches!(p, Pending::Inbox(i, _) if *i == index))
        {
            return;
        }
        if let Some(key) = session.key() {
            self.request(
                Command::Inbox(key),
                Pending::Inbox(index, session.inbox.revision),
            );
        }
    }

    pub fn retry_queued(&mut self, index: usize, id: &str) {
        if self.sessions[index]
            .inbox
            .items
            .iter()
            .any(|item| item.id == id && item.removing)
        {
            self.remove_queued(index, id);
            return;
        }
        let session = &mut self.sessions[index];
        let Some(key) = session.key() else {
            return;
        };
        let Some(item) = session
            .inbox
            .items
            .iter_mut()
            .find(|v| v.id == id && !v.pending)
        else {
            return;
        };
        item.pending = true;
        item.error = None;
        let text = item.text.clone();
        session.inbox.revision += 1;
        self.request(
            Command::Enqueue(key, id.into(), text),
            Pending::Enqueue(index, id.into()),
        );
    }

    pub fn remove_queued(&mut self, index: usize, id: &str) {
        let session = &mut self.sessions[index];
        let Some(key) = session.key() else {
            return;
        };
        let Some(item) = session
            .inbox
            .items
            .iter_mut()
            .find(|v| v.id == id && !v.pending)
        else {
            return;
        };
        item.pending = true;
        item.removing = true;
        item.error = None;
        session.inbox.revision += 1;
        self.request(
            Command::RemoveQueued(key, id.into()),
            Pending::RemoveQueued(index, id.into()),
        );
    }

    fn advance_queue(&mut self, index: usize) {
        if self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Inbox(i, _) if *i == index))
        {
            return;
        }
        let session = &mut self.sessions[index];
        if !session.queue_paused
            && session.inbox.items.is_empty()
            && session.can_send()
            && !session.queue.is_empty()
            && session.error.is_none()
        {
            let queued = session.queue.remove(0);
            let options = std::mem::replace(&mut session.options, queued.options);
            let dirty = session.options_dirty;
            self.send_raw(index, queued.text);
            self.sessions[index].inflight_queued = true;
            self.sessions[index].options = options;
            self.sessions[index].options_dirty =
                dirty || self.sessions[index].options != self.sessions[index].inflight_options;
        }
    }

    pub fn options_changed(&mut self, index: usize) {
        self.sessions[index].options_dirty = true;
    }

    pub fn attach(&mut self, index: usize, attachment: Attachment) -> bool {
        self.sessions[index].attachments.retain(|f| {
            !(f.kind == attachment.kind
                && if f.kind == super::AttachmentKind::Image {
                    f.content == attachment.content
                } else {
                    f.path == attachment.path
                        && f.start_line == attachment.start_line
                        && f.end_line == attachment.end_line
                })
        });
        if self.sessions[index]
            .attachments
            .iter()
            .filter(|f| f.kind == attachment.kind)
            .count()
            >= 16
        {
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
        let project = self.sessions[index].scope.project.clone();
        let name = path.clone();
        let task = if is_image(&path) {
            Task::Image {
                project,
                source: path,
            }
        } else {
            Task::File { project, path }
        };
        self.request(Command::Task(task), Pending::Attachment(index, name));
    }

    pub fn attach_image(&mut self, index: usize, name: String, bytes: Vec<u8>) {
        if self.sessions[index].attaching + self.sessions[index].attachments.len() >= 16 {
            return;
        }
        self.sessions[index].attaching += 1;
        let task = Task::EncodeImage {
            project: self.sessions[index].scope.project.clone(),
            bytes,
        };
        self.request(Command::Task(task), Pending::Attachment(index, name));
    }

    pub fn complete_mention(
        &mut self,
        index: usize,
        token: super::Completion,
        path: String,
        project: String,
        replacement: String,
    ) {
        self.mention(index, false, token, path, project, replacement);
    }

    pub fn complete_side_mention(
        &mut self,
        index: usize,
        token: super::Completion,
        path: String,
        project: String,
        replacement: String,
    ) {
        self.mention(index, true, token, path, project, replacement);
    }

    fn mention(
        &mut self,
        index: usize,
        side: bool,
        token: super::Completion,
        path: String,
        project: String,
        replacement: String,
    ) {
        let session = &mut self.sessions[index];
        if session.attaching > 0 {
            return;
        }
        let draft = if side {
            &session.side_draft
        } else {
            &session.draft
        };
        if token.start > token.end || draft.get(token.start..token.end).is_none() {
            return;
        }
        session.attaching += 1;
        session.error = None;
        session.completion_generation += 1;
        let pending = Pending::Mention {
            index,
            side,
            generation: session.completion_generation,
            draft: draft.clone(),
            token: token.clone(),
            replacement,
        };
        let task = if token.kind == super::CompletionKind::PullRequest {
            Task::AttachPullRequest { project, url: path }
        } else {
            Task::File { project, path }
        };
        self.request(Command::Task(task), pending);
    }

    pub fn dismiss_side_question(&mut self, index: usize) {
        self.cancel_mentions(index);
        let session = &mut self.sessions[index];
        session.side_generation += 1;
        session.completion_generation += 1;
        session.generating = false;
        session.side_question = None;
        session.side_error = None;
        session.side_draft.clear();
        session.side_attachments.clear();
    }

    pub fn cancel_mentions(&mut self, index: usize) {
        let requests: Vec<_> = self
            .pending
            .iter()
            .filter_map(|(id, p)| {
                matches!(p,Pending::Mention {index:i,..} if *i==index).then_some(*id)
            })
            .collect();
        for id in &requests {
            self.pending.remove(id);
        }
        self.outgoing.retain(|(id, _)| !requests.contains(id));
        self.sessions[index].attaching = self.sessions[index]
            .attaching
            .saturating_sub(requests.len());
        self.sessions[index].completion_generation += 1;
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
        session.side_error = None;
        session.side_generation += 1;
        let generation = session.side_generation;
        session.side_draft = prompt.clone();
        session.side_question = Some((prompt.clone(), String::new()));
        let context = super::composer::with_context(&prompt, &session.side_attachments);
        self.request(
            Command::Generate(key, context),
            Pending::Generate(index, prompt, generation),
        );
        true
    }

    pub fn open_project(&mut self, path: String) {
        self.opening = true;
        self.error = None;
        self.latest_open = self.request(Command::Open(path), Pending::Open(None, self.navigation));
    }

    pub fn launch_project(&mut self, path: String, provider: Provider) {
        self.opening = true;
        self.error = None;
        self.latest_open = self.request(
            Command::Open(path),
            Pending::Open(Some(provider), self.navigation),
        );
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
            .filter(|i| !self.sessions[*i].archived && !self.sessions[*i].forgotten)
        {
            self.select(index);
        } else {
            self.new_session(Scope {
                project: path.into(),
                provider: self.preferences.default_provider,
            });
        }
    }

    /// Worktrees created by Caelum, scoped to their original project. Git's full
    /// catalogue is still used for branch choices and checkout availability.
    pub fn managed_worktrees<'a>(
        &'a self,
        project: &'a str,
    ) -> impl Iterator<Item = &'a caelum_protocol::GitWorktree> {
        self.services
            .worktrees(project)
            .into_iter()
            .flat_map(|listing| &listing.worktrees)
            .filter(move |tree| {
                !tree.main
                    && self.managed_worktrees.contains(&tree.path)
                    && self.group_path(&tree.project.path) == project
            })
    }

    /// Switch to this exact checkout, including the main checkout. A project
    /// group's remembered session may belong to a different linked worktree.
    pub fn select_checkout(&mut self, path: &str) -> bool {
        if !self.projects.iter().any(|project| project.path == path)
            || self
                .services
                .worktrees(self.group_path(path))
                .is_some_and(|listing| {
                    !listing
                        .worktrees
                        .iter()
                        .any(|tree| tree.project.path == path && tree.available)
                })
        {
            return false;
        }
        let usable = |i: usize| {
            let session = &self.sessions[i];
            session.scope.project == path && !session.archived && !session.forgotten
        };
        let index = self
            .remembered_checkouts
            .get(path)
            .or(self.remembered.get(path))
            .copied()
            .filter(|&i| usable(i))
            .or_else(|| (0..self.sessions.len()).rev().find(|&i| usable(i)));
        if let Some(index) = index {
            self.select(index);
        } else {
            self.new_session(Scope {
                project: path.into(),
                provider: self.preferences.default_provider,
            });
        }
        true
    }

    pub fn project_is_open(&self, path: &str) -> bool {
        let root = self.group_path(path);
        self.projects
            .iter()
            .any(|p| self.group_path(&p.path) == root)
    }

    /// Close a project group in navigation, retaining sessions, drafts and work.
    pub fn close_project(&mut self, path: &str) {
        let root = self.group_path(path).to_owned();
        let Some(position) = self
            .projects
            .iter()
            .position(|p| self.group_path(&p.path) == root)
        else {
            return;
        };
        let closed: std::collections::HashSet<_> = self
            .projects
            .iter()
            .filter(|p| self.group_path(&p.path) == root)
            .map(|p| p.path.clone())
            .collect();
        let active = self
            .active_project
            .as_ref()
            .is_some_and(|p| self.group_path(p) == root);
        self.closed_projects.insert(root.clone(), self.next_request);
        self.projects.retain(|p| !closed.contains(&p.path));
        if let Some(index) = self
            .last_archived
            .filter(|i| self.group_path(&self.sessions[*i].scope.project) == root)
        {
            self.dismiss_archive(index);
        }
        if active {
            if let Some(index) = self.selected {
                self.dismiss_side_question(index);
            }
            self.navigation += 1;
            self.active_project = None;
            self.selected = None;
            if let Some(project) = self
                .projects
                .get(position.min(self.projects.len().saturating_sub(1)))
            {
                let path = project.path.clone();
                self.select_project(&path);
            }
        }
    }

    fn closed_since(&self, project: &str, request: u64) -> bool {
        self.closed_projects
            .get(self.group_path(project))
            .is_some_and(|closed| request <= *closed)
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
            if let Some(branch) = self
                .services
                .checkouts
                .get(&path)
                .and_then(|c| c.git.as_ref())
                .and_then(|g| g.branch.clone())
            {
                self.task(Task::Discover {
                    project: path.clone(),
                    branch,
                });
            }
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

    pub fn refresh_choices(&mut self, provider: Provider) {
        let Some(project) = self.active_project.clone() else {
            return;
        };
        self.refresh_choices_for(Scope { project, provider });
    }

    pub fn refresh_choices_for(&mut self, scope: Scope) {
        if self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Choices(s) if s == &scope))
        {
            return;
        }
        self.notices.remove(&scope);
        self.request(
            Command::Choices(scope.clone()),
            Pending::Choices(scope.clone()),
        );
        if scope.provider == Provider::OpenCode {
            self.request(Command::Commands(scope.clone()), Pending::Commands(scope));
        }
    }

    pub fn test_sound(&mut self) {
        self.request(
            Command::Sound(self.preferences.sound_volume),
            Pending::Notify,
        );
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

    pub fn project_draft(&mut self, scope: Scope) -> usize {
        if let Some(i) = self.sessions.iter().rposition(|s| {
            s.scope == scope && s.remote_id.is_none() && !s.archived && !s.forgotten && !s.working()
        }) {
            self.select(i);
            i
        } else {
            self.new_session(scope)
        }
    }

    pub fn acknowledge(&mut self) -> bool {
        if let Some(i) = self.selected {
            if !self.window_background
                && !self.conversation_hidden
                && self.sessions[i].loaded
                && !self.sessions[i].loading
                && self.sessions[i].reading < 0.0
            {
                return std::mem::take(&mut self.sessions[i].unread);
            }
        }
        false
    }

    pub fn select(&mut self, index: usize) {
        if index >= self.sessions.len() || self.sessions[index].forgotten {
            return;
        }
        if self.selected != Some(index) {
            if let Some(previous) = self.selected {
                self.dismiss_side_question(previous);
            }
            self.navigation += 1;
        }
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        self.active_project = Some(session.scope.project.clone());
        self.remembered.insert(session.scope.project.clone(), index);
        self.remembered_checkouts
            .insert(session.scope.project.clone(), index);
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
        let inbox_loading = self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Inbox(i, _) if *i == index));
        let session = &mut self.sessions[index];
        if session.archived
            || session.forgotten
            || session.stopping
            || session.archiving
            || session.attaching > 0
            || session.commanding
        {
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
        let prompt = super::composer::with_context(&prompt, &session.attachments);
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
                session.inbox.items.push(super::InboxPrompt {
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
        if session.working()
            || session.loading
            || inbox_loading
            || !session.queue.is_empty()
            || !session.inbox.items.is_empty()
        {
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

    fn send_raw(&mut self, index: usize, prompt: String) -> bool {
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
            Event::Checkout(project) => self.checkout_changed(&project),
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
                if self.preferences.input_needed
                    && prompts.iter().any(|p| {
                        !self.approvals.contains(p)
                            && self.preferences.should_notify(self.visible_request(
                                &p.provider,
                                &p.project_path,
                                p.session_id.as_deref(),
                            ))
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
                if self.preferences.input_needed
                    && prompts.iter().any(|p| {
                        !self.questions.contains(p)
                            && self.preferences.should_notify(self.visible_request(
                                &p.provider,
                                &p.project_path,
                                Some(&p.session_id),
                            ))
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
        if let Pending::Mention {
            index,
            side,
            generation,
            draft,
            token,
            replacement,
        } = pending
        {
            let session = &mut self.sessions[index];
            session.attaching = session.attaching.saturating_sub(1);
            if self.selected != Some(index)
                || session.archived
                || generation != session.completion_generation
                || draft
                    != (if side {
                        &session.side_draft
                    } else {
                        &session.draft
                    })
                    .as_str()
            {
                return;
            }
            let attachment = match result {
                Ok(Reply::Task(TaskReply::File(file))) => {
                    Attachment::new(file.path, file.content, None, file.truncated)
                }
                Ok(Reply::Task(TaskReply::Details(details))) => Attachment::pull_request(&details),
                Err(error) => {
                    session.error = Some(error);
                    return;
                }
                _ => {
                    session.error = Some("Unexpected context response".into());
                    return;
                }
            };
            let attached = if side {
                let attachments = &mut self.sessions[index].side_attachments;
                attachments.retain(|a| !(a.kind == attachment.kind && a.path == attachment.path));
                if attachments.len() < 16 {
                    attachments.push(attachment);
                    true
                } else {
                    self.sessions[index].side_error = Some("Attach at most 16 items".into());
                    false
                }
            } else {
                self.attach(index, attachment)
            };
            if attached {
                let (text, caret) = token.replace(&draft, &replacement);
                if side {
                    self.sessions[index].side_draft = text;
                    self.sessions[index].side_completion_caret = Some(caret);
                } else {
                    self.sessions[index].draft = text;
                    self.sessions[index].completion_caret = Some(caret);
                }
            }
            return;
        }
        if let Pending::Task(task) = pending {
            let launch = self.launches.remove(&request);
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
            if let (
                Task::Shell {
                    project,
                    session,
                    id,
                    stop: true,
                },
                Ok(TaskReply::Shell(output)),
            ) = (&task, &result)
            {
                let output = if output.is_empty() {
                    self.services
                        .checkouts
                        .get(project)
                        .and_then(|c| c.shell_output.get(&(session.clone(), id.clone())))
                        .unwrap_or(output)
                } else {
                    output
                };
                for owner in self.sessions.iter_mut().filter(|s| {
                    s.scope.provider == Provider::OpenCode
                        && s.scope.project == *project
                        && s.remote_id.as_deref() == Some(session)
                }) {
                    if let Some(shell) = owner.shells.iter().find(|s| s.id == *id) {
                        owner.conversation.apply(Provider::OpenCode, "session.shell.ended", &serde_json::json!({
                            "shell": {"id":id,"command":shell.command,"status":"killed","time":{"started":shell.started_at,"completed":super::presentation::now()}},
                            "output":{"output":output}
                        }));
                    }
                    owner.shells.retain(|s| s.id != *id);
                    owner.shell_revision += 1;
                    owner.revision += 1;
                }
            }
            if let Ok(TaskReply::Git(_, updates)) = &result {
                for update in updates {
                    self.branch_update(update.clone());
                }
            }
            if let Ok(TaskReply::Worktree(worktree)) = &result {
                if let Task::CreateWorktree { project, .. } = &task {
                    self.managed_worktrees.insert(worktree.path.clone());
                    // Record ownership before opening: closing the group must also
                    // reject an in-flight Open for this newly created checkout.
                    let root = self.group_path(project).to_owned();
                    self.project_roots
                        .insert(worktree.project.path.clone(), root);
                    if let Some((provider, _)) = launch.filter(|(_, navigation)| {
                        *navigation == self.navigation && !self.closed_since(project, request)
                    }) {
                        self.launch_project(worktree.project.path.clone(), provider);
                    }
                }
                if matches!(task, Task::RemoveWorktree { .. }) {
                    self.managed_worktrees.remove(&worktree.path);
                    let root = self.project_roots.get(&worktree.project.path).cloned();
                    self.projects.retain(|p| p.path != worktree.project.path);
                    for session in &mut self.sessions {
                        if session.scope.project == worktree.project.path {
                            session.archived = true;
                        }
                    }
                    self.remembered
                        .retain(|_, i| self.sessions[*i].scope.project != worktree.project.path);
                    self.remembered_checkouts
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
                let root = self.group_path(task.project()).to_owned();
                if !self.closed_since(&root, request) && self.project_is_open(&root) {
                    // Older versions already created worktrees in this repository's
                    // dedicated Caelum storage folder. Remember them across directory
                    // preference changes, alongside explicitly recorded creations.
                    for worktree in &listing.worktrees {
                        if !worktree.main
                            && std::path::Path::new(&worktree.path).parent()
                                == Some(std::path::Path::new(&listing.storage_directory))
                        {
                            self.managed_worktrees.insert(worktree.path.clone());
                        }
                    }
                    for worktree in &listing.worktrees {
                        if worktree.project.path != root
                            && (worktree.main || !self.managed_worktrees.contains(&worktree.path))
                        {
                            continue;
                        }
                        let owner = self
                            .project_roots
                            .entry(worktree.project.path.clone())
                            .or_insert_with(|| root.clone());
                        if *owner != root {
                            continue;
                        }
                        if worktree.available && worktree.project.path != root {
                            let mut project_info = worktree.project.clone();
                            project_info.name = worktree.label.clone().unwrap_or_else(|| {
                                std::path::Path::new(&worktree.path)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned()
                            });
                            if let Some(project) = self
                                .projects
                                .iter_mut()
                                .find(|p| p.path == worktree.project.path)
                            {
                                project.name = project_info.name;
                            } else {
                                self.projects.push(project_info);
                                for provider in Provider::ALL {
                                    let scope = Scope {
                                        project: worktree.project.path.clone(),
                                        provider,
                                    };
                                    self.request(
                                        Command::List(scope.clone()),
                                        Pending::List(scope.clone()),
                                    );
                                    self.refresh_choices_for(scope);
                                }
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
            if let Task::Shell {
                project,
                session,
                id,
                stop: false,
            } = &task
            {
                let running = self.sessions.iter().any(|owner| {
                    owner.scope.provider == Provider::OpenCode
                        && owner.scope.project == *project
                        && owner.remote_id.as_ref() == Some(session)
                        && owner.shells.iter().any(|shell| shell.id == *id)
                });
                if !running {
                    if let Some(checkout) = self.services.checkouts.get_mut(project) {
                        checkout.shell_output.remove(&(session.clone(), id.clone()));
                    }
                }
            }
            if let Some(next) = self.refresh_after.remove(&task.slot()) {
                self.task(next);
            }
            if let Task::Git { project } = &task {
                if self.dirty_checkouts.remove(project) {
                    self.checkout_changed(project);
                }
            }
            return;
        }
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                match pending {
                    Pending::Inbox(index, _) => self.sessions[index].error = Some(error),
                    Pending::Enqueue(index, id) | Pending::RemoveQueued(index, id) => {
                        let inbox = &mut self.sessions[index].inbox;
                        inbox.revision += 1;
                        if let Some(item) = inbox.items.iter_mut().find(|v| v.id == id) {
                            item.pending = false;
                            item.error = Some(error);
                        }
                    }
                    Pending::Control(index, prompt) => {
                        self.sessions[index].commanding = false;
                        self.sessions[index].error = Some(error);
                        self.sessions[index].failed_prompt = Some(prompt);
                    }
                    Pending::Generate(index, _, generation) => {
                        if self.sessions[index].side_generation == generation {
                            self.sessions[index].generating = false;
                            self.sessions[index].side_error = Some(error);
                        }
                    }
                    Pending::Notify => {}
                    Pending::Load => self.persistence_error = Some(error),
                    Pending::Save(_, mut commands) => {
                        self.saving = false;
                        commands.append(&mut self.durable_outgoing);
                        self.durable_outgoing = commands;
                        self.persistence_error = Some(error);
                    }
                    Pending::Task(_) | Pending::Mention { .. } => unreachable!(),
                    Pending::Commands(scope) => {
                        self.notices.insert(scope, error);
                    }
                    Pending::Shells(index, _) => self.sessions[index].error = Some(error),
                    Pending::Archive(index, true, _, _)
                        if self.sessions[index].scope.provider == Provider::OpenCode
                            && error
                                .contains("OpenCode session does not belong to this project") =>
                    {
                        self.forget_stale_opencode_session(index);
                    }
                    Pending::Archive(index, archived, _, _) => {
                        self.sessions[index].archiving = false;
                        self.sessions[index].error = Some(error);
                        if !archived && self.last_archived == Some(index) {
                            self.archive_clock = Some((super::presentation::now(), 8_000));
                        }
                    }
                    Pending::Attachment(index, _) => {
                        self.sessions[index].attaching =
                            self.sessions[index].attaching.saturating_sub(1);
                        self.sessions[index].error = Some(error);
                    }
                    Pending::Details(_) => {}
                    Pending::Open(_, _) => {
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
                        if session.inflight_queued {
                            session.queue.insert(
                                0,
                                QueuedPrompt {
                                    text: prompt,
                                    options: session.inflight_options.clone(),
                                },
                            );
                        } else {
                            session.failed_prompt = Some(prompt);
                        }
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
                            if session.inflight_queued {
                                session.queue.insert(
                                    0,
                                    QueuedPrompt {
                                        text: prompt,
                                        options: session.inflight_options.clone(),
                                    },
                                );
                            } else {
                                session.failed_prompt = Some(prompt);
                            }
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
                                    .is_none_or(|id| target.remote_id.as_ref() == Some(id))
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
            (Pending::Inbox(index, revision), Reply::Inbox(items)) => {
                self.sessions[index].inbox.snapshot(revision, &items)
            }
            (Pending::Enqueue(index, id), Reply::Done) => {
                let inbox = &mut self.sessions[index].inbox;
                inbox.revision += 1;
                if let Some(item) = inbox.items.iter_mut().find(|v| v.id == id) {
                    item.pending = false;
                    item.error = None;
                }
            }
            (Pending::RemoveQueued(index, id), Reply::Done) => {
                self.sessions[index].inbox.settle(&id);
            }
            (Pending::Control(index, _), Reply::Done) => {
                self.sessions[index].commanding = false;
                self.refresh(index);
            }
            (Pending::Generate(index, prompt, generation), Reply::Generated(text)) => {
                if self.sessions[index].side_generation == generation {
                    self.sessions[index].generating = false;
                    self.sessions[index].side_question = Some((prompt, text));
                }
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
            (Pending::Attachment(index, _), Reply::Task(TaskReply::File(file))) => {
                self.sessions[index].attaching = self.sessions[index].attaching.saturating_sub(1);
                self.attach(
                    index,
                    Attachment::new(file.path, file.content, None, file.truncated),
                );
            }
            (Pending::Attachment(index, name), Reply::Task(TaskReply::Image(data))) => {
                self.sessions[index].attaching = self.sessions[index].attaching.saturating_sub(1);
                let mut attachment = Attachment::new(name, String::new(), None, false);
                attachment.kind = super::AttachmentKind::Image;
                attachment.content = data;
                self.attach(index, attachment);
            }
            (Pending::Load, Reply::Snapshot(snapshot)) => {
                self.store_ready = true;
                if let Some(snapshot) = snapshot {
                    let snapshot = *snapshot;
                    self.saved = Some(snapshot.clone());
                    self.managed_worktrees = snapshot.managed_worktrees;
                    self.preferences = snapshot.preferences;
                    for (project, saved) in &self.preferences.files {
                        let c = self.services.checkouts.entry(project.clone()).or_default();
                        c.tabs = saved
                            .tabs
                            .iter()
                            .filter(|p| {
                                !p.is_empty()
                                    && !std::path::Path::new(p).is_absolute()
                                    && !std::path::Path::new(p)
                                        .components()
                                        .any(|c| matches!(c, std::path::Component::ParentDir))
                            })
                            .take(40)
                            .cloned()
                            .collect();
                        c.selected_file = saved
                            .selected
                            .clone()
                            .filter(|p| c.tabs.contains(p))
                            .or_else(|| c.tabs.first().cloned());
                    }
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
                                session.inbox.items = saved.inbox;
                                for item in &mut session.inbox.items {
                                    if item.pending {
                                        item.pending = false;
                                        item.error = Some(
                                            "Delivery needs reconciliation. Refresh or retry."
                                                .into(),
                                        );
                                    }
                                }
                                if let Some(pending) = saved.pending_submission {
                                    session.queue.insert(0, pending);
                                }
                                session.queue_paused = true;
                                session.archived = saved.archived;
                                session.reading = saved.reading;
                                session.unread = saved.unread;
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
                            self.remembered_checkouts.insert(s.scope.project.clone(), i);
                        }
                        self.remembered
                            .extend(snapshot.remembered.into_iter().filter(|(_, i)| {
                                *i < self.sessions.len() && !self.sessions[*i].archived
                            }));
                        self.remembered_checkouts.extend(
                            self.remembered
                                .iter()
                                .filter(|(path, i)| self.sessions[**i].scope.project == **path)
                                .map(|(path, i)| (path.clone(), *i)),
                        );
                        self.remembered_checkouts.extend(
                            snapshot
                                .remembered_checkouts
                                .into_iter()
                                .filter(|(path, i)| {
                                    self.sessions
                                        .get(*i)
                                        .is_some_and(|s| s.scope.project == *path && !s.archived)
                                }),
                        );
                        let details: Vec<_> = self
                            .sessions
                            .iter()
                            .enumerate()
                            .filter(|(_, s)| self.project_is_open(&s.scope.project))
                            .filter_map(|(i, s)| s.remote_id.as_ref().map(|_| i))
                            .collect();
                        self.detail_queue.extend(details);
                        if let Some(index) = snapshot.selected.filter(|i| *i < self.sessions.len())
                        {
                            self.select(index);
                            self.refresh_project();
                        }
                    }
                }
            }
            (Pending::Save(snapshot, mut commands), Reply::Done) => {
                self.saved = Some(*snapshot);
                self.saving = false;
                self.ready_outgoing.append(&mut commands);
            }
            (Pending::Commands(scope), Reply::Commands(commands)) => {
                self.provider_commands.insert(scope, commands);
            }
            (Pending::Shells(index, revision), Reply::Shells(shells)) => {
                let session = &mut self.sessions[index];
                if revision != session.shell_revision {
                    if let Some(key) = session.key() {
                        let revision = session.shell_revision;
                        self.request(Command::Shells(key), Pending::Shells(index, revision));
                    }
                    return;
                }
                session.shells =
                    shells
                        .into_iter()
                        .filter(|shell| {
                            !session.conversation.entries.iter().any(|e| {
                                e.id == format!("opencode-shell-{}", shell.id) && e.complete
                            })
                        })
                        .collect();
                if let (Some(remote), Some(checkout)) = (
                    &session.remote_id,
                    self.services.checkouts.get_mut(&session.scope.project),
                ) {
                    checkout.shell_output.retain(|(owner, id), _| {
                        owner != remote || session.shells.iter().any(|shell| &shell.id == id)
                    });
                }
            }
            (Pending::Archive(index, archived, select, revision), Reply::Done) => {
                self.sessions[index].archiving = false;
                self.sessions[index].archived = archived;
                if archived {
                    let owns_notice = self.archived_notice(index, revision);
                    if self.selected == Some(index) {
                        self.new_session(self.sessions[index].scope.clone());
                        if owns_notice {
                            self.archive_replacement = self.selected;
                        }
                    }
                } else {
                    self.dismiss_archive(index);
                    if !self.closed_since(&self.sessions[index].scope.project, request)
                        && select.is_some_and(|(selected, navigation)| {
                            self.selected == Some(selected) && self.navigation == navigation
                        })
                    {
                        self.select(index);
                    }
                }
            }
            (Pending::Open(provider, navigation), Reply::Project(project)) => {
                if self.closed_since(&project.path, request) {
                    if request == self.latest_open {
                        self.opening = false;
                    }
                    return;
                }
                if !self.projects.iter().any(|p| p.path == project.path) {
                    self.projects.push(project.clone());
                }
                // Restored, closed groups defer detail reads until reopened.
                let details: Vec<_> = self
                    .sessions
                    .iter()
                    .enumerate()
                    .filter(|(i, s)| {
                        self.group_path(&s.scope.project) == self.group_path(&project.path)
                            && s.remote_id.is_some()
                            && s.details.is_none()
                            && !self.detail_queue.contains(i)
                            && !self
                                .pending
                                .values()
                                .any(|p| matches!(p, Pending::Details(index) if index == i))
                    })
                    .map(|(i, _)| i)
                    .collect();
                self.detail_queue.extend(details);
                if request == self.latest_open {
                    self.opening = false;
                    if provider.is_none() || navigation == self.navigation {
                        self.select_project(&project.path);
                        if let Some(provider) = provider {
                            self.project_draft(Scope {
                                project: project.path.clone(),
                                provider,
                            });
                        }
                        self.refresh_project();
                    }
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
                    self.request(
                        Command::Shells(key.clone()),
                        Pending::Shells(index, self.sessions[index].shell_revision),
                    );
                    self.refresh_inbox(index);
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
                if session.scope.provider == Provider::OpenCode {
                    for message in &page.data {
                        if message["type"] == "user" {
                            if let Some(id) = message["id"].as_str() {
                                session.inbox.settle(id);
                            }
                        }
                    }
                }
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
                if session.forgotten {
                    continue;
                }
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
        !self.window_background
            && !self.conversation_hidden
            && self.selected.is_some_and(|i| {
                let session = &self.sessions[i];
                session.scope.project == project
                    && session.scope.provider.as_str() == provider
                    && session.reading < 0.0
                    && id.is_none_or(|id| session.remote_id.as_deref() == Some(id))
            })
    }

    fn provider_event(&mut self, path: &str, provider: Provider, method: &str, data: &Value) {
        if method == "filesystem.changed" && self.active_project.as_deref() == Some(path) {
            self.refresh_files();
            return;
        }
        let disconnected = matches!(method, "codex/processExited" | "server.disconnected");
        let id = match provider {
            Provider::Codex => data["threadId"].as_str().or(data["thread"]["id"].as_str()),
            Provider::OpenCode => data["sessionID"]
                .as_str()
                .or(data["session"]["id"].as_str()),
        };
        let mut completed = Vec::new();
        let mut shells_completed = Vec::new();
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
            if let Some(id) = data["_event_id"].as_str() {
                if session.seen_events.iter().any(|seen| seen == id) {
                    continue;
                }
                session.seen_events.push_back(id.into());
                if session.seen_events.len() > 512 {
                    session.seen_events.pop_front();
                }
            }
            let was_working = session.working();
            let activity = session.conversation.activity;
            if method.starts_with("session.inbox.") {
                if let Some(item) = session.inbox.event(method, data) {
                    session.conversation.history(
                        provider,
                        &[serde_json::json!({"id": item.id, "type":"user", "text":item.text})],
                        false,
                    );
                }
            }
            if method == "session.shell.started" {
                session.shell_revision += 1;
                if let (Some(id), Some(command), Some(started_at)) = (
                    data["shell"]["id"].as_str(),
                    data["shell"]["command"].as_str(),
                    data["shell"]["time"]["started"].as_u64(),
                ) {
                    if !session.shells.iter().any(|s| s.id == id)
                        && !session
                            .conversation
                            .entries
                            .iter()
                            .any(|e| e.id == format!("opencode-shell-{id}") && e.complete)
                    {
                        session
                            .shells
                            .push(caelum_protocol::OpenCodeBackgroundShell {
                                id: id.into(),
                                command: command.into(),
                                started_at,
                            });
                    }
                }
            }
            if method == "session.shell.ended" {
                session.shell_revision += 1;
                let shell = data["shell"]["id"].as_str().unwrap_or_default();
                let entry_id = format!("opencode-shell-{shell}");
                if session
                    .conversation
                    .entries
                    .iter()
                    .any(|e| e.id == entry_id && !e.complete)
                    || session.shells.iter().any(|s| s.id == shell)
                {
                    shells_completed.push(index);
                }
                session.shells.retain(|s| s.id != shell);
                if let (Some(checkout), Some(remote)) = (
                    self.services.checkouts.get_mut(&session.scope.project),
                    &session.remote_id,
                ) {
                    checkout
                        .shell_output
                        .remove(&(remote.clone(), shell.to_owned()));
                }
            }
            session.revision += 1;
            session.conversation.apply(provider, method, data);
            session.unread |= activity != session.conversation.activity;
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
            if self.preferences.should_notify(
                self.selected == Some(index)
                    && !self.window_background
                    && !self.conversation_hidden
                    && self.sessions[index].reading < 0.0,
            ) && !self.sessions[index].disconnected
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
        for index in shells_completed {
            if self.preferences.background_shells
                && self.preferences.should_notify(self.visible_request(
                    provider.as_str(),
                    path,
                    self.sessions[index].remote_id.as_deref(),
                ))
            {
                self.request(
                    Command::Notify(
                        "Caelum · Background command finished".into(),
                        self.sessions[index].title.clone(),
                    ),
                    Pending::Notify,
                );
            }
        }
        if matches!(
            method,
            "item/completed"
                | "turn/completed"
                | "session.shell.ended"
                | "session.execution.succeeded"
                | "session.tool.success"
                | "session.tool.failed"
                | "session.execution.failed"
                | "session.execution.interrupted"
                | "filesystem.changed"
        ) {
            self.checkout_changed(path);
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

fn is_image(path: &str) -> bool {
    matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "ico")
    )
}
