use super::*;
use caelum_core::application::{Event, Provider, Scope};
use caelum_protocol::{CodexEvent, OpenCodeEvent};
use serde_json::json;

fn state(provider: Provider) -> Workspace {
    let mut state = Workspace::live(None);
    let l = state.live.as_mut().unwrap();
    l.fixture = true;
    l.app.projects.push(caelum_protocol::ProjectInfo {
        name: "Project".into(),
        path: "/project".into(),
    });
    for id in ["foreground", "background"] {
        let index = l.app.new_session(Scope {
            project: "/project".into(),
            provider,
        });
        let session = &mut l.app.sessions[index];
        session.remote_id = Some(id.into());
        session.loaded = true;
        session.last_activity = Some(1);
    }
    l.app.select(0);
    l.sync_editors();
    state
}

fn event(provider: Provider, method: &str, data: serde_json::Value) -> Event {
    match provider {
        Provider::Codex => Event::Codex(CodexEvent {
            project_path: "/project".into(),
            method: method.into(),
            params: data,
            request_id: None,
        }),
        Provider::OpenCode => Event::OpenCode(OpenCodeEvent {
            project_path: "/project".into(),
            event_type: method.into(),
            data,
            id: None,
            created: Some(1000),
            location: None,
        }),
    }
}

fn delta(provider: Provider, id: &str, text: &str) -> Event {
    event(
        provider,
        match provider {
            Provider::Codex => "item/agentMessage/delta",
            Provider::OpenCode => "session.text.delta",
        },
        json!({"threadId":id,"sessionID":id,"itemId":"answer","assistantMessageID":"answer","delta":text,"_created":1000}),
    )
}

#[test]
fn background_streams_land_immediately_without_invalidating_unchanged_rows() {
    for (provider, methods) in [
        (
            Provider::Codex,
            &[
                "item/agentMessage/delta",
                "item/reasoning/textDelta",
                "item/reasoning/summaryTextDelta",
            ][..],
        ),
        (
            Provider::OpenCode,
            &["session.text.delta", "session.reasoning.delta"][..],
        ),
    ] {
        for method in methods {
            let mut state = state(provider);
            let l = state.live.as_mut().unwrap();
            let update = || {
                event(
                    provider,
                    method,
                    json!({"threadId":"background","sessionID":"background","itemId":"answer","assistantMessageID":"answer","delta":" café","_created":1000}),
                )
            };
            assert!(l.apply_events([update()]), "first activity: {method}");
            assert!(l.app.sessions[1].working());
            assert_eq!(l.app.sessions[1].last_activity, Some(1000));
            assert!(
                !l.apply_events([update(), update()]),
                "stable row: {method}"
            );
            assert_eq!(
                l.app.sessions[1].conversation.entries[0].text,
                " café café café"
            );
            assert!(l.app.sessions[0].conversation.entries.is_empty());
            assert!(!l.apply_events([]));
            l.app.select(1);
            assert!(l.apply_events([update()]), "selected transcript: {method}");
            assert_eq!(
                l.app.sessions[1].conversation.entries[0].text,
                " café café café café"
            );
        }
    }
}

#[test]
fn background_unread_retry_and_title_changes_invalidate_the_view() {
    for provider in Provider::ALL {
        let mut state = state(provider);
        let l = state.live.as_mut().unwrap();
        // Busy can already be set by turn-start while the first output is unread.
        l.app.sessions[1].conversation.working = true;
        assert!(l.apply_events([delta(provider, "background", "first")]));
        assert!(l.app.sessions[1].unread);
        assert!(!l.apply_events([delta(provider, "background", " second")]));
        l.app.sessions[1].conversation.retrying = true;
        assert!(l.apply_events([delta(provider, "background", " resumed")]));
        assert!(!l.app.sessions[1].conversation.retrying);
        let mut renamed = delta(provider, "background", " renamed");
        match &mut renamed {
            Event::Codex(e) => e.params["thread"] = json!({"id":"background","name":"New title"}),
            Event::OpenCode(e) => {
                e.data["session"] = json!({"id":"background","title":"New title"})
            }
            _ => unreachable!(),
        }
        assert!(l.apply_events([renamed]));
        assert_eq!(l.app.sessions[1].title, "New title");
    }
}

#[test]
fn redraw_suppression_keeps_project_provider_and_unique_session_ownership() {
    for provider in Provider::ALL {
        let mut state = state(provider);
        let l = state.live.as_mut().unwrap();
        l.apply_events([delta(provider, "background", "first")]);
        // Reused remote ids in another provider or checkout are distinct owners.
        l.app.sessions[0].remote_id = Some("background".into());
        l.app.sessions[0].scope.provider = match provider {
            Provider::Codex => Provider::OpenCode,
            Provider::OpenCode => Provider::Codex,
        };
        assert!(!l.apply_events([delta(provider, "background", " provider")]));
        l.app.sessions[0].scope.provider = provider;
        l.app.sessions[0].scope.project = "/other".into();
        assert!(!l.apply_events([delta(provider, "background", " checkout")]));
        assert!(l.app.sessions[0].conversation.entries.is_empty());
        assert!(l.apply_events([delta(provider, "missing", "unknown owner")]));
        let mut nested = delta(provider, "background", " nested");
        match &mut nested {
            Event::Codex(e) => {
                e.params.as_object_mut().unwrap().remove("threadId");
                e.params["thread"] = json!({"id":"background"});
            }
            Event::OpenCode(e) => {
                e.data.as_object_mut().unwrap().remove("sessionID");
                e.data["session"] = json!({"id":"background"});
            }
            _ => unreachable!(),
        }
        assert!(!l.apply_events([nested]));
        l.app.sessions[0].scope.project = "/project".into();
        assert!(l.apply_events([delta(provider, "background", " both")]));
        assert_eq!(l.app.sessions[0].conversation.entries[0].text, " both");
        assert_eq!(
            l.app.sessions[1].conversation.entries[0].text,
            "first provider checkout nested both"
        );
    }
}

#[test]
fn mixed_batches_keep_completion_errors_and_permission_requests_immediate() {
    let provider = Provider::Codex;
    let mut state = state(provider);
    let l = state.live.as_mut().unwrap();
    l.apply_events([delta(provider, "background", "first")]);
    let approval = caelum_protocol::ApprovalPrompt {
        provider: "codex".into(),
        project_path: "/project".into(),
        session_id: Some("background".into()),
        request_id: "approval".into(),
        summary: "Run command".into(),
        description: "cargo check".into(),
    };
    let question = caelum_protocol::QuestionPrompt {
        provider: "codex".into(),
        project_path: "/project".into(),
        session_id: "background".into(),
        request_id: "question".into(),
        title: "Choose a branch".into(),
        fields: vec![],
    };
    assert!(l.apply_events([
        delta(provider, "background", " second"),
        Event::Approvals(vec![approval.clone()]),
        Event::Questions(vec![question.clone()]),
        delta(provider, "background", " third"),
    ]));
    assert_eq!(l.app.approvals, vec![approval]);
    assert_eq!(l.app.questions, vec![question]);
    assert_eq!(
        l.app.sessions[1].conversation.entries[0].text,
        "first second third"
    );
    assert!(l.apply_events([
        delta(provider, "background", " final"),
        event(
            provider,
            "turn/completed",
            json!({"threadId":"background","turn":{"id":"turn"},"_created":2000})
        ),
    ]));
    assert!(!l.app.sessions[1].working());
    assert!(l.app.sessions[1].conversation.entries[0].complete);
    assert_eq!(
        l.app.sessions[1].conversation.entries[0].text,
        "first second third final"
    );
    assert!(l.apply_events([event(
        provider,
        "error",
        json!({"threadId":"background","message":"Connection failed"})
    )]));
    assert_eq!(
        l.app.sessions[1].error.as_deref(),
        Some("Connection failed")
    );
    assert!(l.apply_events([event(
        provider,
        "future/event",
        json!({"threadId":"background"})
    )]));
}

#[test]
fn bundled_history_and_selected_updates_invalidate_mixed_streams() {
    let provider = Provider::OpenCode;
    let mut state = state(provider);
    let l = state.live.as_mut().unwrap();
    l.apply_events([delta(provider, "background", "first")]);
    let mut history = delta(provider, "background", " second");
    let Event::OpenCode(e) = &mut history else {
        unreachable!()
    };
    e.data["message"] = json!({"id":"history","type":"user","text":"Restored prompt"});
    assert!(l.apply_events([history]));
    assert!(
        l.app.sessions[1]
            .conversation
            .entries
            .iter()
            .any(|e| e.text == "Restored prompt")
    );
    assert!(l.apply_events([
        delta(provider, "background", " third"),
        delta(provider, "foreground", "visible"),
        delta(provider, "background", " fourth"),
    ]));
    assert_eq!(l.app.sessions[0].conversation.entries[0].text, "visible");
    assert!(
        l.app.sessions[1]
            .conversation
            .entries
            .iter()
            .any(|e| e.text == "first second third fourth")
    );
}

#[test]
fn background_streaming_preserves_idle_commits_typing_undo_and_navigation() {
    let provider = Provider::Codex;
    let app = spec();
    let mut state = state(provider);
    state
        .live
        .as_mut()
        .unwrap()
        .apply_events([delta(provider, "background", "Background")]);
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    advance(&mut rt, 0.6);
    fill(&mut rt, "live-draft-0", "Draft");
    advance(&mut rt, 0.5);
    let commits = rt.stats.commits;
    for _ in 0..10 {
        rt.update_if(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply_events([delta(provider, "background", " text")])
        });
        advance(&mut rt, 0.05);
    }
    assert_eq!(rt.stats.commits, commits);
    assert!(element(&rt, "live-draft-0").focused);
    key(&mut rt, Key::Text(" café".into()), Mods::default());
    rt.update_if(|s, _| {
        s.live
            .as_mut()
            .unwrap()
            .apply_events([delta(provider, "background", " latest")])
    });
    advance(&mut rt, 0.1);
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, "Draft café");
    key(
        &mut rt,
        Key::Command('z'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, "Draft");
    click(&mut rt, "live-session-1");
    advance(&mut rt, 0.5);
    let expected = format!("Background{} latest", " text".repeat(10));
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[1]
            .conversation
            .entries[0]
            .text,
        expected
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| !e.exiting && e.text.as_deref() == Some(&expected))
    );
    click(&mut rt, "live-session-0");
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, "Draft");
    click(&mut rt, "live-draft-0");
    key(&mut rt, Key::End, Mods::default());
    key(&mut rt, Key::Text(" again".into()), Mods::default());
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Draft again"
    );
}

fn deltas(foreground: bool) -> Vec<Event> {
    (usize::from(!foreground)..6)
        .map(|i| Event::Codex(CodexEvent {
            project_path: "/evaluation".into(),
            method: "item/agentMessage/delta".into(),
            params: json!({"threadId":format!("thread-{i}"),"itemId":"profile-live","delta":" delta","_created":1000}),
            request_id: None,
        }))
        .collect()
}

#[test]
#[ignore = "opt-in release background streaming CPU profile; run alone"]
fn background_redraw_profile() {
    use std::time::Instant;
    for (label, foreground, typing) in [
        ("background", false, false),
        ("background-typing", false, true),
        ("foreground", true, false),
    ] {
        let app = spec();
        let mut state = crate::evaluation::workspace();
        let l = state.live.as_mut().unwrap();
        l.evaluation = false;
        l.fixture = true;
        l.sync_editors();
        l.drafts[0].focused = true;
        l.apply_events(deltas(foreground));
        let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
        rt.poll = app.poll;
        rt.on_key = app.on_key;
        advance(&mut rt, 0.6);
        let mut frames = Vec::new();
        let mut typed = Vec::new();
        let commits = rt.stats.commits;
        for i in 0..360 {
            let events = deltas(foreground);
            let typing = typing && i % 6 == 0;
            let start = Instant::now();
            rt.update_if(|s, _| s.live.as_mut().unwrap().apply_events(events));
            if typing {
                rt.key(KeyEvent {
                    key: Key::Text("x".into()),
                    mods: Mods::default(),
                });
            }
            rt.frame(1.0 / 60.0);
            if let Some(focused) = crate::live::presented_focus(&rt) {
                rt.update(|s, _| s.live.as_mut().unwrap().focus_key = focused);
            }
            let elapsed = start.elapsed().as_secs_f64() * 1_000.0;
            frames.push(elapsed);
            if typing {
                typed.push(elapsed);
            }
        }
        let stats = |mut values: Vec<f64>| {
            if values.is_empty() {
                return serde_json::Value::Null;
            }
            values.sort_by(f64::total_cmp);
            json!({"p50_ms":values[values.len()/2],"p95_ms":values[values.len()*95/100],"max_ms":values.last().unwrap()})
        };
        eprintln!(
            "{}",
            json!({"workload":label,"frames":stats(frames),"typing_frames":stats(typed),"commits":rt.stats.commits-commits})
        );
    }
}
