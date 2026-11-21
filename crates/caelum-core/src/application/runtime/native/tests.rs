use super::*;
use crate::{
    application::{Event, Options, Provider, Scope, Workspace},
    remote::{Host, MachineId},
};
use std::{sync::mpsc, time::Duration};

fn local(target: &NativeTarget, parent: &std::path::Path) -> (Runtime, mpsc::Receiver<()>) {
    let (wake, rx) = mpsc::channel();
    (
        Runtime::native_desktop(target.clone(), parent.to_owned(), move || {
            let _ = wake.send(());
        }),
        rx,
    )
}

fn request(
    local: &mut (Runtime, mpsc::Receiver<()>),
    id: u64,
    command: Command,
) -> Result<Reply, String> {
    local.0.submit(id, command).unwrap();
    local.1.recv_timeout(Duration::from_secs(5)).unwrap();
    let Some(Event::Completed(owner, result)) = local.0.drain().pop() else {
        panic!("missing completion")
    };
    assert_eq!(owner, id);
    result
}

#[test]
fn offline_drafts_and_options_survive_restart_without_loading_a_laptop_provider_or_store() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("workspace.json");
    let target = NativeTarget {
        machine: MachineId {
            host: Host::Boat,
            account: "account-1".into(),
            id: "vm-example".into(),
        },
        repository: "example/project".into(),
    };
    let store = Store(target.store_path(&parent));
    let root = target.root().unwrap();
    let mut app = Workspace::default();
    app.projects.push(caelum_protocol::ProjectInfo {
        path: root.clone(),
        name: "project".into(),
    });
    let existing = app.new_session(Scope {
        project: root.clone(),
        provider: Provider::OpenCode,
    });
    app.sessions[existing].remote_id = Some("ses_native".into());
    let other = app.new_session(Scope {
        project: root.clone(),
        provider: Provider::Codex,
    });
    app.select(existing);
    app.native_connection_lost(&root);
    app.sessions[existing].draft = "Continue after reconnect".into();
    app.sessions[existing].options = Options {
        model: Some("example/model".into()),
        effort: Some("high".into()),
        agent: Some("plan".into()),
    };
    app.sessions[other].draft = "A parked Codex draft".into();
    let mut writer = local(&target, &parent);
    assert!(request(
        &mut writer,
        1,
        Command::Save(store.clone(), Box::new(app.snapshot()))
    )
    .is_ok());
    assert!(request(&mut writer, 2, Command::Load(Store(parent.clone()))).is_err());
    // Even a project that really exists on this computer is never opened by the local worker.
    assert!(request(
        &mut writer,
        3,
        Command::Open(temp.path().display().to_string())
    )
    .is_err());
    drop(writer);

    let mut reader = local(&target, &parent);
    let mut restored = Workspace::default();
    restored.enable_store(store.0.clone());
    let (id, load) = restored.commands().pop().unwrap();
    restored.apply(Event::Completed(id, request(&mut reader, id, load)));
    assert_eq!(restored.selected, Some(existing));
    assert_eq!(
        restored.sessions[existing].remote_id.as_deref(),
        Some("ses_native")
    );
    assert_eq!(
        restored.sessions[existing].draft,
        app.sessions[existing].draft
    );
    assert_eq!(
        restored.sessions[existing].options,
        app.sessions[existing].options
    );
    assert_eq!(restored.sessions[other].draft, app.sessions[other].draft);
    assert!(restored.sessions[other].remote_id.is_none());
    assert!(!parent.exists());
    assert_ne!(target.store_path(&parent.with_extension("dev")), store.0);
}
