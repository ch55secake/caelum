use super::*;

#[test]
fn closing_projects_by_keyboard_preserves_drafts_and_keeps_navigation_usable() {
    use caelum_core::application::{Command, Event, Reply};
    let mut rt = design_runtime("sidebar-projects", (800.0, 620.0));
    fill(&mut rt, "live-draft-0", "Retain this project draft café");
    let project = crate::code_view::source_key("/projects/rune", "project");
    click(&mut rt, &project);
    click(
        &mut rt,
        &crate::code_view::source_key("/projects/caelum", "project"),
    );
    let close = crate::code_view::source_key("/projects/rune", "close-project");
    click(&mut rt, &close);
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    let first = crate::code_view::source_key("/projects/caelum", "project");
    click(&mut rt, &first);
    key(&mut rt, Key::Tab, Mods::default());
    key(&mut rt, Key::Right, Mods::default());
    advance(&mut rt, 0.2);
    assert!(
        element(
            &rt,
            &crate::code_view::source_key("/projects/caelum", "close-project")
        )
        .focused,
        "focused: {:?}",
        rt.elements()
            .iter()
            .filter(|e| e.focused)
            .map(|e| &e.key)
            .collect::<Vec<_>>()
    );
    key(&mut rt, Key::Text(" ".into()), Mods::default());
    advance(&mut rt, 0.4);
    assert!(
        !rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .project_is_open("/projects/caelum")
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Retain this project draft café"
    );
    click(&mut rt, "search-sessions");
    fill(&mut rt, "session-search", "Refine");
    assert!(rt.find("live-session-0").is_none());
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.open_project("/projects/caelum".into());
        let open = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Open(_)))
            .unwrap()
            .0;
        l.apply(Event::Completed(
            open,
            Ok(Reply::Project(caelum_protocol::ProjectInfo {
                path: "/projects/caelum".into(),
                name: "caelum".into(),
            })),
        ));
    });
    advance(&mut rt, 0.4);
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Retain this project draft café"
    );
    for path in ["/projects/website", "/projects/tools", "/projects/caelum"] {
        click(
            &mut rt,
            &crate::code_view::source_key(path, "close-project"),
        );
        advance(&mut rt, 0.4);
    }
    assert!(rt.state.live.as_ref().unwrap().app.selected.is_none());
    assert!(element(&rt, "open-project").focused);
    assert!(rt.find("welcome-open").is_some());
    assert!(rt.find("message-composer").is_none());
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
    let mut live = crate::live::Live::new(None);
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
    crate::lifecycle::open_url(&mut state, &url, &mut rune::Tx::default());
    let live = state.live.as_mut().unwrap();
    let commands = live.app.commands();
    assert_eq!(commands.len(), 1);
    assert!(matches!(commands[0].1, Command::Load(_)));
    live.app.apply(Event::Completed(
        commands[0].0,
        Ok(Reply::Snapshot(Some(Box::new(original.snapshot())))),
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
    crate::lifecycle::open_url(&mut state, &original_url, &mut rune::Tx::default());
    let live = state.live.as_mut().unwrap();
    live.poll(&mut rune::Tx::default());
    assert_eq!(live.drafts[1].text, "draft in the running app");
    crate::lifecycle::open_url(
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

#[test]
fn project_url_wakes_an_idle_app_to_process_the_open_request() {
    use caelum_core::application::Store;
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    let directory = std::env::temp_dir().join(format!(
        "caelum-project-url-wake-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let wake_count = Arc::new(AtomicUsize::new(0));
    let mut state = Workspace::live(None);
    state
        .live
        .as_mut()
        .unwrap()
        .start(Store(directory.join("workspace.json")), {
            let wake_count = Arc::clone(&wake_count);
            Arc::new(move || {
                wake_count.fetch_add(1, Ordering::SeqCst);
            })
        });
    let before = wake_count.load(Ordering::SeqCst);
    let url = caelum_core::desktop::project_url(std::path::Path::new("/next-project")).unwrap();

    crate::lifecycle::open_url(&mut state, &url, &mut rune::Tx::default());

    assert!(wake_count.load(Ordering::SeqCst) > before);
    drop(state);
    fs::remove_dir_all(directory).unwrap();
}
