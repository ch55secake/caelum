//! A saved, session-independent starting point. Requests never select a conversation.
use super::{is_image, Pending, Session, Workspace};
use crate::application::{
    Attachment, AttachmentKind, Command, Options, Provider, Reply, Scope, Task, TaskReply,
};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeDraft {
    pub text: String,
    pub attachments: Vec<Attachment>,
    pub project: Option<String>,
    /// None follows the user's new-session defaults until explicitly changed.
    pub provider: Option<Provider>,
    pub options: Options,
}

#[derive(Default)]
enum Target {
    #[default]
    Ready,
    Opening(u64),
}

#[derive(Default)]
pub struct Home {
    pub draft: HomeDraft,
    pub error: Option<String>,
    pub attaching: usize,
    generation: u64,
    target: Target,
}

impl Home {
    /// Captured by native file dialogs so a reply cannot attach to a later draft.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn opening(&self) -> bool {
        matches!(self.target, Target::Opening(_))
    }

    pub(super) fn restore(&mut self, draft: HomeDraft) {
        // Loading storage must not replace input entered during startup.
        if self.draft == HomeDraft::default()
            && self.generation == 0
            && self.attaching == 0
            && !self.opening()
        {
            self.draft = draft;
        }
    }
}

pub(super) enum Request {
    Open,
    Attachment { generation: u64, name: String },
}

impl Workspace {
    /// A navigation barrier for asynchronous project opens and archive restoration.
    pub fn leave_conversation(&mut self) {
        self.navigation += 1;
        self.notifications.cancel();
        if let Some(index) = self.selected.take() {
            self.cancel_mentions(index);
        }
        self.conversation_hidden = true;
    }

    pub fn home_provider(&self) -> Provider {
        self.home
            .draft
            .provider
            .unwrap_or(self.preferences.default_provider)
    }

    pub fn home_scope(&self) -> Option<Scope> {
        Some(Scope {
            project: self.home.draft.project.clone()?,
            provider: self.home_provider(),
        })
    }

    pub fn home_options(&self) -> Options {
        if self.home.draft.provider.is_some() {
            self.home.draft.options.clone()
        } else {
            self.preferences
                .providers
                .get(self.home_provider().as_str())
                .cloned()
                .unwrap_or_default()
        }
    }

    pub fn home_set_provider(&mut self, provider: Provider) {
        self.home.draft.options = if self.home_provider() == provider {
            self.home_options()
        } else {
            self.preferences
                .providers
                .get(provider.as_str())
                .cloned()
                .unwrap_or_default()
        };
        self.home.draft.provider = Some(provider);
        if let Some(scope) = self.home_scope() {
            self.refresh_choices_for(scope);
        }
    }

    pub fn home_project_available(&self) -> bool {
        self.home.draft.project.as_deref().is_some_and(|path| {
            self.projects.iter().any(|p| p.path == path)
                && self
                    .services
                    .worktrees(self.group_path(path))
                    .is_none_or(|listing| {
                        listing
                            .worktrees
                            .iter()
                            .any(|t| t.project.path == path && t.available)
                    })
        })
    }

    pub fn home_select_project(&mut self, project: String) {
        self.home.generation += 1;
        self.home.attaching = 0;
        self.home.target = Target::Ready;
        self.home.error = None;
        self.home.draft.project = Some(project.clone());
        if let Some(scope) = self.home_scope() {
            self.refresh_choices_for(scope);
        }
        self.task(Task::Worktrees {
            project: self.group_path(&project).into(),
            directory: self.preferences.worktree_directory.clone(),
        });
    }

    pub fn home_open_project(&mut self, path: String) {
        self.home.error = None;
        let request = self.request(Command::Open(path), Pending::Home(Request::Open));
        self.home.target = Target::Opening(request);
    }

    pub fn home_attach_file(&mut self, path: String) {
        let Some(project) = self
            .home
            .draft
            .project
            .clone()
            .filter(|_| self.home_project_available())
        else {
            self.home.error = Some("Choose a project before attaching files".into());
            return;
        };
        let task = if is_image(&path) {
            Task::Image {
                project,
                source: path.clone(),
            }
        } else {
            Task::File {
                project,
                path: path.clone(),
            }
        };
        self.home_attachment(path, task);
    }

    pub fn home_attach_image(&mut self, name: String, bytes: Vec<u8>) {
        self.home_attachment(
            name,
            Task::EncodeImage {
                project: self.home.draft.project.clone().unwrap_or_default(),
                bytes,
            },
        );
    }

    fn home_attachment(&mut self, name: String, task: Task) {
        if self.home.attaching + self.home.draft.attachments.len() >= 16 {
            self.home.error = Some("Attach at most 16 items per message".into());
            return;
        }
        self.home.attaching += 1;
        self.home.error = None;
        self.request(
            Command::Task(task),
            Pending::Home(Request::Attachment {
                generation: self.home.generation,
                name,
            }),
        );
    }

    pub(super) fn complete_home(
        &mut self,
        request: u64,
        pending: Request,
        result: Result<Reply, String>,
    ) {
        match pending {
            Request::Open => {
                if !matches!(self.home.target, Target::Opening(id) if id == request) {
                    return;
                }
                self.home.target = Target::Ready;
                match result {
                    Ok(Reply::Project(project)) => {
                        if self.closed_since(&project.path, request) {
                            return;
                        }
                        if !self.projects.iter().any(|p| p.path == project.path) {
                            self.projects.push(project.clone());
                        }
                        self.home_select_project(project.path);
                    }
                    other => {
                        self.home.error = Some(
                            other
                                .err()
                                .unwrap_or_else(|| "Unexpected project response".into()),
                        )
                    }
                }
            }
            Request::Attachment { generation, name } => {
                if generation != self.home.generation {
                    return;
                }
                self.home.attaching = self.home.attaching.saturating_sub(1);
                let attachment = match result {
                    Ok(Reply::Task(TaskReply::File(file))) => {
                        Attachment::new(file.path, file.content, None, file.truncated)
                    }
                    Ok(Reply::Task(TaskReply::Image(data))) => {
                        let mut attachment = Attachment::new(name, String::new(), None, false);
                        attachment.kind = AttachmentKind::Image;
                        attachment.content = data;
                        attachment
                    }
                    other => {
                        self.home.error = Some(format!(
                            "Could not attach {name}: {}",
                            other
                                .err()
                                .unwrap_or_else(|| "Unexpected attachment response".into())
                        ));
                        return;
                    }
                };
                self.home.draft.attachments.retain(|a| {
                    !(a.kind == attachment.kind
                        && a.path == attachment.path
                        && a.content == attachment.content)
                });
                self.home.draft.attachments.push(attachment);
            }
        }
    }

    pub fn send_home(&mut self) -> Option<usize> {
        if self.home.opening()
            || self.home.attaching > 0
            || (self.home.draft.text.trim().is_empty() && self.home.draft.attachments.is_empty())
        {
            return None;
        }
        if !self.home_project_available() {
            self.home.error =
                Some("Choose an available project or worktree to start a session".into());
            return None;
        }
        let scope = self.home_scope()?;
        let mut session = Session::new(scope.clone());
        session.options = self.home_options();
        if session.options.model.is_none() {
            session.options.model = self
                .choices
                .get(&scope)
                .and_then(|c| c.default_model.clone());
        }
        session.attachments = self.home.draft.attachments.clone();
        let index = self.sessions.len();
        self.sessions.push(session);
        if !self.send(index, self.home.draft.text.clone()) {
            self.home.error = self.sessions.pop().and_then(|s| s.error);
            return None;
        }
        self.select(index);
        self.home.draft.text.clear();
        self.home.draft.attachments.clear();
        self.home.error = None;
        self.home.generation += 1;
        Some(index)
    }
}
