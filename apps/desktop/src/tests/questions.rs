use super::*;
use caelum_core::application::{Attachment, Command, Event, Reply};
use caelum_protocol::QuestionPrompt;
use serde_json::json;

fn prompt(provider: &str, session: &str, request: &str) -> QuestionPrompt {
    QuestionPrompt {
        provider: provider.into(),
        project_path: "/project".into(),
        session_id: session.into(),
        request_id: request.into(),
        title: "What should we work on next?".into(),
        fields: vec![json!({"key":"answer","label":"Your answer","type":"string","required":true})],
    }
}

fn questions(rt: &mut Runtime<Workspace>, prompts: Vec<QuestionPrompt>) {
    rt.update(|s, _| s.live.as_mut().unwrap().apply(Event::Questions(prompts)));
    advance(rt, 0.3);
}

fn replies(rt: &mut Runtime<Workspace>) -> Vec<(u64, Command)> {
    rt.state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .into_iter()
        .filter(|(_, command)| matches!(command, Command::Answer(..) | Command::Cancel(..)))
        .collect()
}

#[test]
fn question_dock_restores_the_exact_draft_after_a_failed_answer_and_retry() {
    let mut rt = live_runtime();
    let original = "Keep café ☁️ and its selection.\n".repeat(60);
    fill(&mut rt, "live-draft-0", &original);
    rt.update(|s, tx| {
        let l = s.live.as_mut().unwrap();
        l.app.sessions[0].remote_id = Some("thread".into());
        l.app.sessions[0].loaded = true;
        l.app.attach(
            0,
            Attachment::new("src/main.rs".into(), "saved context".into(), None, false),
        );
        l.drafts[0].set_selection(5, 10);
        tx.scroll_to_y("composer-editor", 260.0);
    });
    advance(&mut rt, 0.3);
    let l = rt.state.live.as_ref().unwrap();
    let selection = (l.drafts[0].anchor, l.drafts[0].focus);
    let offset = l.composer.editors[0].position.get().offset[1];
    assert!(offset > 200.0);
    let request = prompt("codex", "thread", "question");
    questions(&mut rt, vec![request]);
    assert!(rt.find("message-composer").is_none());
    assert!(rt.find("send").is_none());
    assert!(rt.find("model-trigger").is_none());
    assert!(element(&rt, "question-input-question-answer").focused);
    assert!(
        !element(&rt, "question-composer")
            .path
            .contains("live-transcript-0")
    );
    rt.paste("Use the native glass dock");
    key(
        &mut rt,
        Key::Enter,
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    let sent = replies(&mut rt);
    assert!(
        matches!(sent.as_slice(), [(_, Command::Answer(p, Some(values)))]
        if p.session_id == "thread" && values["answer"] == "Use the native glass dock")
    );
    rt.paste("must not edit a pending answer");
    advance(&mut rt, 0.1);
    assert_eq!(
        element(&rt, "question-input-question-answer")
            .text
            .as_deref(),
        Some("Use the native glass dock")
    );
    key(
        &mut rt,
        Key::Enter,
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    click(&mut rt, "question-submit-question");
    assert!(replies(&mut rt).is_empty());
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            sent[0].0,
            Err("Connection interrupted".into()),
        ))
    });
    advance(&mut rt, 0.2);
    assert!(rt.find("question-error").is_some());
    assert_eq!(
        element(&rt, "question-input-question-answer")
            .text
            .as_deref(),
        Some("Use the native glass dock")
    );
    fill(
        &mut rt,
        "question-input-question-answer",
        "Retry with this answer",
    );
    click(&mut rt, "question-submit-question");
    let retried = replies(&mut rt);
    assert!(
        matches!(retried.as_slice(), [(_, Command::Answer(_, Some(values)))] if values["answer"] == "Retry with this answer")
    );
    rt.update(|s, _| {
        s.live
            .as_mut()
            .unwrap()
            .apply(Event::Completed(retried[0].0, Ok(Reply::Done)))
    });
    advance(&mut rt, 0.1);
    assert!(
        rt.find("question-composer").is_some(),
        "wait for provider resolution"
    );
    questions(&mut rt, vec![]);
    assert!(rt.find("question-composer").is_none());
    assert!(element(&rt, "live-draft-0").focused);
    let l = rt.state.live.as_ref().unwrap();
    assert_eq!(l.drafts[0].text, original);
    assert_eq!((l.drafts[0].anchor, l.drafts[0].focus), selection);
    assert_eq!(l.composer.editors[0].position.get().offset[1], offset);
    assert_eq!(l.app.sessions[0].attachments[0].content, "saved context");
    key(
        &mut rt,
        Key::Command('z'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    assert!(
        rt.state.live.as_ref().unwrap().drafts[0].text.is_empty(),
        "the original native undo history survives the question"
    );
}

#[test]
fn question_drafts_and_pending_requests_stay_with_their_session() {
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.sessions[0].remote_id = Some("thread".into());
        l.app.sessions[1].remote_id = Some("session".into());
    });
    let first = prompt("codex", "thread", "z-first");
    let next = prompt("codex", "thread", "a-next");
    let other = prompt("opencode", "session", "z-first");
    questions(&mut rt, vec![first.clone()]);
    fill(
        &mut rt,
        "question-input-z-first-answer",
        "First session draft",
    );
    questions(&mut rt, vec![next.clone(), other.clone(), first.clone()]);
    assert!(rt.find("question-input-a-next-answer").is_none());
    assert_eq!(
        element(&rt, "question-count").text.as_deref(),
        Some("2 pending")
    );
    click(&mut rt, "question-submit-z-first");
    let first_send = replies(&mut rt)[0].0;
    click(&mut rt, "live-session-1");
    assert_eq!(
        element(&rt, "question-input-z-first-answer")
            .text
            .as_deref(),
        Some("")
    );
    fill(
        &mut rt,
        "question-input-z-first-answer",
        "Other session draft",
    );
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            first_send,
            Err("First session failed".into()),
        ))
    });
    advance(&mut rt, 0.2);
    assert!(rt.find("question-error").is_none());
    assert_eq!(
        element(&rt, "question-input-z-first-answer")
            .text
            .as_deref(),
        Some("Other session draft")
    );
    click(&mut rt, "live-session-0");
    assert_eq!(
        element(&rt, "question-input-z-first-answer")
            .text
            .as_deref(),
        Some("First session draft")
    );
    assert!(rt.find("question-error").is_some());
    key(&mut rt, Key::Escape, Mods::default());
    let dismissed = replies(&mut rt);
    assert!(
        matches!(dismissed.as_slice(), [(_, Command::Answer(p, None))] if p.provider == "codex" && p.request_id == "z-first")
    );
    questions(&mut rt, vec![other.clone(), next.clone()]);
    assert!(rt.find("question-input-a-next-answer").is_some());
    assert!(rt.find("question-input-z-first-answer").is_none());
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            dismissed[0].0,
            Err("Obsolete failure".into()),
        ))
    });
    advance(&mut rt, 0.2);
    assert!(rt.find("question-error").is_none());
    questions(&mut rt, vec![other]);
    assert!(rt.find("message-composer").is_some());
    click(&mut rt, "live-session-1");
    assert_eq!(
        element(&rt, "question-input-z-first-answer")
            .text
            .as_deref(),
        Some("Other session draft")
    );
}

#[test]
fn question_choices_take_keyboard_focus_and_escape_dismisses_only_the_question() {
    let mut rt = live_runtime();
    fill(&mut rt, "live-draft-0", "Keep my next message");
    rt.update(|s, _| {
        let session = &mut s.live.as_mut().unwrap().app.sessions[0];
        session.remote_id = Some("thread".into());
        session.conversation.working = true;
    });
    let mut request = prompt("codex", "thread", "choice");
    request.fields = vec![
        json!({"key":"answer","type":"string","required":true,"options":[{"value":"small","label":"Small"},{"value":"full","label":"Full"}]}),
    ];
    questions(&mut rt, vec![request]);
    assert!(element(&rt, "question-choice-answer-0").focused);
    key(&mut rt, Key::Down, Mods::default());
    assert!(element(&rt, "question-choice-answer-1").focused);
    key(
        &mut rt,
        Key::Enter,
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    let sent = replies(&mut rt);
    assert!(
        matches!(sent.as_slice(), [(_, Command::Answer(_, Some(values)))] if values["answer"] == "full")
    );
    rt.update(|s, _| {
        s.live
            .as_mut()
            .unwrap()
            .apply(Event::Completed(sent[0].0, Err("Try again".into())))
    });
    advance(&mut rt, 0.2);
    key(&mut rt, Key::Escape, Mods::default());
    assert!(matches!(
        replies(&mut rt).as_slice(),
        [(_, Command::Answer(_, None))]
    ));
    assert!(rt.state.live.as_ref().unwrap().app.sessions[0].working());
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep my next message"
    );
}

#[test]
fn long_question_forms_scroll_inside_the_dock_and_keep_actions_visible() {
    for theme in ["light", "dark"] {
        for size in [(800.0, 620.0), (1280.0, 1000.0)] {
            let mut rt = live_runtime_size(size);
            rt.update(|s, _| {
                s.inspector.set_open(true);
                let l = s.live.as_mut().unwrap();
                l.app.preferences.theme = theme.into();
                l.app.sessions[0].remote_id = Some("thread".into());
            });
            let mut request = prompt("codex", "thread", "long");
            request.fields = vec![
                json!({"key":"answer","label":"Choose an implementation","type":"string","required":true,"options":(0..24).map(|i| json!({"value":format!("option-{i}"),"label":format!("Option {i}: a detailed answer that should wrap in narrow windows"),"description":"Keep the whole answer readable while leaving space for the conversation."})).collect::<Vec<_>>()}),
            ];
            questions(&mut rt, vec![request]);
            for key in [
                "question-composer",
                "question-actions",
                "question-submit-long",
                "question-dismiss-long",
            ] {
                let e = element(&rt, key);
                assert!(
                    e.rect
                        .iter()
                        .zip(e.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{theme} {size:?}: {e:?}"
                );
            }
            let transcript = element(&rt, "live-transcript-0").rect;
            let dock = element(&rt, "question-composer").rect;
            assert!(transcript[3] > 100.0, "{theme} {size:?}: {transcript:?}");
            assert!(transcript[1] + transcript[3] <= dock[1] + 1.0);
            let footer = element(&rt, "question-actions").rect;
            let (x, y) = rt.find("question-fields-0-long").unwrap();
            rt.wheel(x, y, -10000.0);
            advance(&mut rt, 0.3);
            assert_eq!(rt.scroll_at_bottom("question-fields-0-long"), Some(true));
            assert!(
                element(&rt, "question-actions")
                    .rect
                    .iter()
                    .zip(footer)
                    .all(|(a, b)| (*a - b).abs() < 1.0)
            );
            let last = element(&rt, "question-long-answer-23");
            assert!(
                last.visible[3] > 20.0,
                "{theme} {size:?}: {last:?}; viewport {:?}",
                element(&rt, "question-fields-0-long")
            );
        }
    }
}

#[test]
fn a_provider_question_temporarily_covers_the_side_question_editor() {
    let mut rt = live_runtime();
    click(&mut rt, "live-session-1");
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.sessions[1].remote_id = Some("session".into());
    });
    fill(&mut rt, "live-draft-1", "/btw");
    key(&mut rt, Key::Enter, Mods::default());
    fill(&mut rt, "side-question-input", "My separate question");
    questions(&mut rt, vec![prompt("opencode", "session", "agent")]);
    assert!(rt.find("side-question").is_none());
    assert!(rt.find("question-composer").is_some());
    questions(&mut rt, vec![]);
    assert!(element(&rt, "side-question-input").focused);
    assert_eq!(
        rt.state.live.as_ref().unwrap().commands.side_editor.text,
        "My separate question"
    );
}

#[test]
fn a_question_takeover_dismisses_the_previous_editors_context_menu() {
    let mut rt = live_runtime();
    fill(&mut rt, "live-draft-0", "Keep this draft");
    secondary(&mut rt, "live-draft-0");
    rt.update(|s, _| s.live.as_mut().unwrap().app.sessions[0].remote_id = Some("thread".into()));
    questions(&mut rt, vec![prompt("codex", "thread", "question")]);
    assert!(!crate::context::is_open(&rt.state));
    assert!(element(&rt, "question-input-question-answer").focused);
    secondary(&mut rt, "question-input-question-answer");
    questions(&mut rt, vec![]);
    assert!(!crate::context::is_open(&rt.state));
    assert!(element(&rt, "live-draft-0").focused);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep this draft"
    );
}

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
    assert!(
        !rt.state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .iter()
            .any(|(_, command)| matches!(command, Command::Answer(..)))
    );
    click(&mut rt, "question-question-branch-1");
    click(&mut rt, "question-submit-question");
    assert!(
        matches!(rt.state.live.as_mut().unwrap().app.commands().as_slice(), [(_, Command::Answer(prompt, Some(answer)))] if prompt.session_id == "thread" && answer["branch"] == "feature")
    );
}
