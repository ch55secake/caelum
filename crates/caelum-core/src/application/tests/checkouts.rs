use super::*;

#[test]
fn real_checkout_watcher_refreshes_edits_and_staging_without_losing_the_draft() {
    use std::{fs, process::Command as Process, time::Instant};
    let directory =
        std::env::temp_dir().join(format!("caelum-live-checkout-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let git = |args: &[&str]| {
        let result = Process::new("git")
            .args(args)
            .current_dir(&directory)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    git(&["init", "-q"]);
    fs::write(directory.join("Cargo.lock"), "before\n").unwrap();
    git(&["add", "Cargo.lock"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.com",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-qm",
        "fixture",
    ]);
    git(&["checkout", "--detach", "-q"]);
    let project = directory.to_string_lossy().into_owned();
    let mut runtime = Runtime::new(|| {});
    runtime.watch_checkouts(vec![project.clone()]);
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        path: project.clone(),
        name: "Fixture".into(),
    });
    let index = app.new_session(scope(Provider::Codex, &project));
    app.sessions[index].draft = "Keep this independent draft".into();
    app.commands();
    app.open_file(project.clone(), "Cargo.lock".into());
    app.checkout_changed(&project);
    let mut wait = |app: &mut Workspace, ready: &dyn Fn(&Workspace) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            for (id, command) in app.commands() {
                runtime.submit(id, command).unwrap();
            }
            for event in runtime.drain() {
                app.apply(event);
            }
            if ready(app) {
                break;
            }
            assert!(Instant::now() < deadline, "checkout refresh timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait(&mut app, &|a| {
        a.services.checkouts[&project].git.is_some()
            && a.services.checkouts[&project]
                .files
                .contains_key("Cargo.lock")
    });
    fs::write(directory.join("Cargo.lock"), "after\n").unwrap();
    wait(&mut app, &|a| {
        let c = &a.services.checkouts[&project];
        c.files["Cargo.lock"].content == "after\n" && c.git.as_ref().unwrap().changes.len() == 1
    });
    app.task(Task::Diff {
        project: project.clone(),
        path: "Cargo.lock".into(),
    });
    wait(&mut app, &|a| {
        a.services.checkouts[&project]
            .diffs
            .contains_key("Cargo.lock")
    });
    git(&["add", "Cargo.lock"]);
    wait(&mut app, &|a| {
        a.services.checkouts[&project].diffs["Cargo.lock"]
            .staged
            .contains("+after")
    });
    assert_eq!(app.sessions[index].draft, "Keep this independent draft");
    assert_eq!(
        app.services.checkouts[&project].selected_file.as_deref(),
        Some("Cargo.lock")
    );
    drop(runtime);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checkout_refresh_coalesces_edits_without_losing_inflight_file_updates() {
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        path: "/a".into(),
        name: "a".into(),
    });
    let checkout = app.services.checkouts.entry("/a".into()).or_default();
    checkout.tabs.push("file.rs".into());
    checkout.selected_file = Some("file.rs".into());
    app.checkout_changed("/a");
    let first = app.commands();
    let git = first
        .iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Git { .. })))
        .unwrap()
        .0;
    let file = first
        .iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::File { .. })))
        .unwrap()
        .0;
    for _ in 0..20 {
        app.checkout_changed("/a");
    }
    assert!(app.commands().is_empty());
    let status = serde_json::from_value(json!({"projectPath":"/a","repositoryRoot":"/a","branch":null,"changes":[{"path":"file.rs","indexStatus":" ","worktreeStatus":"M"}]})).unwrap();
    app.apply(Event::Completed(
        git,
        Ok(Reply::Task(TaskReply::Git(status, vec![]))),
    ));
    let second = app.commands();
    assert_eq!(
        second
            .iter()
            .filter(|(_, c)| matches!(c, Command::Task(Task::Git { .. })))
            .count(),
        1
    );
    assert!(!second
        .iter()
        .any(|(_, c)| matches!(c, Command::Task(Task::File { .. }))));
    app.apply(Event::Completed(
        file,
        Ok(Reply::Task(TaskReply::File(
            caelum_protocol::ProjectFileContent {
                path: "file.rs".into(),
                content: "older snapshot".into(),
                size_bytes: 14,
                truncated: false,
            },
        ))),
    ));
    assert!(
        app.commands()
            .iter()
            .any(|(_, c)| matches!(c, Command::Task(Task::File { path, .. }) if path == "file.rs")),
        "a change during an in-flight read must refresh again"
    );
    assert_eq!(
        app.services.checkouts["/a"].selected_file.as_deref(),
        Some("file.rs")
    );
}
