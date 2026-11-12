mod archive;
mod completion;
mod composer;
mod events;
mod notifications;
mod persistence;
mod projects;
mod queue;
mod sessions;
mod submission;
mod tasks;

use std::collections::{BTreeSet, HashMap};

use caelum_protocol::{ApprovalPrompt, ProjectInfo, ProviderChoices, QuestionPrompt};

use super::{
    Attachment, Command, Conversation, Options, Preferences, Provider, QueuedPrompt, Scope,
    Services, SessionKey, Snapshot, Store, Summary, Task,
};

pub struct Session {
    pub scope: Scope,
    pub remote_id: Option<String>,
    pub title: String,
    /// Last meaningful activity in Unix milliseconds; selection and token deltas do not bump it.
    pub last_activity: Option<u64>,
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
    pub attachment_errors: Vec<(String, String)>,
    pub completion_caret: Option<usize>,
    pub completion_generation: u64,
    pub queue: Vec<QueuedPrompt>,
    pub inbox: super::Inbox,
    pub queue_paused: bool,
    pub archived: bool,
    /// Discovery must have started after the last successful local archive change.
    archive_barrier: u64,
    /// Removed from Caelum because the provider no longer associates it with
    /// this project. Kept in memory only so pending requests retain stable indices.
    pub forgotten: bool,
    pub archiving: bool,
    pub deleting: bool,
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
            last_activity: None,
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
            attachment_errors: Vec::new(),
            completion_caret: None,
            completion_generation: 0,
            queue: Vec::new(),
            inbox: Default::default(),
            queue_paused: false,
            archived: false,
            archive_barrier: 0,
            forgotten: false,
            archiving: false,
            deleting: false,
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

    fn record_activity(&mut self, at: u64) {
        self.last_activity = Some(self.last_activity.unwrap_or_default().max(at));
    }

    pub fn attachment_error(&mut self, name: String, error: String) {
        self.attachment_errors.retain(|(path, _)| *path != name);
        if self.attachment_errors.len() == 16 {
            self.attachment_errors.remove(0);
        }
        self.attachment_errors.push((name, error));
    }

    fn has_context(&self) -> bool {
        self.remote_id.is_some()
            || !self.conversation.entries.is_empty()
            || !self.draft.trim().is_empty()
            || !self.attachments.is_empty()
            || !self.queue.is_empty()
            || !self.inbox.items.is_empty()
            || self.title != "New session"
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

    /// Whether a draft may be submitted, including to the queue while working.
    pub fn can_submit(&self) -> bool {
        !self.archived
            && !self.forgotten
            && !self.stopping
            && !self.archiving
            && self.attaching == 0
            && !self.commanding
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
    ListArchived(Scope),
    Archive(usize, bool, Option<(usize, u64)>, u64),
    DeleteSession(usize),
    Attachment(usize, String),
    Mention {
        index: usize,
        side: bool,
        name: String,
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
    notifications: notifications::State,
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
    deferred_lists: HashMap<Scope, (u64, Vec<Summary>)>,
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
}

fn update_summary(session: &mut Session, summary: Summary) {
    // A refresh during execution must not reorder rows on every streamed update.
    if !session.working() || session.last_activity.is_none() || session.revision == 0 {
        if let Some(at) = summary.updated_at {
            session.record_activity(at);
        }
    }
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
