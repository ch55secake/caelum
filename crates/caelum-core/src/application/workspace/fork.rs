//! Provider-backed conversation forks. Picker pages are read independently of the parent's transcript.
mod prompt;

use super::{update_summary, Pending, Session, Workspace};
use crate::application::{presentation, split_context, Command, Provider, Reply, SessionKey};
use caelum_protocol::SessionHistoryPage;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ForkBoundary {
    CodexTurn(String),
    CodexBefore(String),
    OpenCodeBefore(String),
    OpenCodeTip(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HistoryAction {
    Fork,
    Edit,
    Rewind,
}

#[derive(Debug)]
pub struct ForkPoint {
    pub label: String,
    pub boundary: ForkBoundary,
    pub newer_turns: usize,
    entry: String,
    draft: Option<Result<prompt::Draft, String>>,
}

enum Phase {
    Ready,
    Reading(u64),
    Creating(u64),
    Confirm(ForkBoundary, FilePreview),
}

pub enum FilePreview {
    ConversationOnly,
    Loading(u64),
    Available(Vec<String>),
    Unavailable(String),
}

pub struct ForkPicker {
    token: u64,
    pub action: HistoryAction,
    index: usize,
    source: SessionKey,
    navigation: u64,
    phase: Phase,
    pub points: Vec<ForkPoint>,
    seen_turns: usize,
    pub error: Option<String>,
    cursor: Option<String>,
    // OpenCode pages are newest first. Carry the following user prompt across a page break.
    newer_prompt: Option<String>,
    latest_prompt: Option<String>,
    tip: Option<String>,
    target_entry: Option<String>,
    cursors: std::collections::HashSet<String>,
}

impl ForkPicker {
    pub fn token(&self) -> u64 {
        self.token
    }
    pub fn confirmation(&self) -> Option<&ForkPoint> {
        let Phase::Confirm(boundary, _) = &self.phase else {
            return None;
        };
        self.points.iter().find(|p| &p.boundary == boundary)
    }
    pub fn file_preview(&self) -> Option<&FilePreview> {
        match &self.phase {
            Phase::Confirm(_, files) => Some(files),
            _ => None,
        }
    }
    pub fn loading(&self) -> bool {
        matches!(self.phase, Phase::Reading(_))
    }
    pub fn creating(&self) -> bool {
        matches!(self.phase, Phase::Creating(_))
    }
    pub fn has_older(&self) -> bool {
        self.cursor.is_some()
    }

    fn replace_page(&mut self, page: SessionHistoryPage) {
        if page
            .next_cursor
            .as_ref()
            .is_some_and(|c| c.is_empty() || !self.cursors.insert(c.clone()))
        {
            self.cursor = None;
            self.error =
                Some("The provider returned an invalid history cursor. Reload the picker.".into());
            return;
        }
        self.cursor = page.next_cursor;
        self.points.clear();
        for record in page.data.iter().rev() {
            let Some(id) = record["id"].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
            let tip = self.tip.get_or_insert_with(|| id.to_owned());
            let (boundary, prompt, user, entry) = match self.source.scope.provider {
                Provider::Codex => {
                    let status = record["status"]
                        .as_str()
                        .or(record["status"]["type"].as_str());
                    if !matches!(status, Some("completed" | "failed" | "interrupted")) {
                        continue;
                    }
                    let Some(user) = record["items"]
                        .as_array()
                        .and_then(|items| items.iter().find(|i| i["type"] == "userMessage"))
                    else {
                        continue;
                    };
                    let text = user["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|item| item["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let boundary = if self.action == HistoryAction::Fork {
                        ForkBoundary::CodexTurn(id.into())
                    } else {
                        ForkBoundary::CodexBefore(id.into())
                    };
                    (
                        boundary,
                        text,
                        user,
                        format!("codex-{}", user["id"].as_str().unwrap_or("")),
                    )
                }
                Provider::OpenCode => {
                    if record["type"] != "user" {
                        continue;
                    }
                    self.latest_prompt.get_or_insert_with(|| id.to_owned());
                    let boundary = if self.action != HistoryAction::Fork {
                        ForkBoundary::OpenCodeBefore(id.into())
                    } else {
                        self.newer_prompt.replace(id.into()).map_or_else(
                            || ForkBoundary::OpenCodeTip(tip.clone()),
                            ForkBoundary::OpenCodeBefore,
                        )
                    };
                    (
                        boundary,
                        record["text"].as_str().unwrap_or_default().to_owned(),
                        record,
                        format!("opencode-{id}"),
                    )
                }
            };
            let shown = presentation::visible_text(split_context(&prompt).0);
            let label = shown
                .lines()
                .map(str::trim)
                .find(|s| !s.is_empty())
                .unwrap_or("Attached context")
                .chars()
                .take(180)
                .collect();
            self.points.push(ForkPoint {
                label,
                boundary,
                newer_turns: self.seen_turns,
                entry,
                draft: (self.action == HistoryAction::Edit)
                    .then(|| prompt::recover(self.source.scope.provider, &prompt, user)),
            });
            self.seen_turns += 1;
        }
        if let Some(target) = &self.target_entry {
            self.points.retain(|p| &p.entry == target);
            if !self.points.is_empty() {
                self.cursor = None;
            } else if self.cursor.is_none() {
                self.error =
                    Some("The selected prompt is no longer in the provider history.".into());
            }
        }
    }
}

pub(super) enum Request {
    Page,
    Files,
    Create {
        index: usize,
        source: SessionKey,
        editing: bool,
    },
    Rewind {
        index: usize,
        source: SessionKey,
    },
}

impl Workspace {
    pub fn fork_picker(&self) -> Option<&ForkPicker> {
        self.fork.as_ref().filter(|p| {
            self.selected == Some(p.index)
                && self.navigation == p.navigation
                && self.sessions[p.index].key().as_ref() == Some(&p.source)
        })
    }

    pub fn dismiss_fork(&mut self) {
        self.fork = None;
    }

    pub fn reload_history_action(&mut self, index: usize) {
        if let Some(p) = self
            .fork_picker()
            .filter(|p| p.index == index && !p.creating())
        {
            self.open_history_action(index, p.action, p.target_entry.clone());
        }
    }

    fn fork_source(&self, index: usize) -> Result<SessionKey, &'static str> {
        let s = &self.sessions[index];
        let key = s
            .key()
            .ok_or("Send a message before choosing a conversation boundary.")?;
        if self.selected != Some(index)
            || s.archived
            || s.forgotten
            || s.deleting
            || s.archiving
            || s.relocation.pending()
            || !self.project_is_open(&s.scope.project)
        {
            return Err("Open an active conversation to use its history.");
        }
        if s.disconnected {
            return Err("Reconnect the session before using its history.");
        }
        if self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Fork(Request::Create { index: i, .. } | Request::Rewind {index:i,..}) if *i == index))
        {
            return Err("A history operation is already in progress.");
        }
        if s.working()
            || s.stopping
            || s.commanding
            || s.control_busy()
            || s.generating
            || !s.shells.is_empty()
            || crate::application::subagents::running(&s.conversation.entries) > 0
            || !s.queue.is_empty()
            || !s.inbox.items.is_empty()
            || self.needs_input(s)
        {
            return Err("Finish active work and resolve queued prompts or input requests before changing history.");
        }
        Ok(key)
    }

    pub fn open_fork(&mut self, index: usize) -> bool {
        self.open_history_action(index, HistoryAction::Fork, None)
    }

    pub fn open_history_action(
        &mut self,
        index: usize,
        action: HistoryAction,
        target_entry: Option<String>,
    ) -> bool {
        let source = match self.fork_source(index) {
            Ok(key) => key,
            Err(error) => {
                self.sessions[index].error = Some(error.into());
                return false;
            }
        };
        let command = if source.scope.provider == Provider::Codex && action != HistoryAction::Fork {
            Command::CodexControlHistory(source.clone(), action)
        } else {
            Command::History(source.clone(), None)
        };
        let request = self.request(command, Pending::Fork(Request::Page));
        self.sessions[index].error = None;
        self.fork = Some(ForkPicker {
            token: request,
            action,
            target_entry,
            cursors: Default::default(),
            index,
            source,
            navigation: self.navigation,
            phase: Phase::Reading(request),
            points: vec![],
            seen_turns: 0,
            error: None,
            cursor: None,
            newer_prompt: None,
            latest_prompt: None,
            tip: None,
        });
        true
    }

    pub fn older_fork_points(&mut self) {
        let Some(picker) = self
            .fork_picker()
            .filter(|p| matches!(p.phase, Phase::Ready))
        else {
            return;
        };
        let Some(cursor) = picker.cursor.clone() else {
            return;
        };
        let command = Command::History(picker.source.clone(), Some(cursor));
        let id = self.request(command, Pending::Fork(Request::Page));
        let picker = self.fork.as_mut().unwrap();
        picker.phase = Phase::Reading(id);
        picker.error = None;
    }

    pub fn fork_at(&mut self, boundary: &ForkBoundary) {
        let Some(picker) = self.fork_picker().filter(|p| {
            matches!(p.phase, Phase::Ready)
                && p.points.iter().any(|point| &point.boundary == boundary)
        }) else {
            return;
        };
        let index = picker.index;
        let action = picker.action;
        let draft = picker
            .points
            .iter()
            .find(|p| &p.boundary == boundary)
            .and_then(|p| p.draft.clone());
        let source = match self.fork_source(index) {
            Ok(key) => key,
            Err(error) => {
                self.fork.as_mut().unwrap().error = Some(error.into());
                return;
            }
        };
        if action == HistoryAction::Rewind {
            let files = match boundary {
                ForkBoundary::OpenCodeBefore(id) => {
                    let latest = self
                        .fork
                        .as_ref()
                        .unwrap()
                        .latest_prompt
                        .clone()
                        .expect("user checkpoint");
                    FilePreview::Loading(self.request(
                        Command::CheckpointFiles(source, id.clone(), latest),
                        Pending::Fork(Request::Files),
                    ))
                }
                _ => FilePreview::ConversationOnly,
            };
            self.fork.as_mut().unwrap().phase = Phase::Confirm(boundary.clone(), files);
            return;
        }
        let draft = match draft.transpose() {
            Ok(draft) => draft,
            Err(error) => {
                self.fork.as_mut().unwrap().error = Some(error);
                return;
            }
        };
        let editing = draft.is_some();
        let command = match draft {
            Some(draft) => Command::EditBranch(
                source.clone(),
                boundary.clone(),
                draft.text,
                draft.attachments,
            ),
            None => Command::Fork(source.clone(), boundary.clone()),
        };
        let request = self.request(
            command,
            Pending::Fork(Request::Create {
                index,
                source,
                editing,
            }),
        );
        let picker = self.fork.as_mut().unwrap();
        picker.error = None;
        picker.phase = Phase::Creating(request);
    }

    pub fn confirm_rewind(&mut self, files: bool) {
        let Some(p) = self.fork_picker() else { return };
        let Some(point) = p.confirmation() else {
            return;
        };
        if files
            && !matches!(p.file_preview(),Some(FilePreview::Available(paths)) if !paths.is_empty())
        {
            return;
        }
        let before = match &point.boundary {
            ForkBoundary::CodexBefore(id) if !files => id.clone(),
            ForkBoundary::OpenCodeBefore(id) => id.clone(),
            _ => return,
        };
        let index = p.index;
        let tip = p.tip.clone().expect("checkpoint from history");
        let admission = self.fork_source(index).and_then(|key| {
            if self.sessions[index].loading {
                Err("Wait for history to finish loading, then confirm again.")
            } else {
                Ok(key)
            }
        });
        let source = match admission {
            Ok(key) => key,
            Err(e) => {
                self.fork.as_mut().unwrap().error = Some(e.into());
                return;
            }
        };
        self.sessions[index].controls = super::controls::Status::Rewinding;
        let id = self.request(
            Command::Rewind {
                key: source.clone(),
                before,
                tip,
                files,
            },
            Pending::Fork(Request::Rewind { index, source }),
        );
        let p = self.fork.as_mut().unwrap();
        p.error = None;
        p.phase = Phase::Creating(id);
    }

    pub(super) fn complete_fork(
        &mut self,
        request: u64,
        pending: Request,
        result: Result<Reply, String>,
    ) {
        match pending {
            Request::Files => {
                if !self.fork_picker().is_some_and(
                    |p| matches!(p.file_preview(),Some(FilePreview::Loading(id)) if *id==request),
                ) {
                    return;
                }
                if let Phase::Confirm(_, files) = &mut self.fork.as_mut().unwrap().phase {
                    *files = match result {
                        Ok(Reply::CheckpointFiles(paths)) => FilePreview::Available(paths),
                        other => FilePreview::Unavailable(
                            other
                                .err()
                                .unwrap_or_else(|| "Unexpected snapshot response".into()),
                        ),
                    };
                }
            }
            Request::Page => {
                if !self
                    .fork_picker()
                    .is_some_and(|p| matches!(p.phase, Phase::Reading(id) if id == request))
                {
                    return;
                }
                let picker = self.fork.as_mut().unwrap();
                picker.phase = Phase::Ready;
                match result {
                    Ok(Reply::History(page)) => {
                        picker.replace_page(page);
                        if picker.target_entry.is_some()
                            && picker.points.is_empty()
                            && picker.has_older()
                            && picker.error.is_none()
                        {
                            self.older_fork_points();
                        }
                    }
                    other => {
                        picker.error = Some(
                            other
                                .err()
                                .unwrap_or_else(|| "Unexpected history response".into()),
                        )
                    }
                }
            }
            Request::Rewind { index, source } => {
                if self.sessions[index].key().as_ref() != Some(&source)
                    || self.sessions[index].forgotten
                {
                    return;
                }
                let foreground = self
                    .fork_picker()
                    .is_some_and(|p| matches!(p.phase, Phase::Creating(id) if id==request));
                let session = &mut self.sessions[index];
                session.controls = super::controls::Status::Idle;
                match result {
                    Ok(Reply::Done) => {
                        // Normal history hydration merges older rows. After a destructive
                        // rewind, rebuild from the provider so removed turns cannot survive.
                        session.conversation = Default::default();
                        session.revision += 1;
                        session.loaded = false;
                        session.next_cursor = None;
                        session.reading = -1.0;
                        session.controls =
                            super::controls::Status::Finished("Conversation rewound.".into());
                        if foreground {
                            self.dismiss_fork();
                        }
                        self.refresh(index);
                    }
                    other => {
                        let error = other
                            .err()
                            .unwrap_or_else(|| "Unexpected rewind response".into());
                        if foreground {
                            let p = self.fork.as_mut().unwrap();
                            p.phase = Phase::Ready;
                            p.error = Some(error);
                        } else {
                            session.error = Some(error);
                        }
                    }
                }
            }
            Request::Create {
                index,
                source,
                editing,
            } => {
                let foreground = self
                    .fork_picker()
                    .is_some_and(|p| matches!(p.phase, Phase::Creating(id) if id == request));
                let result = result.and_then(|reply| match reply {
                    Reply::Forked { source, child } if !editing => Ok((source, child, None)),
                    Reply::EditableBranch {
                        source,
                        child,
                        text,
                        attachments,
                    } if editing => Ok((source, child, Some(prompt::Draft { text, attachments }))),
                    _ => Err("Unexpected branch response".into()),
                });
                let (child, draft) = match result {
                    Ok((owner, child, draft))
                        if owner == source
                            && child.key.scope == source.scope
                            && !child.key.id.is_empty()
                            && child.key.id != source.id =>
                    {
                        (child, draft)
                    }
                    other => {
                        let error = other
                            .err()
                            .unwrap_or_else(|| "Provider returned a different fork".into());
                        if foreground {
                            let picker = self.fork.as_mut().unwrap();
                            picker.phase = Phase::Ready;
                            picker.error = Some(error);
                        } else if self.sessions[index].key().as_ref() == Some(&source)
                            && !self.sessions[index].forgotten
                        {
                            self.sessions[index].error = Some(error);
                        }
                        return;
                    }
                };
                // Discovery may have already created an editor for the child. Never replace it.
                let child_index = match self
                    .sessions
                    .iter()
                    .position(|s| s.key().as_ref() == Some(&child.key))
                {
                    Some(i) => i,
                    None => {
                        let mut s = Session::new(source.scope.clone());
                        s.remote_id = Some(child.key.id.clone());
                        s.loaded = false;
                        update_summary(&mut s, child, request);
                        self.sessions.push(s);
                        let i = self.sessions.len() - 1;
                        if self.project_is_open(&source.scope.project) {
                            self.detail_queue.push(i);
                        }
                        i
                    }
                };
                if let Some(draft) = draft {
                    let child = &mut self.sessions[child_index];
                    if !child.forgotten
                        && !child.archived
                        && child.draft.is_empty()
                        && child.attachments.is_empty()
                        && !child.working()
                    {
                        child.draft = draft.text;
                        child.attachments = draft.attachments;
                        child.completion_caret = Some(child.draft.len());
                    } else if !child.forgotten {
                        // Never replace an existing editor or a previous unsent message.
                        self.sessions[index].error=Some("Branch created, but its draft was already in use. The original prompt is unchanged.".into());
                    }
                }
                if foreground {
                    self.dismiss_fork();
                    if !self.closed_since(&source.scope.project, request)
                        && !self.sessions[child_index].archived
                        && !self.sessions[child_index].forgotten
                    {
                        self.select(child_index);
                    }
                }
            }
        }
    }
}
