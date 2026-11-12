use super::*;

#[test]
#[ignore = "requires a locally installed/authenticated provider and CAELUM_RUNE_SMOKE_PROJECT"]
fn real_provider_discovery_and_history_smoke() {
    use caelum_core::application::Provider;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let path = std::env::var("CAELUM_RUNE_SMOKE_PROJECT")
        .expect("set a project containing provider history");
    std::env::var_os("CAELUM_STATE")
        .or_else(|| std::env::var_os("CAELUM_RUNE_STATE"))
        .expect("set an isolated CAELUM_STATE path for smoke checks");
    let provider = if std::env::var("CAELUM_RUNE_SMOKE_PROVIDER").as_deref() == Ok("codex") {
        Provider::Codex
    } else {
        Provider::OpenCode
    };
    let app = spec();
    let mut state = Workspace::live(Some(path));
    app.setup.unwrap()(&mut state, Arc::new(|| {}));
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut selected = None;
    loop {
        rt.frame(1.0 / 60.0);
        let live = rt.state.live.as_ref().unwrap();
        assert!(
            live.app.error.is_none(),
            "project failed: {:?}",
            live.app.error
        );
        if let Some(error) = live
            .app
            .notices
            .iter()
            .find(|(scope, _)| scope.provider == provider)
            .map(|(_, error)| error)
        {
            panic!("provider failed: {error}");
        }
        if let Some(index) = selected {
            let session: &caelum_core::application::Session = &live.app.sessions[index];
            assert!(
                session.error.is_none(),
                "history failed: {:?}",
                session.error
            );
            if session.loaded && !session.loading {
                assert!(
                    !session.conversation.entries.is_empty(),
                    "choose a project with nonempty history"
                );
                println!(
                    "{}: {} sessions discovered; {} history entries rendered",
                    provider.as_str(),
                    live.app.sessions.len(),
                    session.conversation.entries.len()
                );
                break;
            }
        } else if let Some(index) = live
            .app
            .sessions
            .iter()
            .position(|s| s.scope.provider == provider && s.remote_id.is_some())
        {
            selected = Some(index);
            rt.update(|s, _| s.live.as_mut().unwrap().app.select(index));
        }
        assert!(
            Instant::now() < deadline,
            "provider discovery/history timed out"
        );
        std::thread::sleep(Duration::from_millis(16));
    }
}

#[test]
#[ignore = "requires a real Git checkout, authenticated gh and CAELUM_RUNE_SMOKE_PR"]
fn real_workspace_services_smoke() {
    use caelum_core::application::{
        Command, Event, Reply, Runtime as WorkerRuntime, Task, TaskReply,
    };
    use caelum_protocol::{GitHubFeedbackKind, ProjectSearchMode};
    use std::time::{Duration, Instant};
    let project = std::env::var("CAELUM_RUNE_SMOKE_PROJECT").expect("set a real checkout");
    let url = std::env::var("CAELUM_RUNE_SMOKE_PR").expect("set a pull request URL");
    let mut runtime = WorkerRuntime::new(|| {});
    let mut id = 0;
    let mut run = |task: Task| {
        id += 1;
        runtime.submit(id, Command::Task(task)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            for event in runtime.drain() {
                if let Event::Completed(current, result) = event
                    && current == id
                {
                    let Reply::Task(reply) = result.expect("workspace service failed") else {
                        panic!("wrong reply")
                    };
                    return reply;
                }
            }
            assert!(Instant::now() < deadline, "workspace service timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    assert!(
        matches!(run(Task::Directory { project: project.clone(), path: String::new(), offset: 0 }), TaskReply::Directory(page) if !page.entries.is_empty())
    );
    assert!(
        matches!(run(Task::File { project: project.clone(), path: "Cargo.toml".into() }), TaskReply::File(file) if !file.content.is_empty())
    );
    assert!(
        matches!(run(Task::Search { project: project.clone(), query: "Cargo.toml".into(), mode: ProjectSearchMode::Files }), TaskReply::Search(page) if !page.matches.is_empty())
    );
    let TaskReply::Git(status, _) = run(Task::Git {
        project: project.clone(),
    }) else {
        panic!("Git status")
    };
    assert!(
        matches!(run(Task::Worktrees { project: project.clone(), directory: String::new() }), TaskReply::Worktrees(Some(list)) if !list.worktrees.is_empty())
    );
    if let Some(change) = status.changes.first() {
        assert!(matches!(
            run(Task::Diff {
                project: project.clone(),
                path: change.path.clone()
            }),
            TaskReply::Diff(_)
        ));
    }
    let TaskReply::Details(details) = run(Task::Details {
        project: project.clone(),
        url: url.clone(),
    }) else {
        panic!("PR overview")
    };
    assert!(matches!(
        run(Task::Discover {
            project: project.clone(),
            branch: details.head_ref_name.clone()
        }),
        TaskReply::Discovered(Some(_))
    ));
    let head = details.head_ref_oid;
    let TaskReply::ReviewFiles(files) = run(Task::ReviewFiles {
        project: project.clone(),
        url: url.clone(),
        head: head.clone(),
        page: 1,
    }) else {
        panic!("PR files")
    };
    assert!(!files.files.is_empty());
    for kind in [
        GitHubFeedbackKind::Discussion,
        GitHubFeedbackKind::Reviews,
        GitHubFeedbackKind::Threads,
    ] {
        assert!(matches!(
            run(Task::Feedback {
                project: project.clone(),
                url: url.clone(),
                head: head.clone(),
                kind,
                cursor: None,
                thread: None
            }),
            TaskReply::Feedback(_)
        ));
    }
    println!(
        "Real workspace: directory, file, search, Git/diff, worktrees, PR discovery/overview, {} changed files and all feedback categories loaded",
        files.files.len()
    );
}
