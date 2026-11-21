mod handoff;
mod models;
mod native;
mod transcript;
mod workspace;

use super::*;
use caelum_core::{
    application::{Command, Event, Provider, Reply},
    remote::{Inventory, Operation, Outcome, Source},
};

#[test]
fn small_destination_picker_keeps_creation_separate_from_attaching() {
    let mut rt = design_runtime("", (510., 440.));
    rt.update(|s, _| crate::remote::scene(s, "remote-picker"));
    advance(&mut rt, 0.2);
    assert!(rt.find("attach-vm-vm-example").is_some() && rt.find("create-remote").is_none());
    let machine = element(&rt, "attach-vm-vm-example");
    assert!(machine.visible[3] >= machine.rect[3] - 1.);
    click(&mut rt, "handoff-new-vm");
    advance(&mut rt, 0.45);
    assert!(rt.find("attach-vm-vm-example").is_none());
    let initial = element(&rt, "create-remote");
    assert!(initial.visible[3] >= 28.);
    reveal(&mut rt, "vm-lifetime-Until stopped");
    click(&mut rt, "vm-lifetime-Until stopped");
    let footer = element(&rt, "create-remote");
    assert!(
        (footer.rect[1] - initial.rect[1]).abs() < 1. && footer.visible[3] >= 28.,
        "{footer:?}"
    );
    assert!(footer.focusable);
    click(&mut rt, "handoff-existing-vm");
    assert!(rt.find("attach-vm-vm-example").is_some() && rt.find("create-remote").is_none());
    let app = &mut rt.state.live.as_mut().unwrap().app;
    assert!(app.remote.saved.checkouts.is_empty());
    assert!(app.commands().iter().all(|(_, c)| !matches!(
        c,
        Command::Remote(Operation::Create(..) | Operation::Prompt(..))
    )));
}

#[test]
fn settings_create_vm_opens_the_form_and_only_provisions_on_submit() {
    let mut rt = design_runtime("", (800., 620.));
    rt.update(|s, _| crate::remote::scene(s, "remote-settings"));
    advance(&mut rt, 0.2);
    let entry = element(&rt, "open-create-vm");
    assert_eq!(entry.label.as_deref(), Some("Create VM…"));
    assert!(entry.visible[3] >= 28.);
    click(&mut rt, "open-create-vm");
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        assert_eq!(l.panels.overlay, Some(crate::panels::Screen::CreateVm));
        assert!(l.app.remote.saved.checkouts.is_empty());
        let (id, project) = l
            .app
            .commands()
            .into_iter()
            .find_map(|(id, command)| match command {
                Command::Remote(Operation::Discover(project)) => Some((id, project)),
                _ => None,
            })
            .unwrap();
        assert_eq!(Some(&project), l.app.active_project.as_ref());
        l.apply(Event::Completed(
            id,
            Ok(Reply::Remote(Outcome::Source(Source {
                published_tip: Some("a".repeat(40)),
                account: "example-account".into(),
                project,
                repository: "example/project".into(),
                base: "feat/remote-ui".into(),
                environment: None,
            }))),
        ));
    });
    advance(&mut rt, 0.2);
    assert_eq!(
        element(&rt, "handoff-base").text.as_deref(),
        Some("feat/remote-ui")
    );
    click(&mut rt, "vm-size-Large");
    click(&mut rt, "vm-lifetime-Until stopped");
    assert!(
        element(&rt, "vm-lifetime-detail")
            .text
            .unwrap()
            .contains("until you stop")
    );
    let submit = element(&rt, "create-remote");
    assert_eq!(submit.label.as_deref(), Some("Create VM"));
    assert!(submit.focusable && submit.visible[3] >= 28.);
    click(&mut rt, "create-remote");
    let l = rt.state.live.as_mut().unwrap();
    assert_eq!(l.panels.screen, crate::panels::Screen::Remote);
    assert!(l.panels.overlay.is_none());
    let creates: Vec<_> = l
        .app
        .commands()
        .into_iter()
        .filter_map(|(req, c)| match c {
            Command::Remote(Operation::Create(s, p)) => Some((req, s, p)),
            _ => None,
        })
        .collect();
    let [(req, source, plan)] = creates.as_slice() else {
        panic!("{creates:?}")
    };
    assert_eq!(source.repository, "example/project");
    assert_eq!(source.base, "feat/remote-ui");
    assert_eq!(plan.branch, "feat/remote-ui");
    assert_eq!(plan.size, caelum_core::remote::Size::Large);
    assert_eq!(plan.lifetime, caelum_core::remote::Lifetime::UntilStopped);

    let denied = "Create checkout environment: Boat returned HTTP 403. Automatic setup needs environment.write.";
    rt.update(|s, _| {
        s.live
            .as_mut()
            .unwrap()
            .apply(Event::Completed(*req, Err(denied.into())))
    });
    advance(&mut rt, 0.2);
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.path.contains("remote-setup-composer")
                && e.text.as_deref() == Some(denied)
                && e.visible[3] > 0.)
    );
    assert!(
        rt.elements()
            .iter()
            .all(|e| e.text.as_deref() != Some("Creating machine…") || e.exiting)
    );
    click(&mut rt, "refresh-remote-checkout");
    let commands = rt.state.live.as_mut().unwrap().app.commands();
    assert!(
        matches!(commands.as_slice(), [(_, Command::Remote(Operation::Create(s, p)))] if s == source && p == plan)
    );
}

#[test]
fn a_launcher_provider_survives_changing_the_project_before_creating_a_vm() {
    use crate::live::{live, live_mut};
    let mut rt = design_runtime("", (800., 620.));
    rt.update(|s, _| {
        crate::remote::scene(s, "remote-settings");
        crate::panels::open_launcher(s);
    });
    advance(&mut rt, 0.2);
    click(&mut rt, "launch-provider-codex");
    click(&mut rt, "open-create-vm");
    let publish_source = |rt: &mut Runtime<Workspace>| {
        rt.update(|s, _| {
            let l = live_mut(s);
            let (r, project) = l
                .app
                .commands()
                .into_iter()
                .find_map(|(r, op)| match op {
                    Command::Remote(Operation::Discover(p)) => Some((r, p)),
                    _ => None,
                })
                .unwrap();
            l.apply(Event::Completed(
                r,
                Ok(Reply::Remote(Outcome::Source(Source {
                    project,
                    account: "example-account".into(),
                    repository: "example/project".into(),
                    base: "feature/launcher".into(),
                    published_tip: Some("a".repeat(40)),
                    environment: None,
                }))),
            ));
        });
        advance(rt, 0.2);
    };
    publish_source(&mut rt);
    click(&mut rt, "change-vm-project");
    let project = &live(&rt.state).app.projects[0].path;
    let choice = crate::code_view::source_key(project, "vm-project");
    click(&mut rt, &choice);
    publish_source(&mut rt);
    click(&mut rt, "create-remote");
    let app = &mut live_mut(&mut rt.state).app;
    let c = &app.remote.saved.checkouts[app.remote.saved.selected.as_ref().unwrap()];
    assert_eq!(c.thread.provider, Provider::Codex);
    assert_eq!(c.branch(), Some("feature/launcher"));
    assert!(c.thread.conversation.is_none());
    assert!(matches!(
        app.commands().as_slice(),
        [(_, Command::Remote(Operation::Create(..)))]
    ));
}

#[test]
fn launcher_create_vm_can_connect_and_choose_a_folder_from_an_empty_workspace() {
    let mut rt = design_runtime("", (800., 620.));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.leave_conversation();
        l.app.active_project = None;
        l.app.projects.clear();
        crate::panels::open_launcher(s);
    });
    advance(&mut rt, 0.2);
    click(&mut rt, "open-create-vm");
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let (id, _) = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Remote(Operation::Inventory)))
            .unwrap();
        l.apply(Event::Completed(id, Err("No saved Boat key".into())));
    });
    advance(&mut rt, 0.2);
    fill(&mut rt, "boat-api-key", "fixture-key");
    click(&mut rt, "connect-boat");
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let (id, _) = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Remote(Operation::Connect(_))))
            .unwrap();
        l.apply(Event::Completed(
            id,
            Ok(Reply::Remote(Outcome::Inventory(Inventory {
                account: "example-account".into(),
                machines: vec![],
            }))),
        ));
    });
    advance(&mut rt, 0.2);
    assert!(element(&rt, "browse-vm-project").visible[3] >= 28.);
    assert!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .remote
            .saved
            .checkouts
            .is_empty()
    );
}

#[test]
fn remote_composer_and_workspace_menu_target_the_vm_when_the_local_checkout_was_open() {
    let mut rt = design_runtime("", (1280., 900.));
    rt.update(|s, _| crate::remote::scene(s, "remote-machine"));
    advance(&mut rt, 0.3);
    let badge = element(&rt, "remote-badge");
    assert!(
        (26.0..=36.0).contains(&badge.visible[2]) && badge.visible[3] >= 26.,
        "{badge:?}"
    );
    assert!(rt.find("remote-pin-stats").is_some());
    key(&mut rt, Key::Escape, Mods::default());
    fill(
        &mut rt,
        "remote-draft-checkout-example",
        "Review the remote branch",
    );
    click(&mut rt, "vm-provider-trigger");
    click(&mut rt, "option0");
    let thread =
        &rt.state.live.as_ref().unwrap().app.remote.saved.checkouts["checkout-example"].thread;
    assert_eq!(thread.provider, Provider::Codex);
    assert_eq!(thread.draft, "Review the remote branch");
    assert_eq!(
        thread.lines[1].speaker,
        caelum_core::remote::Speaker::OpenCode
    );
    click(&mut rt, "remote-draft-checkout-example");
    key(&mut rt, Key::Enter, Mods::default());
    let app = &mut rt.state.live.as_mut().unwrap().app;
    assert!(app.commands().iter().any(|(_,op)|matches!(op,Command::Remote(Operation::Prompt(target,p)) if target.machine.id=="vm-example" && p.provider==Provider::Codex && p.conversation.as_deref()==Some("thread-example") && p.text=="Review the remote branch")));
    assert!(app.active_project.is_none());
    assert!(!element(&rt, "vm-provider-trigger").focusable);
    choose_workspace_action(&mut rt, "View remote changes");
    assert!(rt.state.live.as_mut().unwrap().app.commands().iter().any(|(_,op)|matches!(op,Command::Remote(Operation::Git(target,_)) if target.machine.id=="vm-example")));
}

#[test]
fn compact_remote_review_keeps_both_actions_and_the_machine_badge_visible() {
    let mut rt = design_runtime("", (800., 620.));
    rt.update(|s, _| crate::remote::scene(s, "remote-changes"));
    advance(&mut rt, 0.4);
    for key in ["remote-commit", "remote-push", "remote-badge"] {
        let e = element(&rt, key);
        assert!(
            e.rect
                .iter()
                .zip(e.visible)
                .all(|(a, b)| (*a - b).abs() < 1.),
            "{key}: {e:?}"
        );
    }
    assert!(rt.find("remote-pinned-stats").is_none());
}
