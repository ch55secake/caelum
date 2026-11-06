use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use caelum_protocol::{
    ApprovalDecision, ApprovalPrompt, CodexEvent, OpenCodeEvent, ProjectInfo, ProviderChoices,
    QuestionPrompt, SessionHistoryPage,
};

use crate::{
    approvals::ApprovalBroker, codex::CodexProvider, opencode::OpenCodeProvider,
    processes::ProcessRegistry, projects::Project, questions::QuestionBroker,
};

use super::{Options, Provider, Scope, SessionKey, Summary};

/// Shared by desktop adapters; constructing this does not start a provider.
#[derive(Clone)]
pub struct ProviderRuntime {
    pub codex: CodexProvider,
    pub opencode: OpenCodeProvider,
    pub approvals: ApprovalBroker,
    pub questions: QuestionBroker,
    pub processes: Arc<ProcessRegistry>,
}

impl Default for ProviderRuntime {
    fn default() -> Self {
        let processes = Arc::new(ProcessRegistry::default());
        let approvals = ApprovalBroker::default();
        let questions = QuestionBroker::default();
        Self {
            codex: CodexProvider::new(processes.clone(), approvals.clone(), questions.clone()),
            opencode: OpenCodeProvider::new(
                processes.clone(),
                approvals.clone(),
                questions.clone(),
            ),
            processes,
            approvals,
            questions,
        }
    }
}

impl ProviderRuntime {
    pub fn shutdown(&self) {
        self.approvals.shutdown();
        // Close registration first: an in-flight connection cannot leak a child
        // after shutdown, and terminating children unblocks protocol handshakes.
        self.processes.shutdown();
        self.codex.shutdown();
        self.opencode.shutdown();
    }

    fn execute(&self, command: &Command) -> Result<Reply, String> {
        let result = match command {
            Command::Open(path) => {
                return Project::open(path)
                    .map(|project| Reply::Project(project.info()))
                    .map_err(|e| e.to_string());
            }
            Command::List(scope) => {
                let summaries = match scope.provider {
                    Provider::Codex => self
                        .codex
                        .list_sessions(&scope.project)
                        .map_err(|e| e.to_string())?
                        .threads
                        .into_iter()
                        .map(|s| Summary {
                            key: SessionKey {
                                scope: scope.clone(),
                                id: s.id,
                            },
                            title: s.name.filter(|v| !v.is_empty()).unwrap_or(s.preview),
                            working: s.is_working,
                            options: Options::default(),
                        })
                        .collect(),
                    Provider::OpenCode => self
                        .opencode
                        .list_sessions(&scope.project)
                        .map_err(|e| e.to_string())?
                        .sessions
                        .into_iter()
                        .map(|s| Summary {
                            key: SessionKey {
                                scope: scope.clone(),
                                id: s.id,
                            },
                            title: s.title.unwrap_or_default(),
                            working: s.is_working,
                            options: Options {
                                model: s.model,
                                effort: s.effort,
                                agent: s.agent,
                            },
                        })
                        .collect(),
                };
                return Ok(Reply::Sessions(summaries));
            }
            Command::Choices(scope) => {
                return match scope.provider {
                    Provider::Codex => self
                        .codex
                        .choices(&scope.project)
                        .map_err(|e| e.to_string()),
                    Provider::OpenCode => self
                        .opencode
                        .choices(&scope.project)
                        .map_err(|e| e.to_string()),
                }
                .map(Reply::Choices);
            }
            Command::Start(scope, options) => {
                return match scope.provider {
                    Provider::Codex => self
                        .codex
                        .start_session_with_model(&scope.project, options.model.as_deref())
                        .map(|s| Summary {
                            key: SessionKey {
                                scope: scope.clone(),
                                id: s.thread.id,
                            },
                            title: s.thread.name.unwrap_or(s.thread.preview),
                            working: s.thread.is_working,
                            options: Options {
                                model: s.model,
                                effort: s.effort,
                                agent: None,
                            },
                        })
                        .map_err(|e| e.to_string()),
                    Provider::OpenCode => self
                        .opencode
                        .start_session_with_options(
                            &scope.project,
                            options.model.as_deref(),
                            options.agent.as_deref(),
                        )
                        .map(|s| Summary {
                            key: SessionKey {
                                scope: scope.clone(),
                                id: s.session.id,
                            },
                            title: s.session.title.unwrap_or_default(),
                            working: s.session.is_working,
                            options: Options {
                                model: s.session.model,
                                effort: s.session.effort,
                                agent: s.session.agent,
                            },
                        })
                        .map_err(|e| e.to_string()),
                }
                .map(Reply::Session);
            }
            Command::Resume(key) => {
                return match key.scope.provider {
                    Provider::Codex => self
                        .codex
                        .resume_session(&key.scope.project, &key.id)
                        .map(|s| Summary {
                            key: key.clone(),
                            title: s.thread.name.unwrap_or(s.thread.preview),
                            working: s.thread.is_working,
                            options: Options {
                                model: s.model,
                                effort: s.effort,
                                agent: None,
                            },
                        })
                        .map_err(|e| e.to_string()),
                    Provider::OpenCode => self
                        .opencode
                        .resume_session(&key.scope.project, &key.id)
                        .map(|s| Summary {
                            key: key.clone(),
                            title: s.session.title.unwrap_or_default(),
                            working: s.session.is_working,
                            options: Options {
                                model: s.session.model,
                                effort: s.session.effort,
                                agent: s.session.agent,
                            },
                        })
                        .map_err(|e| e.to_string()),
                }
                .map(Reply::Session);
            }
            Command::History(key, cursor) => {
                return match key.scope.provider {
                    Provider::Codex => self
                        .codex
                        .session_history(&key.scope.project, &key.id, cursor.as_deref())
                        .map_err(|e| e.to_string()),
                    Provider::OpenCode => self
                        .opencode
                        .session_history(&key.scope.project, &key.id, cursor.as_deref())
                        .map_err(|e| e.to_string()),
                }
                .map(Reply::History);
            }
            Command::Send(key, prompt, options) => match key.scope.provider {
                Provider::Codex => self
                    .codex
                    .send_prompt_with_options(
                        &key.scope.project,
                        &key.id,
                        prompt,
                        options.model.as_deref(),
                        options.effort.as_deref(),
                    )
                    .map_err(|e| e.to_string()),
                Provider::OpenCode => {
                    if let Some(agent) = &options.agent {
                        self.opencode
                            .select_agent(&key.scope.project, &key.id, agent)
                            .map_err(|e| e.to_string())?;
                    }
                    self.opencode
                        .send_prompt_with_options(
                            &key.scope.project,
                            &key.id,
                            prompt,
                            None,
                            options.model.as_deref(),
                            options.effort.as_deref(),
                        )
                        .map_err(|e| e.to_string())
                }
            },
            Command::Cancel(key) => match key.scope.provider {
                Provider::Codex => self
                    .codex
                    .cancel_turn(&key.scope.project, &key.id)
                    .map_err(|e| e.to_string()),
                Provider::OpenCode => self
                    .opencode
                    .cancel_turn(&key.scope.project, &key.id)
                    .map_err(|e| e.to_string()),
            },
            Command::Approve(prompt, decision) => self
                .approvals
                .resolve(
                    &prompt.provider,
                    &prompt.project_path,
                    &prompt.request_id,
                    *decision,
                )
                .map_err(|e| e.to_string()),
            Command::Answer(prompt, answer) => self.questions.resolve(
                &prompt.provider,
                &prompt.project_path,
                &prompt.session_id,
                &prompt.request_id,
                answer.clone(),
            ),
        };
        result.map(|()| Reply::Done)
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Open(String),
    List(Scope),
    Choices(Scope),
    Start(Scope, Options),
    Resume(SessionKey),
    History(SessionKey, Option<String>),
    Send(SessionKey, String, Options),
    Cancel(SessionKey),
    Approve(ApprovalPrompt, ApprovalDecision),
    Answer(QuestionPrompt, Option<crate::questions::Answer>),
}

#[derive(Debug)]
pub enum Reply {
    Project(ProjectInfo),
    Sessions(Vec<Summary>),
    Session(Summary),
    Choices(ProviderChoices),
    History(SessionHistoryPage),
    Done,
}

#[derive(Debug)]
pub enum Event {
    Completed(u64, Result<Reply, String>),
    Codex(CodexEvent),
    OpenCode(OpenCodeEvent),
    Approvals(Vec<ApprovalPrompt>),
    Questions(Vec<QuestionPrompt>),
}

type Execute = Arc<dyn Fn(&Command) -> Result<Reply, String> + Send + Sync>;

/// Background I/O with a bounded UI inbox. The UI drains a batch once per frame;
/// event arrival wakes it, so idle windows need no polling timer.
pub struct Runtime {
    providers: Option<ProviderRuntime>,
    execute: Execute,
    inbox: Option<Receiver<Event>>,
    sender: SyncSender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
    closed: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
    forwarders: Vec<JoinHandle<()>>,
}

impl Runtime {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self::with_providers(ProviderRuntime::default(), wake)
    }

    pub fn with_providers(
        providers: ProviderRuntime,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let backend = providers.clone();
        let mut runtime = Self::with_executor(
            Arc::new(move |command| backend.execute(command)),
            Arc::new(wake),
        );
        runtime.forward(providers.codex.subscribe(), Event::Codex);
        runtime.forward(providers.opencode.subscribe(), Event::OpenCode);
        runtime.forward(providers.approvals.subscribe(), Event::Approvals);
        runtime.forward(providers.questions.subscribe(), Event::Questions);
        runtime.providers = Some(providers);
        runtime
    }

    pub(super) fn with_executor(execute: Execute, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (sender, inbox) = mpsc::sync_channel(1024);
        Self {
            providers: None,
            execute,
            inbox: Some(inbox),
            sender,
            wake,
            closed: Arc::new(AtomicBool::new(false)),
            workers: Vec::new(),
            forwarders: Vec::new(),
        }
    }

    fn forward<T: Send + 'static>(&mut self, receiver: Receiver<T>, event: fn(T) -> Event) {
        let sender = self.sender.clone();
        let wake = self.wake.clone();
        let closed = self.closed.clone();
        self.forwarders.push(thread::spawn(move || {
            while !closed.load(Ordering::Acquire) {
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(value) => {
                        if sender.send(event(value)).is_err() {
                            break;
                        }
                        wake();
                    }
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        }));
    }

    pub fn submit(&mut self, id: u64, command: Command) -> Result<(), String> {
        if self.closed.load(Ordering::Acquire) {
            return Err("Application is shutting down".into());
        }
        self.workers.retain(|worker| !worker.is_finished());
        if self.workers.len() >= 64 {
            return Err("Too many pending requests; try again shortly".into());
        }
        let execute = self.execute.clone();
        let sender = self.sender.clone();
        let wake = self.wake.clone();
        let worker = thread::Builder::new()
            .name(format!("caelum-request-{id}"))
            .spawn(move || {
                let result = execute(&command);
                if sender.send(Event::Completed(id, result)).is_ok() {
                    wake();
                }
            })
            .map_err(|e| e.to_string())?;
        self.workers.push(worker);
        Ok(())
    }

    pub fn drain(&self) -> Vec<Event> {
        let events: Vec<_> = self
            .inbox
            .iter()
            .flat_map(|inbox| inbox.try_iter())
            .take(256)
            .collect();
        if events.len() == 256 {
            (self.wake)();
        }
        events
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        self.inbox.take(); // Release any producers waiting on a full inbox.
        if let Some(providers) = &self.providers {
            providers.shutdown();
        }
        for worker in self.forwarders.drain(..).chain(self.workers.drain(..)) {
            let _ = worker.join();
        }
    }
}
