use super::*;
use caelum_core::application::{Provider, Scope};

#[test]
fn clicking_a_notification_reopens_the_conversation_without_changing_drafts() {
    let app = crate::lifecycle::configure(rune::App::new(crate::spec()));
    let handle = app.handle();
    let mut s = Workspace::live(None);
    s.handle = Some(handle);
    let live = s.live.as_mut().unwrap();
    live.app.projects.push(caelum_protocol::ProjectInfo {
        path: "/project".into(),
        name: "Project".into(),
    });
    for provider in Provider::ALL {
        let index = live.app.new_session(Scope {
            project: "/project".into(),
            provider,
        });
        live.app.sessions[index].remote_id = Some("same-id".into());
        live.app.sessions[index].loaded = true;
    }
    live.sync_editors();
    live.drafts[0].text = "Original café draft".into();
    live.drafts[0].set_selection(9, 14);
    live.drafts[1].text = "Other draft".into();
    live.app.select(1);
    live.panels.overlay = Some(Screen::Settings);
    let target = live.app.sessions[0].key();
    clicked(&mut s, target);
    let live = s.live.as_mut().unwrap();
    assert!(dispatch(live));
    assert_eq!(live.app.selected, Some(0));
    assert_eq!(live.panels.screen, Screen::Conversation);
    assert!(live.panels.overlay.is_none());
    assert_eq!(live.drafts[0].text, "Original café draft");
    assert_eq!((live.drafts[0].anchor, live.drafts[0].focus), (9, 14));
    assert_eq!(live.drafts[1].text, "Other draft");
    assert!(
        !dispatch(live),
        "idle dispatch must not request another frame"
    );
}
