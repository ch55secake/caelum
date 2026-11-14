use super::*;

#[test]
fn history_reconciles_overlaps_lagging_deltas_and_unicode_for_both_providers() {
    for provider in Provider::ALL {
        let mut conversation = Conversation::default();
        let (method, delta, history, complete) = match provider {
            Provider::Codex => (
                "item/agentMessage/delta",
                json!({"itemId":"answer","delta":"café"}),
                vec![
                    json!({"status":"inProgress", "items":[{"id":"answer","type":"agentMessage","text":"Hello café ☁️"}]}),
                ],
                (
                    "item/completed",
                    json!({"item":{"id":"answer","type":"agentMessage","text":"Hello café ☁️ done"}}),
                ),
            ),
            Provider::OpenCode => (
                "session.text.delta",
                json!({"assistantMessageID":"answer","delta":"café"}),
                vec![
                    json!({"id":"answer","type":"assistant","content":[{"type":"text","text":"Hello café ☁️"}]}),
                ],
                (
                    "session.text.ended",
                    json!({"assistantMessageID":"answer","text":"Hello café ☁️ done"}),
                ),
            ),
        };
        conversation.apply(provider, method, &delta);
        conversation.history(provider, &history, false);
        let mut next = delta.clone();
        next["delta"] = " ☁️ done".into();
        conversation.apply(provider, method, &next);
        assert_eq!(conversation.entries.len(), 1);
        assert_eq!(conversation.entries[0].text, "Hello café ☁️ done");
        conversation.apply(provider, complete.0, &complete.1);
        conversation.apply(provider, method, &delta);
        conversation.history(provider, &history, false);
        assert_eq!(
            conversation.entries[0].text, "Hello café ☁️ done",
            "late snapshots/deltas cannot truncate completed text"
        );
    }
}

#[test]
fn repeated_prompts_and_older_pages_do_not_duplicate_or_adopt_the_wrong_message() {
    let mut conversation = Conversation::default();
    conversation.optimistic("local-1".into(), "again".into());
    conversation.history(
        Provider::OpenCode,
        &[json!({"id":"old","type":"user","text":"again"})],
        true,
    );
    assert_eq!(conversation.entries.len(), 2);
    assert!(conversation.entries[1].optimistic);
    conversation.history(
        Provider::OpenCode,
        &[
            json!({"id":"old","type":"user","text":"again"}),
            json!({"id":"new","type":"user","text":"again"}),
        ],
        false,
    );
    assert_eq!(conversation.entries.len(), 2);
    assert!(conversation.entries.iter().all(|entry| !entry.optimistic));
    conversation.history(
        Provider::OpenCode,
        &[json!({"id":"old","type":"user","text":"again"})],
        true,
    );
    assert_eq!(conversation.entries[1].id, "opencode-new");
}

#[test]
fn an_unmatched_image_prompt_cannot_block_reconciliation_of_later_turns() {
    for provider in Provider::ALL {
        let image = Attachment::image("screenshot.png".into(), b"\x89PNG\r\n\x1a\n").unwrap();
        let mut conversation = Conversation::default();
        conversation.optimistic(
            "local-image".into(),
            super::super::composer::with_context("Describe this", std::slice::from_ref(&image)),
        );
        for turn in 0..3 {
            conversation.optimistic(format!("local-{turn}"), "Follow up".into());
            let id = format!("user-{turn}");
            match provider {
                Provider::Codex => conversation.apply(provider, "item/started", &json!({
                    "item":{"id":id,"type":"userMessage","content":[{"type":"text","text":"Follow up"}]},
                })),
                Provider::OpenCode => conversation.history(provider, &[
                    json!({"id":id,"type":"user","text":"Follow up"}),
                ], false),
            }
            assert_eq!(
                conversation.entries.len(),
                turn + 2,
                "{provider:?} turn {turn}"
            );
            assert_eq!(
                conversation.entries.iter().filter(|e| e.optimistic).count(),
                1
            );
            assert_eq!(conversation.entries[0].id, "local-image");
        }
    }
}

#[test]
fn one_refresh_adopts_all_pending_prompts_once_without_consuming_a_later_repeat() {
    let mut conversation = Conversation::default();
    for id in ["local-first", "local-second"] {
        conversation.optimistic(id.into(), "again".into());
    }
    let history = [
        json!({"id":"first","type":"user","text":"again"}),
        json!({"id":"second","type":"user","text":"again"}),
    ];
    conversation.history(Provider::OpenCode, &history, false);
    assert_eq!(conversation.entries.len(), 2);
    assert!(conversation.entries.iter().all(|entry| !entry.optimistic));
    conversation.optimistic("local-third".into(), "again".into());
    for _ in 0..2 {
        conversation.history(Provider::OpenCode, &history, false);
        assert_eq!(conversation.entries.len(), 3);
        assert!(
            conversation.entries[2].optimistic,
            "an old echo is not the new submission"
        );
    }
    conversation.history(
        Provider::OpenCode,
        &[json!({"id":"third","type":"user","text":"again"})],
        false,
    );
    assert_eq!(conversation.entries.len(), 3);
    assert!(conversation.entries.iter().all(|entry| !entry.optimistic));
}

#[test]
fn image_context_matching_preserves_older_messages_and_distinct_captures() {
    let image = Attachment::image("screenshot.png".into(), b"\x89PNG\r\n\x1a\n").unwrap();
    let other = Attachment::image("screenshot.png".into(), b"\x89PNG\r\n\x1a\nother").unwrap();
    let prompt =
        super::super::composer::with_context("Describe this", std::slice::from_ref(&image));
    let mut echo = image.clone();
    echo.id = "reconstructed-image".into();
    let echo = super::super::composer::with_context("Describe this", &[echo]);
    let old = json!({"id":"old","type":"user","text":echo});
    let mut conversation = Conversation::default();
    conversation.optimistic("local".into(), prompt);
    conversation.history(Provider::OpenCode, std::slice::from_ref(&old), true);
    conversation.history(Provider::OpenCode, &[old], false);
    assert_eq!(conversation.entries.len(), 2);
    assert!(conversation.entries[1].optimistic);

    conversation.history(Provider::OpenCode, &[json!({
        "id":"different","type":"user","text":super::super::composer::with_context("Describe this", &[other]),
    })], false);
    assert_eq!(conversation.entries.len(), 3);
    assert!(
        conversation.entries[1].optimistic,
        "same filename is not the same captured image"
    );
    for _ in 0..2 {
        conversation.history(
            Provider::OpenCode,
            &[json!({
                "id":"new","type":"user","text":echo,
            })],
            false,
        );
        assert_eq!(conversation.entries.len(), 3);
        assert!(conversation.entries.iter().all(|entry| !entry.optimistic));
        let entry = conversation
            .entries
            .iter()
            .find(|e| e.id == "opencode-new")
            .unwrap();
        let attachments = split_context(&entry.text).1;
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].content, image.content);
    }
}

#[test]
fn delayed_history_preserves_live_tool_output_snapshots() {
    let mut conversation = Conversation::default();
    let history = vec![json!({"id":"message","type":"assistant","content":[{
        "id":"tool", "type":"tool", "name":"bash", "state":{"status":"running","input":{"command":"test"},"content":[{"type":"text","text":"prefix"}]}
    }]})];
    conversation.history(Provider::OpenCode, &history, false);
    conversation.apply(
        Provider::OpenCode,
        "session.tool.updated",
        &json!({"id":"tool","output":"prefix and live output"}),
    );
    conversation.history(Provider::OpenCode, &history, false);
    assert_eq!(conversation.entries.len(), 1);
    assert_eq!(conversation.entries[0].output, "prefix and live output");
}

#[test]
fn delayed_history_cannot_overwrite_live_activity_or_another_session() {
    let mut workspace = Workspace::default();
    let first = started(&mut workspace, scope(Provider::Codex, "/a"), "one");
    workspace.refresh(first);
    let (resume, _) = workspace.commands().pop().unwrap();
    workspace.apply(Event::Completed(
        resume,
        Ok(Reply::Session(summary(scope(Provider::Codex, "/a"), "one"))),
    ));
    let (history, _) = workspace.commands().pop().unwrap();
    let second = workspace.new_session(scope(Provider::Codex, "/a"));
    workspace.apply(codex(
        "/a",
        "one",
        "item/agentMessage/delta",
        json!({"itemId":"answer","delta":"live"}),
    ));
    workspace.apply(Event::Completed(history, Ok(page(vec![json!({"status":"inProgress","items":[{"id":"answer","type":"agentMessage","text":"li"}]})], false))));
    assert!(workspace.sessions[first].working());
    assert_eq!(
        workspace.sessions[first]
            .conversation
            .entries
            .last()
            .unwrap()
            .text,
        "live"
    );
    assert_eq!(workspace.selected, Some(second));
    assert!(workspace.sessions[second].conversation.entries.is_empty());
}

#[test]
fn history_started_before_cancellation_cannot_resurrect_work() {
    let mut workspace = Workspace::default();
    let index = started(&mut workspace, scope(Provider::Codex, "/a"), "one");
    workspace.refresh(index);
    let resume = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        resume,
        Ok(Reply::Session(Summary {
            working: true,
            ..summary(scope(Provider::Codex, "/a"), "one")
        })),
    ));
    let history = workspace.commands().pop().unwrap().0;
    workspace.cancel(index);
    let cancel = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(cancel, Ok(Reply::Done)));
    workspace.apply(Event::Completed(history, Ok(page(Vec::new(), true))));
    assert!(!workspace.sessions[index].working());
    assert!(!workspace.sessions[index].stopping);
}

#[test]
fn initial_history_precedes_nonoverlapping_live_output_and_refresh_keeps_pagination() {
    let mut workspace = Workspace::default();
    let index = workspace.new_session(scope(Provider::Codex, "/a"));
    workspace.sessions[index].remote_id = Some("one".into());
    workspace.sessions[index].loaded = false;
    workspace.refresh(index);
    let resume = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        resume,
        Ok(Reply::Session(summary(scope(Provider::Codex, "/a"), "one"))),
    ));
    let history = workspace.commands().pop().unwrap().0;
    workspace.apply(codex(
        "/a",
        "one",
        "item/agentMessage/delta",
        json!({"itemId":"new", "delta":"New response"}),
    ));
    workspace.apply(Event::Completed(history, Ok(Reply::History(SessionHistoryPage {
        data: vec![json!({"status":"completed","items":[{"id":"old","type":"agentMessage","text":"Older response"}]})],
        next_cursor: Some("older".into()), is_working: Some(false), model: None, effort: None, agent: None,
    }))));
    assert_eq!(
        workspace.sessions[index].conversation.entries[0].id,
        "codex-old"
    );
    assert_eq!(
        workspace.sessions[index].conversation.entries[1].id,
        "codex-new"
    );
    assert!(workspace.sessions[index].working());
    workspace.older(index);
    let older = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(older, Ok(page(Vec::new(), false))));
    workspace.refresh(index);
    let resume = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        resume,
        Ok(Reply::Session(summary(scope(Provider::Codex, "/a"), "one"))),
    ));
    let refresh = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        refresh,
        Ok(Reply::History(SessionHistoryPage {
            data: Vec::new(),
            next_cursor: Some("older".into()),
            is_working: None,
            model: None,
            effort: None,
            agent: None,
        })),
    ));
    assert!(
        workspace.sessions[index].next_cursor.is_none(),
        "refresh must not reset the already-exhausted older cursor"
    );
}
