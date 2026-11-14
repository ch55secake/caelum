//! Read-only provider/restore timing. Prints counts and timings, never session content.
use std::{
    collections::HashMap,
    env, fs,
    sync::mpsc,
    time::{Duration, Instant},
};

use caelum_core::application::{
    Command, Conversation, Event, Provider, ProviderRuntime, Reply, Runtime, Snapshot, Task,
    Workspace,
};
use serde_json::{json, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    let path = args
        .first()
        .ok_or("usage: session_load SNAPSHOT [selected|oldest-open|oldest-archived] [RUNS]")?;
    let target = args.get(1).map(String::as_str).unwrap_or("selected");
    let runs: usize = args.get(2).map_or(Ok(3), |value| value.parse())?;
    let mut snapshot: Snapshot = serde_json::from_slice(&fs::read(path)?)?;
    let index = match target {
        "selected" => snapshot.selected,
        "oldest-open" | "oldest-archived" => snapshot
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                session.scope.provider == Provider::OpenCode
                    && session.remote_id.is_some()
                    && session.archived == (target == "oldest-archived")
            })
            .min_by_key(|(_, session)| session.last_activity)
            .map(|(index, _)| index),
        _ => return Err("unknown target".into()),
    }
    .ok_or("no matching session")?;
    snapshot.selected = Some(index);
    for run in 1..=runs {
        let direct = direct(&snapshot, index)?;
        let restore = restore(snapshot.clone(), index)?;
        println!(
            "{}",
            json!({"target":target, "run":run, "direct":direct, "restore":restore})
        );
    }
    Ok(())
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn direct(snapshot: &Snapshot, index: usize) -> Result<Value, String> {
    let session = &snapshot.sessions[index];
    if session.scope.provider != Provider::OpenCode {
        return Err("direct timing currently requires OpenCode".into());
    }
    let id = session.remote_id.as_deref().ok_or("no remote session")?;
    let providers = ProviderRuntime::default();
    let start = Instant::now();
    providers
        .opencode
        .resume_session(&session.scope.project, id)
        .map_err(|_| "direct resume failed")?;
    let resume_ms = ms(start);
    let start = Instant::now();
    let page = providers
        .opencode
        .session_history(&session.scope.project, id, None)
        .map_err(|_| "direct history failed")?;
    let history_ms = ms(start);
    let bytes = serde_json::to_vec(&page.data).unwrap().len();
    let start = Instant::now();
    let mut conversation = Conversation::default();
    conversation.history(Provider::OpenCode, &page.data, true);
    let apply_ms = ms(start);
    providers.shutdown();
    Ok(
        json!({"resume_ms":resume_ms, "history_ms":history_ms, "apply_ms":apply_ms,
        "messages":page.data.len(), "bytes":bytes, "entries":conversation.entries.len()}),
    )
}

fn restore(snapshot: Snapshot, index: usize) -> Result<Value, String> {
    let (wake, wakes) = mpsc::channel();
    let mut runtime = Runtime::new(move || {
        let _ = wake.send(());
    });
    let mut workspace = Workspace::default();
    // Complete persistence in memory; never migrate or write the supplied snapshot.
    workspace.enable_store("unused-session-profile.json".into());
    let load = workspace.commands().pop().unwrap().0;
    let start = Instant::now();
    workspace.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(snapshot)))),
    ));
    let mut pending = HashMap::new();
    let mut completions = Vec::new();
    loop {
        for (id, command) in workspace.commands() {
            if matches!(command, Command::Save(..)) {
                workspace.apply(Event::Completed(id, Ok(Reply::Done)));
                continue;
            }
            let Some(kind) = read_kind(&command) else {
                workspace.apply(Event::Completed(
                    id,
                    Err("omitted by read-only profile".into()),
                ));
                continue;
            };
            pending.insert(id, (kind, Instant::now()));
            runtime.submit(id, command)?;
        }
        for event in runtime.drain() {
            let mut metric = match &event {
                Event::Completed(id, result) => pending.remove(id).map(|(kind, at)| {
                    json!({"command":kind, "elapsed_ms":ms(at), "at_ms":ms(start), "ok":result.is_ok()})
                }),
                _ => None,
            };
            let apply = Instant::now();
            workspace.apply(event);
            if let Some(metric) = &mut metric {
                metric["apply_ms"] = json!(ms(apply));
            }
            completions.extend(metric);
        }
        if workspace.sessions[index].loaded {
            return Ok(
                json!({"ready_ms":ms(start), "entries":workspace.sessions[index].conversation.entries.len(),
                "completions":completions, "pending":pending.len()}),
            );
        }
        if let Some(error) = &workspace.sessions[index].error {
            // Keep provider messages (which can contain private paths) out of reports.
            return Err(format!("restore failed ({}-byte error)", error.len()));
        }
        if start.elapsed() > Duration::from_secs(90) {
            return Err("restore timed out".into());
        }
        let _ = wakes.recv_timeout(Duration::from_millis(10));
    }
}

fn read_kind(command: &Command) -> Option<&'static str> {
    Some(match command {
        Command::Resume(_) => "resume",
        Command::History(..) => "history",
        Command::Details(_) => "details",
        Command::List(_) => "list",
        Command::ListArchived(_) => "archived",
        Command::Choices(_) => "choices",
        Command::Commands(_) => "commands",
        Command::Shells(_) => "shells",
        Command::Inbox(_) => "inbox",
        Command::Open(_) => "open",
        Command::Task(Task::Git { .. }) => "git",
        Command::Task(Task::Worktrees { .. }) => "worktrees",
        Command::Task(Task::Discover { .. }) => "pull_requests",
        _ => return None,
    })
}
