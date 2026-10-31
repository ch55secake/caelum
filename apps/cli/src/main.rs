use std::{
    collections::HashSet,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        mpsc::{Receiver, RecvTimeoutError},
        Arc,
    },
    time::Duration,
};

#[cfg(target_os = "macos")]
use std::process::Command;

use caelum_core::{
    approvals::ApprovalBroker, codex::CodexProvider, opencode::OpenCodeProvider,
    processes::ProcessRegistry, projects::Project,
};
use caelum_protocol::{ApprovalDecision, ApprovalPrompt, CodexEvent, OpenCodeEvent};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "caelum", version, about = "Your code. New horizons.")]
struct Cli {
    /// Agent provider to use for terminal workflows.
    #[arg(long, value_enum, global = true, default_value_t = ProviderKind::Codex)]
    provider: ProviderKind,
    /// Open this project in the desktop app (defaults to the current directory).
    #[arg(value_name = "PROJECT")]
    project: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<CommandKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ProviderKind {
    Codex,
    Opencode,
}

impl ProviderKind {
    fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Opencode => "opencode",
        }
    }
}

#[derive(Debug, Subcommand)]
enum CommandKind {
    /// Start an interactive agent session.
    Chat {
        #[arg(value_name = "PROJECT")]
        project: Option<PathBuf>,
        /// Resume an existing provider session instead of starting a new one.
        #[arg(short, long, value_name = "ID")]
        session: Option<String>,
    },
    /// Send a task to an agent and print its response.
    Run {
        prompt: String,
        #[arg(short, long, value_name = "PROJECT")]
        project: Option<PathBuf>,
        /// Continue an existing provider session.
        #[arg(short, long, value_name = "ID")]
        session: Option<String>,
    },
    /// List provider sessions for a project.
    Sessions {
        #[arg(value_name = "PROJECT")]
        project: Option<PathBuf>,
    },
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("caelum: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Some(CommandKind::Chat { project, session }) => {
            chat(cli.provider, open_project(project)?, session.as_deref())
        }
        Some(CommandKind::Run {
            prompt,
            project,
            session,
        }) => run_task(
            cli.provider,
            open_project(project)?,
            &prompt,
            session.as_deref(),
        ),
        Some(CommandKind::Sessions { project }) => {
            list_sessions(cli.provider, open_project(project)?)
        }
        None => launch_project_in_desktop(open_project(cli.project)?),
    }
}

fn open_project(path: Option<PathBuf>) -> Result<Project, Box<dyn std::error::Error>> {
    let path = match path {
        Some(path) => path,
        None => std::env::current_dir()
            .map_err(|error| format!("could not determine the current directory: {error}"))?,
    };
    Ok(Project::open(path)?)
}

fn launch_project_in_desktop(project: Project) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("open")
            .args(["-a", "Caelum", "--args"])
            .arg(project.path())
            .output()
            .map_err(|error| {
                format!("could not launch Caelum.app: {error}. Install the desktop app first.")
            })?;
        if !output.status.success() {
            let details = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(format!(
                "could not open the Caelum desktop app{}; install Caelum.app or set up the desktop build",
                if details.is_empty() { String::new() } else { format!(": {details}") }
            )
            .into());
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = project;
        Err("desktop project launching is currently supported on macOS only; use `caelum chat` or `caelum run` for terminal workflows".into())
    }
}

struct AgentProviders {
    codex: CodexProvider,
    opencode: OpenCodeProvider,
    approvals: ApprovalBroker,
    processes: Arc<ProcessRegistry>,
}

impl AgentProviders {
    fn new() -> Self {
        let processes = Arc::new(ProcessRegistry::default());
        let approvals = ApprovalBroker::default();
        Self {
            codex: CodexProvider::new(
                Arc::clone(&processes),
                approvals.clone(),
                caelum_core::questions::QuestionBroker::disabled(),
            ),
            opencode: OpenCodeProvider::new(
                Arc::clone(&processes),
                approvals.clone(),
                caelum_core::questions::QuestionBroker::disabled(),
            ),
            approvals,
            processes,
        }
    }

    fn subscribe(&self, provider: ProviderKind) -> EventReceiver {
        match provider {
            ProviderKind::Codex => EventReceiver::Codex(self.codex.subscribe()),
            ProviderKind::Opencode => EventReceiver::OpenCode(self.opencode.subscribe()),
        }
    }

    fn start_session(
        &self,
        provider: ProviderKind,
        project: &Project,
    ) -> Result<Session, Box<dyn std::error::Error>> {
        match provider {
            ProviderKind::Codex => {
                let result = self.codex.start_session(project.path())?;
                Ok(Session {
                    id: result.thread.id,
                    title: result.thread.name,
                })
            }
            ProviderKind::Opencode => {
                let result = self.opencode.start_session(project.path())?;
                Ok(Session {
                    id: result.session.id,
                    title: result.session.title,
                })
            }
        }
    }

    fn resume_session(
        &self,
        provider: ProviderKind,
        project: &Project,
        session_id: &str,
    ) -> Result<Session, Box<dyn std::error::Error>> {
        match provider {
            ProviderKind::Codex => {
                let result = self.codex.resume_session(project.path(), session_id)?;
                Ok(Session {
                    id: result.thread.id,
                    title: result.thread.name,
                })
            }
            ProviderKind::Opencode => {
                let result = self.opencode.resume_session(project.path(), session_id)?;
                Ok(Session {
                    id: result.session.id,
                    title: result.session.title,
                })
            }
        }
    }

    fn send_prompt(
        &self,
        provider: ProviderKind,
        project: &Project,
        session_id: &str,
        prompt: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match provider {
            ProviderKind::Codex => self.codex.send_prompt(project.path(), session_id, prompt)?,
            ProviderKind::Opencode => {
                self.opencode
                    .send_prompt(project.path(), session_id, prompt)?
            }
        }
        Ok(())
    }

    fn list_sessions(
        &self,
        provider: ProviderKind,
        project: &Project,
    ) -> Result<Vec<Session>, Box<dyn std::error::Error>> {
        match provider {
            ProviderKind::Codex => Ok(self
                .codex
                .list_sessions(project.path())?
                .threads
                .into_iter()
                .map(|thread| Session {
                    id: thread.id,
                    title: thread
                        .name
                        .or_else(|| (!thread.preview.is_empty()).then_some(thread.preview)),
                })
                .collect()),
            ProviderKind::Opencode => Ok(self
                .opencode
                .list_sessions(project.path())?
                .sessions
                .into_iter()
                .map(|session| Session {
                    id: session.id,
                    title: session.title,
                })
                .collect()),
        }
    }
}

impl Drop for AgentProviders {
    fn drop(&mut self) {
        self.approvals.shutdown();
        self.codex.shutdown();
        self.opencode.shutdown();
        self.processes.shutdown();
    }
}

struct Session {
    id: String,
    title: Option<String>,
}

enum EventReceiver {
    Codex(Receiver<CodexEvent>),
    OpenCode(Receiver<OpenCodeEvent>),
}

impl EventReceiver {
    fn recv_timeout(&self, timeout: Duration) -> Result<ProviderEvent, RecvTimeoutError> {
        match self {
            Self::Codex(receiver) => receiver.recv_timeout(timeout).map(ProviderEvent::Codex),
            Self::OpenCode(receiver) => receiver.recv_timeout(timeout).map(ProviderEvent::OpenCode),
        }
    }
}

enum ProviderEvent {
    Codex(CodexEvent),
    OpenCode(OpenCodeEvent),
}

fn chat(
    provider: ProviderKind,
    project: Project,
    resume_id: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let providers = AgentProviders::new();
    let events = providers.subscribe(provider);
    let approval_updates = providers.approvals.subscribe();
    let session = match resume_id {
        Some(id) => providers.resume_session(provider, &project, id)?,
        None => providers.start_session(provider, &project)?,
    };
    print_session(provider, &session);

    let stdin = io::stdin();
    let mut input = String::new();
    loop {
        eprint!("\n> ");
        io::stderr().flush()?;
        input.clear();
        if stdin.read_line(&mut input)? == 0 {
            break;
        }
        let prompt = input.trim();
        if prompt.is_empty() {
            continue;
        }
        if matches!(prompt, "/quit" | "/exit") {
            break;
        }
        providers.send_prompt(provider, &project, &session.id, prompt)?;
        match wait_for_turn(
            provider,
            &project,
            &session.id,
            &providers,
            &events,
            &approval_updates,
        )? {
            TurnStatus::Completed => {}
            TurnStatus::Failed => eprintln!("The provider reported that this turn failed."),
            TurnStatus::Interrupted => eprintln!("Turn interrupted."),
        }
    }
    Ok(())
}

fn run_task(
    provider: ProviderKind,
    project: Project,
    prompt: &str,
    resume_id: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let providers = AgentProviders::new();
    let events = providers.subscribe(provider);
    let approval_updates = providers.approvals.subscribe();
    let session = match resume_id {
        Some(id) => providers.resume_session(provider, &project, id)?,
        None => providers.start_session(provider, &project)?,
    };
    print_session(provider, &session);
    providers.send_prompt(provider, &project, &session.id, prompt)?;
    match wait_for_turn(
        provider,
        &project,
        &session.id,
        &providers,
        &events,
        &approval_updates,
    )? {
        TurnStatus::Completed => Ok(()),
        TurnStatus::Failed => Err(format!("{} turn failed", provider.name()).into()),
        TurnStatus::Interrupted => Err(format!("{} turn was interrupted", provider.name()).into()),
    }
}

fn print_session(provider: ProviderKind, session: &Session) {
    match session
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
    {
        Some(title) => eprintln!("{} session: {} ({title})", provider.name(), session.id),
        None => eprintln!("{} session: {}", provider.name(), session.id),
    }
}

fn list_sessions(
    provider: ProviderKind,
    project: Project,
) -> Result<(), Box<dyn std::error::Error>> {
    let providers = AgentProviders::new();
    let sessions = providers.list_sessions(provider, &project)?;
    if sessions.is_empty() {
        println!(
            "No {} sessions for {}.",
            provider.name(),
            project.path().display()
        );
        return Ok(());
    }
    for session in sessions {
        match session.title.filter(|title| !title.trim().is_empty()) {
            Some(title) => println!("{}\t{}", session.id, title.replace('\n', " ")),
            None => println!("{}", session.id),
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnStatus {
    Completed,
    Failed,
    Interrupted,
}

fn wait_for_turn(
    provider: ProviderKind,
    project: &Project,
    session_id: &str,
    providers: &AgentProviders,
    events: &EventReceiver,
    approval_updates: &Receiver<Vec<ApprovalPrompt>>,
) -> Result<TurnStatus, Box<dyn std::error::Error>> {
    let mut handled_approvals = HashSet::new();
    let mut opencode_turn_started = false;
    loop {
        handle_approvals(
            approval_updates,
            &providers.approvals,
            project.path(),
            &mut handled_approvals,
        )?;
        match events.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                let outcome = event_outcome(&event, session_id, opencode_turn_started);
                opencode_turn_started |= outcome.started;
                for text in outcome.output {
                    print!("{text}");
                    io::stdout().flush()?;
                }
                for activity in outcome.activity {
                    eprintln!("\n[{activity}]");
                }
                if let Some(error) = outcome.error {
                    return Err(error.into());
                }
                if let Some(status) = outcome.complete {
                    println!();
                    return Ok(match status.as_str() {
                        "failed" => TurnStatus::Failed,
                        "interrupted" => TurnStatus::Interrupted,
                        _ => TurnStatus::Completed,
                    });
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(format!("{} event stream disconnected", provider.name()).into())
            }
        }
    }
}

fn handle_approvals(
    updates: &Receiver<Vec<ApprovalPrompt>>,
    broker: &ApprovalBroker,
    project_path: &Path,
    handled: &mut HashSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut latest = None;
    while let Ok(prompts) = updates.try_recv() {
        latest = Some(prompts);
    }
    let Some(prompts) = latest else {
        return Ok(());
    };
    for request in prompts
        .into_iter()
        .filter(|request| request.project_path == project_path.to_string_lossy())
    {
        let key = format!("{}:{}", request.provider, request.request_id);
        if !handled.insert(key) {
            continue;
        }
        eprintln!("\n{}", request.summary);
        if !request.description.is_empty() {
            eprintln!("{}", request.description);
        }
        eprint!("Allow this action once? [y/N] ");
        io::stderr().flush()?;
        let mut answer = String::new();
        let allow = io::stdin().read_line(&mut answer)? > 0
            && matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
        broker.resolve(
            &request.provider,
            &request.project_path,
            &request.request_id,
            if allow {
                ApprovalDecision::AllowOnce
            } else {
                ApprovalDecision::Deny
            },
        )?;
    }
    Ok(())
}

#[derive(Default)]
struct EventOutcome {
    output: Vec<String>,
    activity: Vec<String>,
    started: bool,
    complete: Option<String>,
    error: Option<String>,
}

fn event_outcome(
    event: &ProviderEvent,
    session_id: &str,
    opencode_turn_started: bool,
) -> EventOutcome {
    match event {
        ProviderEvent::Codex(event) => codex_event_outcome(event, session_id),
        ProviderEvent::OpenCode(event) => {
            opencode_event_outcome(event, session_id, opencode_turn_started)
        }
    }
}

fn codex_event_outcome(event: &CodexEvent, session_id: &str) -> EventOutcome {
    if event.method == "codex/processExited" {
        return EventOutcome {
            error: Some("Codex app-server exited unexpectedly".to_owned()),
            ..EventOutcome::default()
        };
    }
    let params = &event.params;
    if params.get("threadId").and_then(serde_json::Value::as_str) != Some(session_id) {
        return EventOutcome::default();
    }
    match event.method.as_str() {
        "item/agentMessage/delta" => EventOutcome {
            output: params
                .get("delta")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .into_iter()
                .collect(),
            ..EventOutcome::default()
        },
        "item/started" => {
            let item = params.get("item");
            let kind = item
                .and_then(|item| item.get("type"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool");
            let activity = match kind {
                "commandExecution" => item
                    .and_then(|item| item.get("command"))
                    .map(format_command)
                    .map(|command| format!("Running command: {command}")),
                "fileChange" => Some("Preparing file changes".to_owned()),
                "webSearch" => item
                    .and_then(|item| item.get("query"))
                    .and_then(serde_json::Value::as_str)
                    .map(|query| format!("Searching the web: {query}")),
                "reasoning" => None,
                _ => Some(format!("Codex activity: {kind}")),
            };
            EventOutcome {
                activity: activity.into_iter().collect(),
                ..EventOutcome::default()
            }
        }
        "item/commandExecution/outputDelta" => EventOutcome {
            activity: params
                .get("delta")
                .and_then(serde_json::Value::as_str)
                .map(|delta| format!("Command output:\n{delta}"))
                .into_iter()
                .collect(),
            ..EventOutcome::default()
        },
        "turn/completed" => {
            let status = params
                .get("turn")
                .and_then(|turn| turn.get("status"))
                .map(|status| {
                    status
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| status.as_str())
                        .unwrap_or("completed")
                })
                .unwrap_or("completed");
            EventOutcome {
                complete: Some(status.to_owned()),
                ..EventOutcome::default()
            }
        }
        _ => EventOutcome::default(),
    }
}

fn opencode_event_outcome(
    event: &OpenCodeEvent,
    session_id: &str,
    turn_started: bool,
) -> EventOutcome {
    if event.event_type == "server.disconnected" {
        return EventOutcome {
            error: Some("OpenCode server disconnected".to_owned()),
            ..EventOutcome::default()
        };
    }
    if event
        .data
        .get("sessionID")
        .and_then(serde_json::Value::as_str)
        != Some(session_id)
    {
        return EventOutcome::default();
    }
    match event.event_type.as_str() {
        "session.execution.started" => EventOutcome {
            started: true,
            ..EventOutcome::default()
        },
        "session.text.delta" => EventOutcome {
            output: event
                .data
                .get("delta")
                .or_else(|| event.data.get("text"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .into_iter()
                .collect(),
            ..EventOutcome::default()
        },
        "session.tool.called" => EventOutcome {
            activity: event
                .data
                .get("name")
                .or_else(|| event.data.get("tool"))
                .and_then(serde_json::Value::as_str)
                .map(|name| format!("Using tool: {name}"))
                .into_iter()
                .collect(),
            ..EventOutcome::default()
        },
        "session.execution.failed" => EventOutcome {
            complete: Some("failed".to_owned()),
            ..EventOutcome::default()
        },
        "session.execution.interrupted" => EventOutcome {
            complete: Some("interrupted".to_owned()),
            ..EventOutcome::default()
        },
        "session.execution.succeeded" => EventOutcome {
            complete: Some("completed".to_owned()),
            ..EventOutcome::default()
        },
        "session.idle" if turn_started => EventOutcome {
            complete: Some("completed".to_owned()),
            ..EventOutcome::default()
        },
        _ => EventOutcome::default(),
    }
}

fn format_command(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::String(command) => command.clone(),
        _ => "command".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use serde_json::json;

    #[test]
    fn parses_terminal_commands_and_provider_selection() {
        let cli = Cli::try_parse_from([
            "caelum",
            "run",
            "Fix the bug",
            "--provider",
            "opencode",
            "--session",
            "ses_123",
        ])
        .unwrap();
        assert_eq!(cli.provider, ProviderKind::Opencode);
        assert!(matches!(
            cli.command,
            Some(CommandKind::Run { session: Some(id), .. }) if id == "ses_123"
        ));
    }

    #[test]
    fn parses_a_project_to_open_in_the_desktop() {
        let cli = Cli::try_parse_from(["caelum", "/workspace/project"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.project, Some(PathBuf::from("/workspace/project")));
    }

    #[test]
    fn codex_event_output_is_scoped_to_the_requested_thread() {
        let event = ProviderEvent::Codex(CodexEvent {
            project_path: "/workspace".to_owned(),
            method: "item/agentMessage/delta".to_owned(),
            params: json!({ "threadId": "thread-1", "delta": "hello" }),
            request_id: None,
        });
        assert_eq!(event_outcome(&event, "thread-1", false).output, ["hello"]);
        assert!(event_outcome(&event, "thread-2", false).output.is_empty());
    }

    #[test]
    fn opencode_completion_and_streamed_text_are_recognized() {
        let delta = ProviderEvent::OpenCode(OpenCodeEvent {
            project_path: "/workspace".to_owned(),
            event_type: "session.text.delta".to_owned(),
            data: json!({ "sessionID": "ses_1", "delta": "hello" }),
            id: None,
            created: None,
            location: None,
        });
        let complete = ProviderEvent::OpenCode(OpenCodeEvent {
            project_path: "/workspace".to_owned(),
            event_type: "session.idle".to_owned(),
            data: json!({ "sessionID": "ses_1" }),
            id: None,
            created: None,
            location: None,
        });
        assert_eq!(event_outcome(&delta, "ses_1", false).output, ["hello"]);
        assert_eq!(event_outcome(&complete, "ses_1", false).complete, None);
        assert_eq!(
            event_outcome(&complete, "ses_1", true).complete.as_deref(),
            Some("completed")
        );
    }

    #[test]
    fn command_format_supports_argv_and_plain_strings() {
        assert_eq!(format_command(&json!(["cargo", "test"])), "cargo test");
        assert_eq!(format_command(&json!("cargo test")), "cargo test");
    }
}
