use super::*;
use crate::{
    live::{live, live_mut},
    panels::Screen,
};
use caelum_core::remote::{Chat, Host, Machine, MachineId, Message};

fn machine() -> Machine {
    Machine {
        key: MachineId {
            host: Host::Boat,
            account: "example".into(),
            id: "vm-fixture".into(),
        },
        name: "test-vm".into(),
        state: "idle".into(),
        auto_stop_unix: None,
    }
}

fn inventory() -> Inventory {
    Inventory {
        account: "example".into(),
        machines: vec![machine()],
    }
}

fn prepared_vm(branch: &str) -> Outcome {
    Outcome::Prepared {
        machine: machine(),
        branch: branch.into(),
        root: "/workspace/checkout".into(),
    }
}

fn prepare(rt: &mut Runtime<Workspace>, expected_project: &str, read_chat: bool) {
    rt.update(|s, _| {
        let l = live_mut(s);
        let mut did_read = false;
        let mut discovered = false;
        for (request, cmd) in l.app.commands() {
            let reply = match cmd {
                Command::HandoffChat(key) => {
                    assert_eq!(
                        (key.scope.project.as_str(), key.id.as_str()),
                        (expected_project, "session-a")
                    );
                    did_read = true;
                    Reply::HandoffChat(Chat {
                        messages: vec![
                            Message::User("Prior question".into()),
                            Message::Assistant("Prior answer".into()),
                        ],
                        clipped: false,
                    })
                }
                Command::Remote(Operation::DiscoverHandoff(project)) => {
                    assert_eq!(project, expected_project);
                    discovered = true;
                    Reply::Remote(Outcome::Source(Source {
                        published_tip: Some("b".repeat(40)),
                        project,
                        account: "example".into(),
                        repository: "example/repo".into(),
                        base: "feature/session".into(),
                        environment: None,
                    }))
                }
                Command::Remote(Operation::Inventory) => {
                    Reply::Remote(Outcome::Inventory(inventory()))
                }
                Command::Remote(Operation::Create(..) | Operation::Prompt(..)) => {
                    panic!("unexpected VM or prompt")
                }
                _ => continue,
            };
            l.apply(Event::Completed(request, Ok(reply)));
        }
        assert!(discovered);
        assert_eq!(did_read, read_chat);
    });
    advance(rt, 0.2);
}

fn confirm(rt: &mut Runtime<Workspace>) {
    rt.update(|s, _| {
        let l = live_mut(s);
        let Some(caelum_core::remote::Picker::Ready(source)) = l.app.remote.picker.clone() else {
            panic!()
        };
        let cmds = l.app.commands();
        let [(req, Command::Remote(Operation::DiscoverHandoff(path)))] = cmds.as_slice() else {
            panic!("{cmds:?}")
        };
        assert_eq!(path, &source.project);
        l.apply(Event::Completed(
            *req,
            Ok(Reply::Remote(Outcome::Source(source))),
        ));
    });
    advance(rt, 0.2);
}

#[test]
fn composer_handoff_can_be_cancelled_then_continued_on_a_vm_without_sending() {
    let mut rt = live_runtime_size((800., 620.));
    rt.update(|s, _| {
        let l = live_mut(s);
        l.app.sessions[0].remote_id = Some("session-a".into());
        l.app.remote.inventory = Some(inventory());
    });
    fill(&mut rt, "live-draft-0", "Edit this on the VM");
    let button = element(&rt, "continue-on-vm");
    assert_eq!(button.label.as_deref(), Some("Continue on VM…"));
    assert!(button.visible[3] >= 28. && button.path.contains("composer"));
    let context = element(&rt, "context-usage").rect;
    assert!(button.text.is_none() && button.rect[2] <= 32.);
    assert!((context[0] - (button.rect[0] + button.rect[2])).abs() <= 6.);
    click(&mut rt, "continue-on-vm");
    assert_eq!(live(&rt.state).panels.overlay, Some(Screen::VmHandoff));
    let popup = element(&rt, "vm-popover");
    assert!(popup.rect[1] + popup.rect[3] <= button.rect[1] && popup.rect[2] <= 420.);
    let old = live_mut(&mut rt.state)
        .app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::HandoffChat(_)))
        .unwrap()
        .0;
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.2);
    assert!(live(&rt.state).app.vm_handoff().is_none());
    click(&mut rt, "continue-on-vm");
    rt.update(|s, _| live_mut(s).apply(Event::Completed(old, Err("Old read failed".into()))));
    prepare(&mut rt, "/project", true);
    click(&mut rt, "attach-vm-vm-fixture");
    confirm(&mut rt);
    rt.update(|s, _| {
        let l = live_mut(s);
        assert_eq!(l.panels.screen, Screen::Remote);
        let cmds = l.app.commands();
        let [(req, Command::Remote(Operation::Prepare(_, _, None)))] = cmds.as_slice() else {
            panic!("{cmds:?}")
        };
        l.apply(Event::Completed(
            *req,
            Ok(Reply::Remote(prepared_vm("feat/existing"))),
        ));
    });
    advance(&mut rt, 0.2);
    let l = live(&rt.state);
    let id = l.app.remote.saved.selected.as_ref().unwrap();
    let checkout = &l.app.remote.saved.checkouts[id];
    assert!(checkout.other_threads.is_empty());
    let thread = &checkout.thread;
    assert_eq!(thread.draft, "Edit this on the VM");
    assert_eq!(l.drafts[0].text, thread.draft);
    let handoff_row = format!("vm-session-{id}/{}", thread.editor_key(id));
    assert!(element(&rt, &handoff_row).item_selected);
    assert!(!element(&rt, "live-session-0").item_selected);
    assert!(
        rt.find(&format!("remote-draft-{}", thread.editor_key(id)))
            .is_some()
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| !e.exiting && e.text.as_deref() == Some("Prior answer"))
    );
    assert!(
        live_mut(&mut rt.state)
            .app
            .commands()
            .iter()
            .all(|(_, c)| !matches!(c, Command::Remote(Operation::Prompt(..))))
    );
    click(&mut rt, "live-session-0");
    click(&mut rt, "continue-on-vm");
    assert_eq!(live(&rt.state).panels.overlay, Some(Screen::VmStats));
    assert_eq!(
        element(&rt, "vm-popover").label.as_deref(),
        Some("Machine stats")
    );
    let l = live_mut(&mut rt.state);
    assert_eq!(l.app.selected, Some(0));
    assert!(l.app.remote.saved.selected.is_none());
    let cmds = l.app.commands();
    assert!(
        cmds.iter().any(
            |(_, c)| matches!(c, Command::Remote(Operation::Stats(m)) if m.id == "vm-fixture")
        )
    );
    assert!(!cmds.iter().any(|(_, c)| matches!(
        c,
        Command::Remote(Operation::DiscoverHandoff(_) | Operation::Create(..))
    )));
    click(&mut rt, "open-linked-vm");
    assert_eq!(live(&rt.state).panels.screen, Screen::Remote);
    assert_eq!(live(&rt.state).drafts[0].text, "Edit this on the VM");
    let id = live(&rt.state).app.remote.saved.selected.clone().unwrap();
    rt.update(|s, _| {
        let l = live_mut(s);
        let req = l
            .app
            .commands()
            .into_iter()
            .find_map(|(req, command)| {
                matches!(command, Command::Remote(Operation::Prepare(..))).then_some(req)
            })
            .unwrap();
        l.apply(Event::Completed(
            req,
            Ok(Reply::Remote(prepared_vm("feat/existing"))),
        ));
    });
    advance(&mut rt, 0.2);
    click(&mut rt, &format!("remove-remote-{id}"));
    click(&mut rt, "confirm-remove-vm");
    rt.update(|s, _| {
        let l = live_mut(s);
        let (r, Command::Remote(Operation::Delete(key))) = l.app.commands().pop().unwrap() else {
            panic!()
        };
        assert_eq!(key, machine().key);
        assert!(l.app.remote.saved.checkouts.contains_key(&id));
        l.apply(Event::Completed(
            r,
            Ok(Reply::Remote(Outcome::Deleted(key))),
        ));
    });
    advance(&mut rt, 0.2);
    let l = live(&rt.state);
    assert_eq!(l.panels.screen, Screen::Conversation);
    assert_eq!(l.app.selected, Some(0));
    assert_eq!(l.drafts[0].text, "Edit this on the VM");
    assert!(l.app.linked_vm(0).is_none());
    assert!(rt.find("continue-on-vm").is_some());
}

#[test]
fn vm_setup_occupies_the_composer_until_the_prepared_checkout_restores_the_draft() {
    let mut rt = live_runtime_size((800., 620.));
    rt.update(|s, _| {
        let l = live_mut(s);
        l.app.sessions[0].remote_id = Some("session-a".into());
        l.app.remote.inventory = Some(inventory());
    });
    fill(&mut rt, "live-draft-0", "Keep this draft");
    let dock = element(&rt, "message-composer").rect;
    let same_dock = |rect: [f32; 4]| {
        for (a, b) in [
            (rect[0], dock[0]),
            (rect[2], dock[2]),
            (rect[1] + rect[3], dock[1] + dock[3]),
        ] {
            assert!((a - b).abs() < 1., "dock {rect:?} vs {dock:?}");
        }
    };
    click(&mut rt, "continue-on-vm");
    prepare(&mut rt, "/project", true);
    click(&mut rt, "handoff-new-vm");
    reveal(&mut rt, "create-remote");
    click(&mut rt, "create-remote");
    confirm(&mut rt);
    let l = live(&rt.state);
    let id = l.app.remote.saved.selected.as_ref().unwrap();
    let owner = l.app.remote.saved.checkouts[id]
        .thread
        .editor_key(id)
        .to_owned();
    let editor = format!("remote-draft-{owner}");
    let transcript = format!("remote-transcript-{owner}");
    same_dock(element(&rt, "remote-setup-composer").rect);
    assert_eq!(
        element(&rt, "remote-setup-status").label.as_deref(),
        Some("Creating VM…")
    );
    assert!(rt.find(&editor).is_none() && rt.find("remote-send").is_none());
    assert!(rt.elements().iter().any(|e| e.path.contains(&transcript)
        && e.text.as_deref() == Some("Prior answer")
        && e.visible[3] > 0.));

    rt.update(|s, _| {
        let l = live_mut(s);
        let commands = l.app.commands();
        let [(req, Command::Remote(Operation::Create(..)))] = commands.as_slice() else {
            panic!("{commands:?}")
        };
        l.apply(Event::Completed(
            *req,
            Ok(Reply::Remote(Outcome::Machine(machine()))),
        ));
    });
    advance(&mut rt, 0.1);
    assert_eq!(
        element(&rt, "remote-setup-status").label.as_deref(),
        Some("Preparing checkout…")
    );
    assert!(rt.find(&editor).is_none());
    rt.update(|s, _| {
        let l = live_mut(s);
        let commands = l.app.commands();
        let [(req, Command::Remote(Operation::Prepare(..)))] = commands.as_slice() else {
            panic!("{commands:?}")
        };
        l.apply(Event::Completed(
            *req,
            Ok(Reply::Remote(prepared_vm("feature/session"))),
        ));
    });
    advance(&mut rt, 0.5);
    same_dock(element(&rt, "message-composer").rect);
    assert!(rt.find("remote-setup-composer").is_none());
    let input = element(&rt, &editor);
    assert!(input.focused);
    assert_eq!(input.text.as_deref(), Some("Keep this draft"));
    assert!(rt.find(&transcript).is_some() && element(&rt, "remote-send").focusable);
    assert_eq!(live(&rt.state).drafts[0].text, "Keep this draft");
    assert!(
        live_mut(&mut rt.state)
            .app
            .commands()
            .iter()
            .all(|(_, c)| !matches!(c, Command::Remote(Operation::Prompt(..))))
    );
}

#[test]
fn home_handoff_uses_the_selected_drafts_project_after_connecting_and_creates_only_on_confirm() {
    let mut rt = live_runtime();
    click(&mut rt, "nav-home");
    rt.update(|s, _| {
        let l = live_mut(s);
        // A fresh launch can restore a background selection while Home stays open.
        l.app.select(1);
        l.app.projects.push(caelum_protocol::ProjectInfo {
            name: "Other".into(),
            path: "/other".into(),
        });
        l.app.home_select_project("/other".into());
        assert_eq!(l.app.active_project.as_deref(), Some("/project"));
    });
    fill(&mut rt, "home-draft", "My Home idea");
    click(&mut rt, "continue-on-vm");
    click(&mut rt, "vm-home-draft");
    rt.update(|s, _| {
        let l = live_mut(s);
        let req = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Remote(Operation::Inventory)))
            .unwrap()
            .0;
        l.apply(Event::Completed(req, Err("No saved account".into())));
    });
    advance(&mut rt, 0.2);
    fill(&mut rt, "boat-api-key", "fixture");
    click(&mut rt, "connect-boat");
    rt.update(|s, _| {
        let l = live_mut(s);
        let req = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Remote(Operation::Connect(_))))
            .unwrap()
            .0;
        l.apply(Event::Completed(
            req,
            Ok(Reply::Remote(Outcome::Inventory(inventory()))),
        ));
    });
    prepare(&mut rt, "/other", false);
    assert!(live(&rt.state).app.remote.saved.checkouts.is_empty());
    click(&mut rt, "handoff-new-vm");
    assert!(rt.find("remote-base").is_none());
    assert!(rt.find("remote-branch").is_none());
    assert_eq!(
        element(&rt, "handoff-base").text.as_deref(),
        Some("feature/session")
    );
    assert!(element(&rt, "create-remote").focusable);
    reveal(&mut rt, "vm-lifetime-24 h");
    click(&mut rt, "vm-lifetime-24 h");
    reveal(&mut rt, "create-remote");
    click(&mut rt, "create-remote");
    confirm(&mut rt);
    let l = live(&rt.state);
    let c = &l.app.remote.saved.checkouts[l.app.remote.saved.selected.as_ref().unwrap()];
    assert_eq!(c.source.project, "/other");
    assert_eq!(c.source.base, "feature/session");
    assert!(matches!(&c.phase, caelum_core::remote::Phase::Create(p)
            if p.branch == "feature/session" && p.lifetime == caelum_core::remote::Lifetime::Day));
    assert_eq!(c.thread.draft, "My Home idea");
    assert!(
        c.thread
            .continued
            .as_ref()
            .unwrap()
            .chat
            .messages
            .is_empty()
    );
    assert_eq!(l.home.editor.text, c.thread.draft);
    assert_eq!(l.app.sessions.len(), 2);
}

#[test]
fn choosing_a_recent_session_on_home_loads_its_chat_and_its_own_editor() {
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let l = live_mut(s);
        l.app.sessions[0].remote_id = Some("session-a".into());
        l.app.remote.inventory = Some(inventory());
    });
    fill(&mut rt, "live-draft-0", "Session draft");
    click(&mut rt, "nav-home");
    rt.update(|s, _| live_mut(s).app.select(1));
    fill(&mut rt, "home-draft", "Home draft");
    click(&mut rt, "continue-on-vm");
    click(&mut rt, "vm-session-0");
    prepare(&mut rt, "/project", true);
    assert_eq!(live(&rt.state).app.selected, Some(1));
    click(&mut rt, "attach-vm-vm-fixture");
    confirm(&mut rt);
    let l = live(&rt.state);
    let thread =
        &l.app.remote.saved.checkouts[l.app.remote.saved.selected.as_ref().unwrap()].thread;
    assert_eq!(thread.draft, "Session draft");
    assert_eq!(thread.continued.as_ref().unwrap().chat.messages.len(), 2);
    assert_eq!(l.home.editor.text, "Home draft");
}

#[test]
fn switching_threads_on_the_same_vm_restores_the_native_draft_and_undo_owner() {
    use caelum_core::remote::{Continuation, Thread};
    let mut rt = design_runtime("", (1280., 900.));
    rt.update(|s, _| {
        crate::remote::scene(s, "remote-machine");
        let c = live_mut(s)
            .app
            .remote
            .saved
            .checkouts
            .get_mut("checkout-example")
            .unwrap();
        c.other_threads.push(Thread {
            continued: Some(Continuation {
                base: None,
                id: "other-editor".into(),
                title: "Local session".into(),
                chat: Chat::default(),
            }),
            draft: "Local draft".into(),
            ..Default::default()
        });
    });
    advance(&mut rt, 0.2);
    key(&mut rt, Key::Escape, Mods::default());
    fill(&mut rt, "remote-draft-checkout-example", "Original edit");
    click(&mut rt, "vm-threads");
    click(&mut rt, "vm-thread-other-editor");
    fill(&mut rt, "remote-draft-other-editor", "New edit");
    click(&mut rt, "vm-threads");
    click(&mut rt, "vm-thread-checkout-example");
    click(&mut rt, "remote-draft-checkout-example");
    key(
        &mut rt,
        Key::Command('z'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    let c = &live(&rt.state).app.remote.saved.checkouts["checkout-example"];
    assert_eq!(c.thread.conversation.as_deref(), Some("thread-example"));
    assert!(c.thread.draft.is_empty());
    assert_eq!(c.other_threads[0].draft, "New edit");
}
