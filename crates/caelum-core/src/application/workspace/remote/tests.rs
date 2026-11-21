mod handoff;

use crate::{
    application::{Command, Event, Reply, Workspace},
    remote::*,
};

fn source() -> Source {
    Source {
        published_tip: None,
        project: "/local/project".into(),
        repository: "example/project".into(),
        account: "owner".into(),
        base: "main".into(),
        environment: None,
    }
}
fn created(app: &mut Workspace) -> (String, u64, Plan) {
    app.remote.picker = Some(Picker::Ready(source()));
    let id = app
        .create_remote(
            "main",
            "feat/new",
            Size::Small,
            Lifetime::HalfHour,
            crate::application::Provider::OpenCode,
        )
        .unwrap();
    let (req, Command::Remote(Operation::Create(_, plan))) = app.commands().pop().unwrap() else {
        panic!()
    };
    (id, req, plan)
}
fn machine(id: &str) -> Machine {
    Machine {
        key: MachineId {
            host: Host::Boat,
            account: "owner".into(),
            id: id.into(),
        },
        name: "dev".into(),
        state: "idle".into(),
        auto_stop_unix: None,
    }
}

fn prepared_vm(machine: Machine, branch: impl Into<String>) -> Outcome {
    Outcome::Prepared {
        machine,
        branch: branch.into(),
        root: "/workspace/checkout".into(),
    }
}

#[test]
fn vm_feed_scopes_and_sorts_drafts_with_local_sessions_without_reordering_on_switch() {
    use crate::application::{FeedSession, Provider, Scope, SessionScope};

    let mut app = Workspace::default();
    for path in ["/root", "/other"] {
        app.projects.push(caelum_protocol::ProjectInfo {
            name: path.into(),
            path: path.into(),
        });
        let i = app.new_session(Scope {
            project: path.into(),
            provider: Provider::Codex,
        });
        app.sessions[i].last_activity = Some(100);
    }
    app.project_roots.insert(source().project, "/root".into());
    let (id, req, _) = created(&mut app);
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(prepared_vm(machine("vm"), "feat/boat"))),
    ));
    let prior = app.selected_feed_session().unwrap();
    let c = app.remote.saved.checkouts.get_mut(&id).unwrap();
    c.thread.draft = "Réviser le plan".into();
    c.thread.last_activity = None; // A checkout restored from an older snapshot.
    assert!(app.new_vm_session(&id, Provider::Codex));
    let fresh = app.selected_feed_session().unwrap();

    app.preferences.session_scope = SessionScope::CurrentProject;
    let expected = [fresh.clone(), FeedSession::Local(0), prior.clone()];
    assert_eq!(app.sidebar_sessions(None, "", false), expected);
    assert_eq!(
        app.sidebar_sessions(None, "feat/boat", false),
        [fresh.clone(), prior.clone()]
    );
    assert_eq!(
        app.sidebar_sessions(None, "RÉVISER", false),
        std::slice::from_ref(&prior)
    );
    assert_eq!(
        app.sidebar_sessions(Some("/other"), "", false),
        [FeedSession::Local(1)]
    );
    assert!(app.sidebar_sessions(None, "", true).is_empty());

    let FeedSession::Vm(link) = &prior else {
        panic!()
    };
    assert!(app.open_vm_thread(link));
    assert_eq!(app.selected_feed_session().as_ref(), Some(&prior));
    assert_eq!(app.sidebar_sessions(None, "", false), expected);
    app.preferences.session_scope = SessionScope::AllProjects;
    assert_eq!(app.sidebar_sessions(None, "", false).len(), 4);
    app.close_project("/root");
    assert_eq!(
        app.sidebar_sessions(None, "", false),
        [FeedSession::Local(1)]
    );
    assert!(link.resolve(&app.remote.saved).is_some());
}

#[test]
fn attaching_reopens_a_chat_but_the_launcher_starts_one_on_the_verified_checkout() {
    use crate::application::Provider;
    let mut app = Workspace::default();
    let vm = machine("vm");
    app.remote.inventory = Some(Inventory {
        account: vm.key.account.clone(),
        machines: vec![vm.clone()],
    });
    let attach = |app: &mut Workspace, provider| {
        app.remote.picker = Some(Picker::Ready(source()));
        app.attach_remote(vm.clone(), provider)
    };
    let prepared = |app: &mut Workspace| {
        let cmds = app.commands();
        let [(req, Command::Remote(Operation::Prepare(t, _, None)))] = cmds.as_slice() else {
            panic!("{cmds:?}")
        };
        assert_eq!(t.machine, vm.key);
        app.apply(Event::Completed(
            *req,
            Ok(Reply::Remote(prepared_vm(vm.clone(), "existing"))),
        ));
    };
    let id = attach(&mut app, Some(Provider::Codex)).unwrap();
    assert_eq!(
        app.remote.saved.checkouts[&id].thread.provider,
        Provider::Codex
    );
    assert!(attach(&mut app, Some(Provider::OpenCode)).is_none());
    prepared(&mut app);
    app.remote
        .saved
        .checkouts
        .get_mut(&id)
        .unwrap()
        .thread
        .draft = "Unsent".into();
    assert_eq!(attach(&mut app, None), Some(id.clone()));
    prepared(&mut app);
    let original = app.remote.saved.checkouts[&id].thread.clone();
    assert_eq!(original.draft, "Unsent");
    assert_eq!(original.provider, Provider::Codex);
    app.remote.picker = Some(Picker::Ready(Source {
        repository: "example/different".into(),
        ..source()
    }));
    assert!(app
        .attach_remote(vm.clone(), Some(Provider::OpenCode))
        .is_none());
    assert_eq!(attach(&mut app, Some(Provider::OpenCode)), Some(id.clone()));
    let c = &app.remote.saved.checkouts[&id];
    assert_eq!(c.thread.provider, Provider::OpenCode);
    assert!(c.thread.draft.is_empty() && c.thread.conversation.is_none());
    assert_eq!(c.other_threads, vec![original]);
    assert!(app.commands().is_empty());
}

#[test]
fn pending_vm_plans_preserve_old_timers_and_require_explicit_manual_stop() {
    use serde_json::{json, Value};
    let mut plan = json!({"token":"attempt","branch":"feature","size":"Small","minutes":120});
    let read = |v: &Value| serde_json::from_value::<Plan>(v.clone()).map(|p| p.lifetime);
    assert_eq!(read(&plan).unwrap(), Lifetime::TwoHours);
    plan.as_object_mut().unwrap().remove("minutes");
    assert_eq!(read(&plan).unwrap(), Lifetime::Hour);
    plan["minutes"] = Value::Null;
    assert_eq!(read(&plan).unwrap(), Lifetime::UntilStopped);
    plan["minutes"] = json!(0);
    assert!(read(&plan).is_err());
}

#[test]
fn the_vm_folder_chooser_discovers_only_the_current_validated_project_without_starting_a_local_session(
) {
    use caelum_protocol::ProjectInfo;
    let mut app = Workspace::default();
    let choose = |app: &mut Workspace, path: &str| {
        app.remote_picker(path.into());
        app.commands()
            .into_iter()
            .find_map(|(id, c)| matches!(c,Command::Open(p) if p==path).then_some(id))
            .unwrap()
    };
    let old = choose(&mut app, "/alias/old");
    let new = choose(&mut app, "/alias/new");
    let resolved = |req, path: &str| {
        Event::Completed(
            req,
            Ok(Reply::Project(ProjectInfo {
                name: "project".into(),
                path: path.into(),
            })),
        )
    };
    app.apply(resolved(old, "/projects/old"));
    assert!(app.projects.is_empty() && app.commands().is_empty());
    app.apply(resolved(new, "/projects/new"));
    assert_eq!(app.projects.len(), 1);
    assert_eq!(app.projects[0].path, "/projects/new");
    assert!(
        matches!(app.commands().as_slice(),[(_,Command::Remote(Operation::Discover(p)))] if p=="/projects/new")
    );
    assert!(
        app.active_project.is_none()
            && app.sessions.is_empty()
            && app.remote.saved.checkouts.is_empty()
    );
}

#[test]
fn lost_create_reply_reuses_the_persisted_attempt_and_late_prepare_keeps_its_owner() {
    let mut app = Workspace::default();
    let (id, req, plan) = created(&mut app);
    app.apply(Event::Completed(req, Err("connection lost".into())));
    let mut restored = Workspace::default();
    restored.remote.saved = app.snapshot().remote;
    restored.select_remote(&id);
    let (req, Command::Remote(Operation::Create(_, retry))) = restored.commands().pop().unwrap()
    else {
        panic!()
    };
    assert_eq!(retry, plan);
    restored.apply(Event::Completed(
        req,
        Ok(Reply::Remote(Outcome::Machine(machine("vm-a")))),
    ));
    let (prepare, Command::Remote(Operation::Prepare(target, base, Some(branch)))) =
        restored.commands().pop().unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (target.machine.id.as_str(), base.as_str(), branch.as_str()),
        ("vm-a", "main", "feat/new")
    );
    let (other, _, _) = created(&mut restored);
    restored.apply(Event::Completed(
        prepare,
        Ok(Reply::Remote(prepared_vm(machine("vm-a"), branch))),
    ));
    assert_eq!(restored.remote.saved.selected.as_ref(), Some(&other));
    assert!(matches!(
        restored.remote.saved.checkouts[&id].phase,
        Phase::Ready { .. }
    ));
    assert!(matches!(
        restored.remote.saved.checkouts[&other].phase,
        Phase::Create(_)
    ));
    assert!(restored.active_project.is_none());
    assert!(restored.sessions.is_empty());
    let mut reopened = Workspace::default();
    reopened.remote.saved = restored.snapshot().remote;
    reopened.select_remote(&id);
    assert!(matches!(
        reopened.commands().as_slice(),
        [(_, Command::Remote(Operation::Prepare(target, _, None)))]
            if target.machine.id == "vm-a" && target.root.as_deref() == Some("/workspace/checkout")
    ));
}

#[test]
fn reopening_reattaches_to_the_saved_run_without_reposting_the_prompt() {
    use crate::application::Provider;
    let mut app = Workspace::default();
    let (id, req, _) = created(&mut app);
    let finish =
        |app: &mut Workspace, req, out| app.apply(Event::Completed(req, Ok(Reply::Remote(out))));
    finish(&mut app, req, Outcome::Machine(machine("vm-a")));
    let (req, _) = app.commands().pop().unwrap();
    finish(&mut app, req, prepared_vm(machine("vm-a"), "feat/new"));
    assert!(app.set_vm_provider(&id, &id, Provider::Codex));
    assert!(app.send_remote(&id, "Edit a file"));
    let saved_before_ack = app.snapshot();
    let (
        req,
        Command::Remote(Operation::Prompt(
            _,
            Prompt {
                provider: Provider::Codex,
                conversation: None,
                model: None,
                ..
            },
        )),
    ) = app.commands().pop().unwrap()
    else {
        panic!()
    };
    let run = Run {
        provider: Provider::Codex,
        conversation: "c".into(),
        id: "r".into(),
        prompt: "p".into(),
    };
    finish(&mut app, req, Outcome::Accepted(run.clone()));
    let (req, _) = app.commands().pop().unwrap();
    finish(
        &mut app,
        req,
        Outcome::Page(Page {
            cursor: Some("after".into()),
            lines: vec![Line {
                key: None,
                tool_detail: None,
                tool_source: None,
                speaker: Speaker::Codex,
                text: "Part".into(),
                partial: true,
            }],
            more: false,
            status: RunStatus::Running,
        }),
    );
    for (snapshot, known) in [(saved_before_ack, false), (app.snapshot(), true)] {
        let mut reopened = Workspace::default();
        reopened.remote.saved =
            serde_json::from_value(serde_json::to_value(snapshot.remote).unwrap()).unwrap();
        reopened.select_remote(&id);
        let (req, _) = reopened.commands().pop().unwrap();
        finish(&mut reopened, req, prepared_vm(machine("vm-a"), "feat/new"));
        let mut work = reopened.commands();
        assert!(!reopened.set_vm_provider(&id, &id, Provider::OpenCode));
        if known {
            let (req, Command::Remote(Operation::Follow(_, restored, cursor))) =
                work.pop().unwrap()
            else {
                panic!()
            };
            assert_eq!(restored, run);
            assert_eq!(cursor.as_deref(), Some("after"));
            finish(
                &mut reopened,
                req,
                Outcome::Page(Page {
                    cursor: Some("end".into()),
                    lines: vec![Line {
                        key: None,
                        tool_detail: None,
                        tool_source: None,
                        speaker: Speaker::Codex,
                        text: "Full reply".into(),
                        partial: false,
                    }],
                    more: false,
                    status: RunStatus::Ended(TurnEnd::Finished),
                }),
            );
            assert!(!reopened.set_vm_provider(&id, "stale-editor", Provider::OpenCode));
            assert!(reopened.set_vm_provider(&id, &id, Provider::OpenCode));
            let lines = &reopened.remote.saved.checkouts[&id].thread.lines;
            assert_eq!(lines.len(), 3);
            assert_eq!(lines[1].text, "Full reply");
            assert_eq!(lines[1].speaker, Speaker::Codex);
            assert_eq!(lines[2].speaker, Speaker::Outcome(TurnEnd::Finished));
            assert!(reopened.send_remote(&id, "Continue"));
            assert!(
                matches!(reopened.commands().as_slice(),[(_,Command::Remote(Operation::Prompt(_,p)))] if p.provider == Provider::OpenCode && p.conversation.as_deref()==Some("c") && p.model.is_none() && p.text=="Continue")
            );
        } else {
            assert!(work.is_empty());
            assert!(!reopened.send_remote(&id, "Edit a file"));
            reopened.recover_remote(&id, "p");
            assert!(
                matches!(reopened.commands().as_slice(), [(_, Command::Remote(Operation::Recover(_, Provider::Codex, None, p)))] if p=="p")
            );
        }
    }
}

#[test]
fn only_the_visible_monitor_polls_and_stale_samples_cannot_stop_a_new_attempt() {
    use std::time::{Duration, Instant};
    let mut app = Workspace::default();
    let (id, req, _) = created(&mut app);
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(Outcome::Machine(machine("vm-a")))),
    ));
    let (req, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(prepared_vm(machine("vm-a"), "feat/new"))),
    ));
    let now = Instant::now();
    assert!(app.monitor_remote(None, now).is_none() && app.commands().is_empty());
    let sample = || {
        Outcome::Stats(Sample {
            machine: machine("vm-a"),
            vcpu: Some(2),
            memory_gb: Some(4),
            activity: Activity::Unavailable,
        })
    };
    app.monitor_remote(Some(&id), now);
    let (old, _) = app.commands().pop().unwrap();
    app.monitor_remote(None, now);
    app.monitor_remote(Some(&id), now);
    let (new, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(old, Ok(Reply::Remote(sample()))));
    assert!(app.remote.monitor.sample(&id).is_none());
    app.apply(Event::Completed(new, Ok(Reply::Remote(sample()))));
    let next = app.monitor_remote(Some(&id), now).unwrap();
    assert!(app.commands().is_empty());
    assert!(
        app.monitor_remote(None, next + Duration::from_secs(60))
            .is_none()
            && app.commands().is_empty()
    );
}
