use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

fn missing_project_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "caelum-cli-test-{}-{}",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn help_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_caelum"))
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("chat"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("sessions"));
}

#[test]
fn missing_project_returns_a_failure_exit_code_for_each_workflow() {
    let missing = missing_project_path();
    let executable = env!("CARGO_BIN_EXE_caelum");
    let commands = [
        vec![missing.display().to_string()],
        vec!["chat".to_owned(), missing.display().to_string()],
        vec![
            "run".to_owned(),
            "Do the task".to_owned(),
            "--project".to_owned(),
            missing.display().to_string(),
        ],
        vec!["sessions".to_owned(), missing.display().to_string()],
    ];

    for args in commands {
        let output = Command::new(executable).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("caelum: "));
    }
}

#[cfg(unix)]
#[test]
fn run_uses_the_shared_codex_provider_without_the_desktop_app() {
    use std::os::unix::fs::PermissionsExt;

    if Command::new("python3").arg("--version").output().is_err() {
        return;
    }

    let root = missing_project_path();
    let project = root.join("project");
    let bin = root.join("bin");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&bin).unwrap();
    let fake_codex = bin.join("codex");
    fs::write(
        &fake_codex,
        r##"#!/usr/bin/env python3
import json
import os
import sys

if sys.argv[1:] == ["login", "status"]:
    sys.exit(0)

def send(value):
    print(json.dumps(value), flush=True)

for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    request_id = request.get("id")
    if method == "initialize":
        send({"id": request_id, "result": {}})
    elif method == "thread/start":
        send({"id": request_id, "result": {"thread": {"id": "thread_fake", "name": "CLI test", "preview": ""}}})
    elif method == "thread/list":
        send({"id": request_id, "result": {"data": [{"id": "thread_fake", "name": "CLI test", "preview": ""}], "nextCursor": None}})
    elif method == "turn/start":
        status = os.environ.get("FAKE_CODEX_TURN_STATUS", "completed")
        send({"method": "turn/started", "params": {"threadId": "thread_fake", "turn": {"id": "turn_fake"}}})
        send({"method": "item/agentMessage/delta", "params": {"threadId": "thread_fake", "delta": "Hello from fake Codex."}})
        send({"method": "turn/completed", "params": {"threadId": "thread_fake", "turn": {"id": "turn_fake", "status": status}}})
        send({"id": request_id, "result": {"turn": {"id": "turn_fake"}}})
    elif method != "initialized":
        send({"id": request_id, "error": {"message": "unexpected method: " + str(method)}})
"##,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_codex).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_codex, permissions).unwrap();

    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_caelum"))
        .args(["run", "Say hello"])
        .current_dir(&project)
        .env("PATH", &path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Hello from fake Codex."));
    assert!(String::from_utf8_lossy(&output.stderr).contains("codex session: thread_fake"));

    let failed = Command::new(env!("CARGO_BIN_EXE_caelum"))
        .args(["run", "Fail the task"])
        .current_dir(&project)
        .env("PATH", &path)
        .env("FAKE_CODEX_TURN_STATUS", "failed")
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("codex turn failed"));

    let sessions = Command::new(env!("CARGO_BIN_EXE_caelum"))
        .arg("sessions")
        .current_dir(&project)
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        sessions.status.success(),
        "{}",
        String::from_utf8_lossy(&sessions.stderr)
    );
    assert!(String::from_utf8_lossy(&sessions.stdout).contains("thread_fake\tCLI test"));

    let mut chat = Command::new(env!("CARGO_BIN_EXE_caelum"))
        .arg("chat")
        .arg(&project)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    chat.stdin
        .take()
        .unwrap()
        .write_all(b"Say hello\n/quit\n")
        .unwrap();
    let chat = chat.wait_with_output().unwrap();
    assert!(
        chat.status.success(),
        "{}",
        String::from_utf8_lossy(&chat.stderr)
    );
    assert!(String::from_utf8_lossy(&chat.stdout).contains("Hello from fake Codex."));
    fs::remove_dir_all(root).unwrap();
}
