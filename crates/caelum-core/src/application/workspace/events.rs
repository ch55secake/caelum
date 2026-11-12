use super::{Pending, Workspace};
use crate::application::{Command, Event, Notification, Provider, Scope, SessionKey, Task};
use serde_json::Value;

impl Workspace {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Branch(update) => self.branch_update(update),
            Event::Checkout(project) => self.checkout_changed(&project),
            Event::Completed(id, result) => {
                if let Some(pending) = self.pending.remove(&id) {
                    let activation = matches!(pending, Pending::Load)
                        .then(|| self.notifications.take(self.navigation))
                        .flatten();
                    self.complete(id, pending, result);
                    if self.store_ready {
                        if let Some(target) = activation {
                            self.activate_notification(target);
                        }
                    }
                    let ready: Vec<_> = self
                        .deferred_lists
                        .keys()
                        .filter(|scope| !self.creating(scope))
                        .cloned()
                        .collect();
                    for scope in ready {
                        if let Some((request, summaries)) = self.deferred_lists.remove(&scope) {
                            self.apply_summaries(request, &scope, summaries, false);
                        }
                    }
                    for index in 0..self.sessions.len() {
                        self.advance_queue(index);
                    }
                    self.schedule_details();
                }
            }
            Event::Approvals(prompts) => {
                if let Some(prompt) = prompts.iter().find(|p| {
                    self.preferences.input_needed
                        && !self.approvals.contains(p)
                        && self.preferences.should_notify(self.visible_request(
                            &p.provider,
                            &p.project_path,
                            p.session_id.as_deref(),
                        ))
                }) {
                    self.request(
                        Command::Notify(Notification {
                            title: "Caelum · Approval needed".into(),
                            body: "A session needs your permission".into(),
                            target: notification_target(
                                &prompt.provider,
                                &prompt.project_path,
                                prompt.session_id.as_deref(),
                            ),
                        }),
                        Pending::Notify,
                    );
                }
                self.approvals = prompts;
            }
            Event::Questions(prompts) => {
                if let Some(prompt) = prompts.iter().find(|p| {
                    self.preferences.input_needed
                        && !self.questions.contains(p)
                        && self.preferences.should_notify(self.visible_request(
                            &p.provider,
                            &p.project_path,
                            Some(&p.session_id),
                        ))
                }) {
                    self.request(
                        Command::Notify(Notification {
                            title: "Caelum · Question".into(),
                            body: "A session needs your answer".into(),
                            target: notification_target(
                                &prompt.provider,
                                &prompt.project_path,
                                Some(&prompt.session_id),
                            ),
                        }),
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

    pub(super) fn branch_update(&mut self, update: caelum_protocol::SessionBranchUpdate) {
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
        if matches!(method, "session.deleted" | "thread/deleted") {
            let deleted = match provider {
                Provider::Codex => data["threadId"].as_str(),
                Provider::OpenCode => data["sessionID"].as_str(),
            };
            if let Some(id) = deleted {
                let indices = self
                    .sessions
                    .iter()
                    .enumerate()
                    .filter(|(_, session)| {
                        session.scope.project == path
                            && session.scope.provider == provider
                            && session.remote_id.as_deref() == Some(id)
                    })
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>();
                for index in indices {
                    self.forget_deleted_session(index);
                }
            }
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
        let mut titles_completed = Vec::new();
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
            if matches!(method, "turn/started" | "session.execution.started") {
                let agent = if session.inflight_prompt.is_some() {
                    session.inflight_options.agent.clone()
                } else {
                    session.options.agent.clone()
                };
                session.conversation.provider_started_with_agent(
                    agent,
                    crate::application::conversation::provider_started_at(data),
                );
            }
            session.conversation.apply(provider, method, data);
            if was_working != session.working() {
                session.record_activity(
                    data["_created"]
                        .as_u64()
                        .unwrap_or_else(crate::application::presentation::now),
                );
            }
            if provider == Provider::OpenCode && method == "session.execution.succeeded" {
                session.title_state.completed_turn();
                titles_completed.push(index);
            }
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
                session.title_state.barrier = self.next_request;
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
                    Command::Notify(Notification {
                        title: "Caelum · Session finished".into(),
                        body: self.sessions[index].title.clone(),
                        target: self.sessions[index].key(),
                    }),
                    Pending::Notify,
                );
            }
            self.refresh(index);
        }
        for index in titles_completed {
            self.refresh_title(index);
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
                    Command::Notify(Notification {
                        title: "Caelum · Background command finished".into(),
                        body: self.sessions[index].title.clone(),
                        target: self.sessions[index].key(),
                    }),
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

fn notification_target(provider: &str, project: &str, session: Option<&str>) -> Option<SessionKey> {
    Some(SessionKey {
        scope: Scope {
            project: project.into(),
            provider: Provider::ALL.into_iter().find(|p| p.as_str() == provider)?,
        },
        id: session?.into(),
    })
}
