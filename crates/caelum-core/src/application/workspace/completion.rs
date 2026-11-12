use super::{titles::Refresh, update_summary, Pending, Session, Workspace};
use crate::application::{
    Attachment, Command, Provider, QueuedPrompt, Reply, Scope, Task, TaskReply,
};

impl Workspace {
    pub(super) fn complete(
        &mut self,
        request: u64,
        pending: Pending,
        result: Result<Reply, String>,
    ) {
        if let Pending::RegenerateTitle(index, key) = pending {
            // The provider publishes the new title through session.renamed.
            // A failed request must not disturb chat, drafts or queued work.
            if self.sessions[index].key().as_ref() == Some(&key) {
                if matches!(result, Ok(Reply::Done)) {
                    self.sessions[index]
                        .title_state
                        .refreshed(crate::application::presentation::now());
                }
                self.refresh_title(index, Refresh::Automatic);
            }
            return;
        }
        if let Pending::Attachment(index, name) = pending {
            self.sessions[index].attaching = self.sessions[index].attaching.saturating_sub(1);
            let attachment = match result {
                Ok(Reply::Task(TaskReply::File(file))) => {
                    Attachment::new(file.path, file.content, None, file.truncated)
                }
                Ok(Reply::Task(TaskReply::Image(data))) => {
                    let mut attachment = Attachment::new(name, String::new(), None, false);
                    attachment.kind = crate::application::AttachmentKind::Image;
                    attachment.content = data;
                    attachment
                }
                result => {
                    self.sessions[index].attachment_error(
                        name,
                        result
                            .err()
                            .unwrap_or_else(|| "Unexpected attachment response".into()),
                    );
                    return;
                }
            };
            self.attach(index, attachment);
            return;
        }
        if let Pending::Mention {
            index,
            side,
            name,
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
                result => {
                    let error = result
                        .err()
                        .unwrap_or_else(|| "Unexpected context response".into());
                    if side {
                        session.side_error = Some(error);
                    } else {
                        session.attachment_error(name, error);
                    }
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
            self.complete_task(request, task, result);
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
                    Pending::Task(_)
                    | Pending::Mention { .. }
                    | Pending::Attachment(..)
                    | Pending::RegenerateTitle(..) => {
                        unreachable!()
                    }
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
                            self.archive_clock =
                                Some((crate::application::presentation::now(), 8_000));
                        }
                    }
                    Pending::DeleteSession(index) => {
                        self.sessions[index].deleting = false;
                        self.sessions[index].error = Some(error);
                    }
                    Pending::Details(_) => {}
                    Pending::Open(_, _) => {
                        if request == self.latest_open {
                            self.opening = false;
                            self.error = Some(error);
                        }
                    }
                    Pending::List(scope)
                    | Pending::ListArchived(scope)
                    | Pending::Choices(scope) => {
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
                                session.last_activity = saved.last_activity;
                                session.title_state.turns = saved.title_refresh_turns;
                                session.title_state.refreshed_at = saved.title_refreshed_at;
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
                // Lists issued while the mutation was in flight can also be stale.
                self.sessions[index].archive_barrier = self.next_request;
                if archived {
                    let owns_notice = self.archived_notice(index, revision);
                    if self.selected == Some(index) {
                        let replacement = self.select_archive_replacement(index);
                        if owns_notice {
                            self.archive_replacement = Some(replacement);
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
            (Pending::DeleteSession(index), Reply::Done) => {
                self.forget_deleted_session(index);
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
                    self.deferred_lists.insert(scope, (request, summaries));
                } else {
                    self.apply_summaries(request, &scope, summaries, false);
                }
            }
            (Pending::ListArchived(scope), Reply::Sessions(summaries)) => {
                self.apply_summaries(request, &scope, summaries, true);
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
                update_summary(session, summary, request);
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
}
