use super::*;
use crate::live::{live, live_mut};
use caelum_core::remote::native::Status;

#[test]
fn going_offline_preserves_the_editor_and_recovery_only_reads_the_native_session() {
    let mut rt = design_runtime("", (800., 620.));
    rt.update(|s, _| crate::remote::scene(s, "native-connected"));
    advance(&mut rt, 0.1);
    let i = live(&rt.state).app.selected.unwrap();
    let key = live(&rt.state).app.sessions[i].key().unwrap();
    assert_eq!(key.scope.project, "/home/user/project");
    fill(
        &mut rt,
        &format!("live-draft-{i}"),
        "keep this offline draft",
    );
    rt.update(|s, _| {
        let l = live_mut(s);
        l.app.native_connection_lost(&key.scope.project);
        l.native.as_mut().unwrap().status = Status::Disconnected("VM disconnected".into());
    });
    advance(&mut rt, 0.1);
    click(&mut rt, "send");
    assert!(rt.find("native-workspace-reconnect").is_some());
    click(&mut rt, &format!("live-draft-{i}"));
    super::key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.1);
    assert_eq!(live(&rt.state).drafts[i].text, "keep this offline draft");
    let app = &mut live_mut(&mut rt.state).app;
    app.refresh_open_sessions();
    let requests = app.commands();
    assert!(
        requests
            .iter()
            .any(|(_, c)| matches!(c,Command::Resume(k) if k==&key))
    );
    assert!(!requests.iter().any(|(_, c)| matches!(
        c,
        Command::Send(..) | Command::Start(..) | Command::Enqueue(..) | Command::Remote(_)
    )));
    assert_eq!(app.sessions[i].key().as_ref(), Some(&key));
}
