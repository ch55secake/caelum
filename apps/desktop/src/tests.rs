use rune::{Env, Key, KeyEvent, Mods, Runtime, runtime::ElementInfo};

use super::{Workspace, spec};

fn runtime(size: (f32, f32)) -> Runtime<Workspace> {
    let app = spec();
    let mut rt = Runtime::new((app.init)(), app.view, size, Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    rt.set_font_families(app.families.0, app.families.1);
    advance(&mut rt, 0.5);
    rt
}

fn advance(rt: &mut Runtime<Workspace>, seconds: f64) {
    for _ in 0..(seconds * 60.0).ceil() as usize {
        rt.frame(1.0 / 60.0);
    }
}

fn element(rt: &Runtime<Workspace>, key: &str) -> ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .unwrap_or_else(|| panic!("missing {key}"))
}

fn click(rt: &mut Runtime<Workspace>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    advance(rt, 0.1);
}

fn key(rt: &mut Runtime<Workspace>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(1.0 / 60.0);
}

#[test]
fn editing_sending_background_streaming_and_stopping_stay_session_scoped() {
    let mut rt = runtime((1280.0, 1000.0));
    let original = rt.state.active().draft.text.clone();
    click(&mut rt, "session-1");
    click(&mut rt, "draft-1");
    rt.paste("Review café ☁️");
    key(
        &mut rt,
        Key::Enter,
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    rt.paste("Keep the draft isolated.");
    key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.active().turns[0].prompt,
        "Review café ☁️\nKeep the draft isolated."
    );
    assert!(rt.state.active().working());
    assert!(rt.state.active().draft.text.is_empty());

    rt.paste("Next message");
    key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.active().turns.len(),
        1,
        "no second send during a stream"
    );
    click(&mut rt, "session-0");
    advance(&mut rt, 6.0);
    assert_eq!(rt.state.active().draft.text, original);
    assert!(!rt.state.sessions[1].working());
    assert!(
        rt.state.sessions[1].turns[0]
            .response
            .ends_with("sample patch.")
    );

    click(&mut rt, "session-1");
    assert_eq!(rt.state.active().draft.text, "Next message");
    click(&mut rt, "send");
    advance(&mut rt, 0.5);
    click(&mut rt, "send");
    let stopped = rt.state.active().turns[1].response.clone();
    assert!(!stopped.is_empty());
    assert!(rt.state.active().turns[1].stopped);
    advance(&mut rt, 6.0);
    assert_eq!(
        rt.state.active().turns[1].response,
        stopped,
        "stop cancels future chunks"
    );
    click(&mut rt, "draft-1");
    rt.paste("Try the keyboard stop shortcut");
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.2);
    key(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.active().working());
    assert!(rt.state.active().turns[2].stopped);
}

#[test]
fn panes_theme_tools_and_diff_preserve_the_conversation_and_draft() {
    let mut rt = runtime((1280.0, 1000.0));
    let draft = rt.state.active().draft.text.clone();
    let response = rt.state.active().turns[0].response.clone();
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    click(&mut rt, "copy-0");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some(response.as_str())
    );
    click(&mut rt, "tool-0");
    assert!(rt.state.active().turns[0].expanded);
    click(&mut rt, "theme");
    assert_eq!(rt.state.dark, Some(false));
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    assert!(rt.state.inspector.is_open());
    click(&mut rt, "diff-mode");
    assert_eq!(rt.state.diff.mode(), rune_kit::diff::DiffMode::Split);

    let (x, y) = rt.find("sidebar-grip").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_move(x + 40.0, y);
    rt.pointer_up(x + 40.0, y);
    advance(&mut rt, 0.5);
    assert_eq!(rt.state.sidebar.width(), 278.0);
    key(
        &mut rt,
        Key::Command('b'),
        Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        },
    );
    advance(&mut rt, 0.5);
    assert!(!rt.state.sidebar.is_open());
    click(&mut rt, "sidebar-toggle");
    advance(&mut rt, 0.5);
    assert!(rt.state.sidebar.is_open());
    assert_eq!(rt.state.sidebar.width(), 278.0);
    assert_eq!(rt.state.active().draft.text, draft);
    assert_eq!(rt.state.active().turns[0].response, response);
}

#[test]
fn compact_layout_keeps_the_composer_and_pane_controls_visible() {
    let mut rt = runtime((800.0, 620.0));
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    for key in [
        "draft-0",
        "send",
        "sidebar-toggle",
        "inspector-toggle",
        "close-inspector",
    ] {
        let e = element(&rt, key);
        assert!(
            e.rect[2] > 0.0 && e.rect[3] > 0.0,
            "{key} has a usable size"
        );
        for (actual, visible) in e.rect.into_iter().zip(e.visible) {
            assert!(
                (actual - visible).abs() < 1.0,
                "{key} is clipped: {:?} vs {:?}",
                e.rect,
                e.visible
            );
        }
    }
    click(&mut rt, "new-session");
    rt.paste("A new draft");
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.selected, 2);
    assert_eq!(
        rt.state.active().draft.text,
        "A new draft",
        "new sessions focus their composer"
    );
}

#[test]
fn scrolling_up_during_a_reply_preserves_the_reading_position() {
    let mut rt = runtime((1000.0, 700.0));
    click(&mut rt, "send");
    advance(&mut rt, 1.0);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(true));
    let (x, y) = rt.find("transcript-0").unwrap();
    rt.wheel(x, y, 250.0);
    advance(&mut rt, 0.1);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(false));
    let reading = element(&rt, "turn-0").rect[1];
    advance(&mut rt, 6.0);
    assert_eq!(element(&rt, "turn-0").rect[1], reading);
    rt.wheel(x, y, -10000.0);
    advance(&mut rt, 0.5);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(true));
}

fn live_runtime() -> Runtime<Workspace> {
    live_runtime_size((1280.0, 1000.0))
}

#[test]
fn desktop_shutdown_saves_the_latest_editor_and_releases_the_workspace_lock() {
    use caelum_core::application::{Provider, Scope, Store, Workspace as Application};
    use std::{
        fs,
        sync::{Arc, mpsc},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    let directory = std::env::temp_dir().join(format!(
        "caelum-desktop-shutdown-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = Store(directory.join("workspace.json"));
    let mut app = Application::default();
    app.new_session(Scope {
        project: "/project".into(),
        provider: Provider::Codex,
    });
    let mut saved = app.snapshot();
    saved.selected = None; // Restore offline drafts without contacting a provider.
    saved.sessions[0].draft = "saved draft".into();
    store.save(&saved).unwrap();
    let (send, receive) = mpsc::channel();
    let mut live = super::live::Live::new(None);
    live.start(
        store.clone(),
        Arc::new(move || {
            let _ = send.send(());
        }),
    );
    assert!(store.lock().is_err());
    live.poll(&mut rune::Tx::default());
    while live.drafts.is_empty() {
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("workspace load wakes the UI");
        live.poll(&mut rune::Tx::default());
    }
    assert_eq!(live.drafts[0].text, "saved draft");
    live.drafts[0].text = "older in-flight save".into();
    live.poll(&mut rune::Tx::default());
    live.drafts[0].text = "latest edit before quit".into();
    drop(live);
    assert_eq!(
        store.load().unwrap().unwrap().sessions[0].draft,
        "latest edit before quit"
    );
    assert!(store.lock().is_ok());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn project_links_wait_for_restore_and_keep_drafts_when_the_app_is_running() {
    use caelum_core::application::{
        Command, Event, Provider, Reply, Scope, Workspace as Application,
    };
    let mut original = Application::default();
    original.projects.push(caelum_protocol::ProjectInfo {
        name: "Original".into(),
        path: "/original".into(),
    });
    original.new_session(Scope {
        project: "/original".into(),
        provider: Provider::Codex,
    });
    original.sessions[0].draft = "restored draft".into();
    let mut state = Workspace::live(None);
    state
        .live
        .as_mut()
        .unwrap()
        .app
        .enable_store("/unused-link-test/workspace.json".into());
    let url = caelum_core::desktop::project_url(std::path::Path::new("/next café")).unwrap();
    super::lifecycle::open_url(&mut state, &url, &mut rune::Tx::default());
    let live = state.live.as_mut().unwrap();
    let commands = live.app.commands();
    assert_eq!(commands.len(), 1);
    assert!(matches!(commands[0].1, Command::Load(_)));
    live.app.apply(Event::Completed(
        commands[0].0,
        Ok(Reply::Snapshot(Some(original.snapshot()))),
    ));
    live.poll(&mut rune::Tx::default());
    let mut commands = live.app.commands();
    if let Some((save, _)) = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Save(..)))
    {
        live.app.apply(Event::Completed(*save, Ok(Reply::Done)));
        commands = live.app.commands();
    }
    let open = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Open(path) if path == "/next café"))
        .unwrap()
        .0;
    live.app.apply(Event::Completed(
        open,
        Ok(Reply::Project(caelum_protocol::ProjectInfo {
            name: "Next".into(),
            path: "/next café".into(),
        })),
    ));
    live.poll(&mut rune::Tx::default());
    assert_eq!(live.drafts[0].text, "restored draft");
    assert_eq!(live.app.active_project.as_deref(), Some("/next café"));
    live.drafts[1].text = "draft in the running app".into();
    let original_url =
        caelum_core::desktop::project_url(std::path::Path::new("/original")).unwrap();
    super::lifecycle::open_url(&mut state, &original_url, &mut rune::Tx::default());
    let live = state.live.as_mut().unwrap();
    live.poll(&mut rune::Tx::default());
    assert_eq!(live.drafts[1].text, "draft in the running app");
    super::lifecycle::open_url(
        &mut state,
        "caelum://open?path=relative",
        &mut rune::Tx::default(),
    );
    assert!(state.live.as_ref().unwrap().app.error.is_some());
    // This reducer-only instance must not try to write the sentinel path on drop.
    let live = state.live.as_mut().unwrap();
    live.app = Application::default();
    live.drafts.clear();
}

fn live_runtime_size(size: (f32, f32)) -> Runtime<Workspace> {
    use caelum_core::application::{Provider, Scope};
    let mut state = Workspace::live(None);
    let live = state.live.as_mut().unwrap();
    live.app.projects.push(caelum_protocol::ProjectInfo {
        name: "Project".into(),
        path: "/project".into(),
    });
    for provider in Provider::ALL {
        live.app.new_session(Scope {
            project: "/project".into(),
            provider,
        });
    }
    live.app.select(0);
    live.poll(&mut rune::Tx::default());
    let app = spec();
    let mut rt = Runtime::new(state, app.view, size, Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    rt.set_font_families(app.families.0, app.families.1);
    advance(&mut rt, 0.5);
    rt
}

#[test]
fn live_creation_streaming_and_failed_submissions_preserve_independent_editors() {
    use caelum_core::application::{Command, Event, Options, Reply, SessionKey, Summary};
    let mut rt = live_runtime();
    click(&mut rt, "live-draft-0");
    rt.paste("First real prompt");
    key(&mut rt, Key::Enter, Mods::default());
    let (request, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    let Command::Start(scope, _) = command else {
        panic!("expected creation")
    };
    rt.paste("Next draft");
    click(&mut rt, "live-session-1");
    click(&mut rt, "live-draft-1");
    rt.paste("Other provider draft");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            request,
            Ok(Reply::Session(Summary {
                key: SessionKey {
                    scope,
                    id: "thread-1".into(),
                },
                title: "Created".into(),
                working: false,
                options: Options::default(),
            })),
        ))
    });
    let (send, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(matches!(command, Command::Send(_, ref prompt, _) if prompt == "First real prompt"));
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.apply(Event::Completed(send, Ok(Reply::Done)));
        app.apply(Event::Codex(caelum_protocol::CodexEvent {
            project_path: "/project".into(), method: "item/agentMessage/delta".into(), request_id: None,
            params: serde_json::json!({ "threadId": "thread-1", "itemId": "answer", "delta": "Background response" }),
        }));
    });
    advance(&mut rt, 0.5);
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.app.selected, Some(1));
    assert_eq!(live.drafts[0].text, "Next draft");
    assert_eq!(live.drafts[1].text, "Other provider draft");
    assert_eq!(
        live.app.sessions[0].conversation.entries[1].text,
        "Background response"
    );
    key(&mut rt, Key::Enter, Mods::default());
    let (request, _) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    rt.paste("Newer unsent draft");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            request,
            Err("Provider unavailable".into()),
        ))
    });
    advance(&mut rt, 0.2);
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.drafts[1].text, "Newer unsent draft");
    assert_eq!(
        live.app.sessions[1].failed_prompt.as_deref(),
        Some("Other provider draft")
    );
    click(&mut rt, "live-session-0");
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, "Next draft");
}

#[test]
fn live_navigation_restores_reading_position_and_approval_decisions_keep_their_owner() {
    use caelum_core::application::{Command, Event, Provider};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.sessions[0].remote_id = Some("thread-1".into());
        app.sessions[0].conversation.history(Provider::Codex, &[serde_json::json!({
            "status":"completed", "items":[{"id":"long","type":"agentMessage","text":"A paragraph worth reading.\n\n".repeat(90)}]
        })], false);
    });
    advance(&mut rt, 0.5);
    let (x, y) = rt.find("live-transcript-0").unwrap();
    rt.wheel(x, y, 300.0);
    advance(&mut rt, 0.2);
    let position = element(&rt, "live-entry-0-codex-long").rect[1];
    click(&mut rt, "live-session-1");
    click(&mut rt, "live-session-0");
    advance(&mut rt, 0.5);
    assert!((element(&rt, "live-entry-0-codex-long").rect[1] - position).abs() < 1.0);
    assert_eq!(rt.scroll_at_bottom("live-transcript-0"), Some(false));
    rt.update(|s, tx| {
        s.live.as_mut().unwrap().app.apply(Event::Approvals(vec![
            caelum_protocol::ApprovalPrompt {
                provider: "codex".into(),
                project_path: "/project".into(),
                session_id: Some("thread-1".into()),
                request_id: "one".into(),
                summary: "Run command?".into(),
                description: "cargo test".into(),
            },
        ]));
        tx.scroll_to_bottom_instant("live-transcript-0");
    });
    advance(&mut rt, 0.3);
    click(&mut rt, "allow-once-one");
    let (_, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(
        matches!(command, Command::Approve(prompt, caelum_protocol::ApprovalDecision::AllowOnce)
        if prompt.project_path == "/project" && prompt.session_id.as_deref() == Some("thread-1") && prompt.request_id == "one")
    );
}

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

#[test]
fn live_provider_and_model_controls_apply_to_first_send() {
    use caelum_core::application::{Command, Provider, Scope};
    use caelum_protocol::{ProviderChoice, ProviderChoices, ProviderModelChoice};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.choices.insert(
            Scope {
                provider: Provider::OpenCode,
                project: "/project".into(),
            },
            ProviderChoices {
                models: ["first", "second"]
                    .into_iter()
                    .map(|id| ProviderModelChoice {
                        id: id.into(),
                        name: id.into(),
                        provider: None,
                        context_window: None,
                        efforts: vec![ProviderChoice {
                            id: "low".into(),
                            name: "Low".into(),
                        }],
                        default_effort: Some("low".into()),
                    })
                    .collect(),
                agents: vec![ProviderChoice {
                    id: "build".into(),
                    name: "Build".into(),
                }],
                default_model: Some("first".into()),
            },
        );
    });
    click(&mut rt, "provider-trigger");
    click(&mut rt, "option1");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .scope
            .provider,
        Provider::OpenCode
    );
    click(&mut rt, "model-trigger");
    click(&mut rt, "option1");
    click(&mut rt, "agent-trigger");
    click(&mut rt, "option0");
    click(&mut rt, "live-draft-0");
    rt.paste("Use my choices");
    key(&mut rt, Key::Enter, Mods::default());
    let (_, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(
        matches!(command, Command::Start(scope, options) if scope.provider == Provider::OpenCode
        && options.model.as_deref() == Some("second") && options.effort.as_deref() == Some("low") && options.agent.as_deref() == Some("build"))
    );
    click(&mut rt, "model-trigger");
    click(&mut rt, "option0");
    click(&mut rt, "live-draft-0");
    rt.paste("Queue with a different model");
    key(&mut rt, Key::Enter, Mods::default());
    let queued = &rt.state.live.as_ref().unwrap().app.sessions[0].queue;
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].options.model.as_deref(), Some("first"));
}

#[test]
fn compact_live_inspector_and_modals_preserve_the_composer_and_focus() {
    let mut rt = live_runtime_size((800.0, 620.0));
    click(&mut rt, "live-draft-0");
    rt.paste("Keep this compact draft");
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    for screen in ["nav-files", "nav-changes"] {
        click(&mut rt, screen);
        for key in [
            "live-draft-0",
            "send",
            "close-inspector",
            "provider-trigger",
        ] {
            let e = element(&rt, key);
            assert!(e.rect[2] > 0.0 && e.rect[3] > 0.0);
            assert!(
                e.rect
                    .into_iter()
                    .zip(e.visible)
                    .all(|(a, b)| (a - b).abs() < 1.0),
                "{screen}: {key} is clipped: {:?} vs {:?}",
                e.rect,
                e.visible
            );
        }
    }
    click(&mut rt, "close-inspector");
    advance(&mut rt, 0.5);
    for screen in ["nav-worktrees", "nav-settings", "session-options"] {
        click(&mut rt, screen);
        let dialog = element(&rt, "workspace-dialog");
        assert!(
            dialog
                .rect
                .into_iter()
                .zip(dialog.visible)
                .all(|(a, b)| (a - b).abs() < 1.0)
        );
        click(&mut rt, "live-draft-0");
        rt.paste("must not enter the background composer");
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[0].text,
            "Keep this compact draft"
        );
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.4);
        assert!(rt.state.live.as_ref().unwrap().panels.overlay.is_none());
    }
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep this compact draft"
    );
}

#[test]
fn two_thousand_entries_and_six_streams_keep_the_native_tree_bounded() {
    let app = spec();
    let mut rt = Runtime::new(
        crate::evaluation::workspace(),
        app.view,
        (1280.0, 1000.0),
        Env::new(true, 1.0),
    );
    rt.poll = app.poll;
    advance(&mut rt, 0.6);
    let mut times = Vec::new();
    for _ in 0..120 {
        let start = std::time::Instant::now();
        rt.frame(1.0 / 60.0);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    let mounted = rt
        .elements()
        .iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|key| key.starts_with("live-entry-"))
        })
        .count();
    assert!(
        mounted < 120,
        "history must be virtualized, mounted {mounted}"
    );
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.app.sessions[0].conversation.entries.len(), 2001);
    assert!(live.app.sessions.iter().all(|s| s.working()));
    println!(
        "2,000 entries × 6 sessions: {mounted} mounted entries; CPU frame p50 {:.2} ms, p95 {:.2} ms (headless, not GPU presentation)",
        times[60], times[114]
    );
    click(&mut rt, "live-draft-0");
    rt.paste("Draft remains editable while six sessions stream");
    rt.frame(1.0 / 60.0);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Draft remains editable while six sessions stream"
    );
}

#[test]
fn native_question_forms_send_scoped_values_and_keep_invalid_answers_editable() {
    use caelum_core::application::{Command, Event};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let live = s.live.as_mut().unwrap(); live.app.sessions[0].remote_id = Some("thread".into());
        live.app.apply(Event::Questions(vec![caelum_protocol::QuestionPrompt { provider: "codex".into(), project_path: "/project".into(), session_id: "thread".into(), request_id: "question".into(), title: "Choose a branch".into(), fields: vec![serde_json::json!({"key":"branch","type":"string","required":true,"options":[{"label":"Main","value":"main"},{"label":"Feature","value":"feature"}]})] }]));
    });
    advance(&mut rt, 0.3);
    click(&mut rt, "question-submit-question");
    assert!(rt.state.live.as_mut().unwrap().app.commands().is_empty());
    click(&mut rt, "question-question-branch-1");
    click(&mut rt, "question-submit-question");
    assert!(
        matches!(rt.state.live.as_mut().unwrap().app.commands().as_slice(), [(_, Command::Answer(prompt, Some(answer)))] if prompt.session_id == "thread" && answer["branch"] == "feature")
    );
}

#[test]
fn native_file_preview_attaches_an_immutable_snapshot_to_the_originating_draft() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    let mut rt = live_runtime();
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    let (directory, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(
        matches!(command, Command::Task(Task::Directory { ref project, .. }) if project == "/project")
    );
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            directory,
            Ok(Reply::Task(TaskReply::Directory(
                caelum_protocol::ProjectDirectoryListing {
                    path: "".into(),
                    entries: vec![caelum_protocol::ProjectFileEntry {
                        name: "main.rs".into(),
                        path: "main.rs".into(),
                        kind: caelum_protocol::ProjectEntryKind::File,
                        size_bytes: Some(12),
                    }],
                    total: 1,
                    next_offset: None,
                },
            ))),
        ))
    });
    advance(&mut rt, 0.3);
    click(&mut rt, "file-tree-main.rs");
    let (file, _) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            file,
            Ok(Reply::Task(TaskReply::File(
                caelum_protocol::ProjectFileContent {
                    path: "main.rs".into(),
                    content: "fn main() {}".into(),
                    size_bytes: 12,
                    truncated: false,
                },
            ))),
        ))
    });
    advance(&mut rt, 0.3);
    click(&mut rt, "attach-preview");
    click(&mut rt, "close-inspector");
    click(&mut rt, "live-session-1");
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.app.sessions[0].attachments[0].content, "fn main() {}");
    assert!(live.app.sessions[1].attachments.is_empty());
}

#[test]
fn large_review_files_are_windowed_while_streams_continue() {
    let app = spec();
    let mut state = crate::evaluation::workspace();
    crate::panels::open_review(
        &mut state,
        "/evaluation".into(),
        "https://github.com/example/evaluation/pull/1".into(),
    );
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    advance(&mut rt, 0.5);
    let count = rt
        .elements()
        .iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|k| k.starts_with("review-file-src/"))
        })
        .count();
    assert!(
        count > 0 && count < 60,
        "only nearby file rows should be mounted: {count}"
    );
    click(&mut rt, "review-file-src/file-0.rs");
    advance(&mut rt, 0.3);
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.path.ends_with("/diff/scroll") && e.rect[3] > 200.0),
        "the selected patch must have a usable viewport"
    );
    assert!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .sessions
            .iter()
            .all(|s| s.working())
    );
    click(&mut rt, "back-to-session");
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
}
