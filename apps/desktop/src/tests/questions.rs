use super::*;

#[test]
fn native_side_questions_preserve_main_draft_focus_and_reject_late_answers() {
    use caelum_core::application::{Command, Event, Reply};
    let mut rt = live_runtime();
    click(&mut rt, "live-session-1");
    rt.update(|s, _| {
        let session = &mut s.live.as_mut().unwrap().app.sessions[1];
        session.remote_id = Some("existing".into());
        session.loaded = true;
        session.conversation.working = true;
    });
    fill(&mut rt, "live-draft-1", "Keep the main draft");
    fill(&mut rt, "live-draft-1", "/btw Why does this work?");
    key(&mut rt, Key::Enter, Mods::default());
    assert!(rt.find("side-question").is_some());
    let commands = rt.state.live.as_mut().unwrap().app.commands();
    let (request, _) = commands
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Generate(..)))
        .unwrap();
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.2);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[1].text,
        "Keep the main draft"
    );
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            request,
            Ok(Reply::Generated("Late answer".into())),
        ))
    });
    assert!(rt.find("side-question").is_none());
    assert!(rt.state.live.as_ref().unwrap().app.sessions[1].working());
    rt.paste(" plus more");
    advance(&mut rt, 0.1);
    assert!(
        rt.state.live.as_ref().unwrap().drafts[1]
            .text
            .contains("plus more")
    );
}

#[test]
fn native_question_forms_send_scoped_values_and_keep_invalid_answers_editable() {
    use caelum_core::application::{Command, Event};
    let mut rt = live_runtime();
    rt.state.live.as_mut().unwrap().app.commands();
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
