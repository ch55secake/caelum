use super::*;

#[test]
fn rendered_media_hides_reminders_but_preserves_literal_code_and_unicode() {
    let input="Keep café  \n<system-reminder>secret ![hidden](secret.png)</system-reminder>\n![visible](image.png)\n```md\n![literal](code.png)\n<system-reminder>literal</system-reminder>\n```\n`![inline](inline.png)`\n";
    let shown = presentation::visible_text(input);
    assert!(shown.starts_with("Keep café  \n"));
    assert!(!shown.contains("secret"));
    assert!(shown.contains("![literal](code.png)"));
    assert!(shown.contains("<system-reminder>literal</system-reminder>"));
    assert!(shown.contains("`![inline](inline.png)`"));
    let media = presentation::markdown_media(input);
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].source, "image.png");
    assert_eq!(
        presentation::visible_text("Answer<system-reminder>unfinished"),
        "Answer"
    );
}

#[test]
fn reminder_fragments_never_enter_presented_rows_or_copied_text() {
    let mut c = Conversation::default();
    for ch in "<system-reminder>secret ![private](private.png)</system-reminder>".chars() {
        c.apply(
            Provider::Codex,
            "item/agentMessage/delta",
            &json!({"itemId":"answer","delta":ch.to_string()}),
        );
        let entry = &c.entries[0];
        assert_eq!(presentation::entry_text(entry), "");
        assert!(!presentation::entry_visible(entry));
        assert!(presentation::markdown_media(&entry.text).is_empty());
    }
    c.apply(
        Provider::Codex,
        "item/agentMessage/delta",
        &json!({"itemId":"answer","delta":"Visible café"}),
    );
    assert_eq!(presentation::entry_text(&c.entries[0]), "Visible café");
    assert!(
        c.entries[0].text.contains("secret"),
        "presentation must retain provider records"
    );
    c.history(Provider::OpenCode, &[
        json!({"id":"synthetic","type":"synthetic","text":"<system-reminder>secret status</system-reminder>"}),
        json!({"id":"user","type":"user","text":"```xml\n<system-reminder>literal</system-reminder>\n```\n    <system-reminder>indented code</system-reminder>\n`<system-reminder>inline</system-reminder>`"}),
    ], false);
    let literal = c
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::User)
        .unwrap();
    assert!(presentation::entry_text(literal).contains("indented code"));
    assert!(presentation::entry_text(literal).contains("literal"));
    assert!(presentation::entry_text(literal).contains("inline"));
    assert!(c
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Status)
        .all(|e| !presentation::entry_visible(e)));
}

#[test]
fn image_prompt_history_adopts_the_optimistic_user_message_once() {
    let image = Attachment::image("screenshot.png".into(), b"\x89PNG\r\n\x1a\n").unwrap();
    for provider in [Provider::Codex, Provider::OpenCode] {
        let prompt =
            super::super::composer::with_context("Describe this", std::slice::from_ref(&image));
        let mut conversation = Conversation::default();
        conversation.optimistic("local-prompt".into(), prompt);
        let message = match provider {
            Provider::Codex => json!({
                "status":"completed",
                "items":[{"id":"user-message", "type":"userMessage", "content":[
                    {"type":"text", "text":"Describe this"},
                    {"type":"image", "url":image.content.clone(), "name":image.path.clone()}
                ]}]
            }),
            Provider::OpenCode => json!({
                "id":"user-message", "type":"user", "text":"Describe this",
                "files":[{"mime":"image/png", "uri":image.content.clone(), "name":image.path.clone()}]
            }),
        };
        if provider == Provider::Codex {
            conversation.apply(
                provider,
                "item/started",
                &json!({"item":message["items"][0].clone()}),
            );
            assert_eq!(conversation.entries.len(), 1, "live event {provider:?}");
        }
        conversation.history(provider, &[message], false);

        assert_eq!(conversation.entries.len(), 1, "{provider:?}");
        let entry = &conversation.entries[0];
        assert_eq!(entry.kind, EntryKind::User);
        assert_eq!(presentation::entry_text(entry), "Describe this");
        assert_eq!(entry.media.len(), 1, "{provider:?}");
        assert_eq!(entry.media[0].source, image.content);
    }
}

fn refresh_transcript(app: &mut Workspace, data: &[serde_json::Value]) {
    app.refresh(0);
    let (request, command) = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Resume(_)))
        .unwrap();
    let Command::Resume(key) = command else {
        unreachable!()
    };
    app.apply(Event::Completed(
        request,
        Ok(Reply::Session(Summary {
            key,
            title: "Image conversation".into(),
            working: false,
            updated_at: None,
            options: Options::default(),
        })),
    ));
    for (request, command) in app.commands() {
        let reply = match command {
            Command::History(..) => Reply::History(caelum_protocol::SessionHistoryPage {
                data: data.to_vec(),
                next_cursor: None,
                is_working: Some(false),
                model: None,
                effort: None,
                agent: None,
            }),
            Command::Inbox(_) => Reply::Inbox(Vec::new()),
            Command::Shells(_) => Reply::Shells(Vec::new()),
            _ => continue,
        };
        app.apply(Event::Completed(request, Ok(reply)));
    }
}

#[test]
fn image_inbox_delivery_reconciles_the_real_submission_path_and_later_turns() {
    for (text, count) in [("Describe this café", 1), ("", 1), ("Compare these", 2)] {
        let mut app = workspace();
        let captured = (0..count)
            .map(|index| {
                let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
                bytes.push(index as u8);
                let mut image = Attachment::image("screenshot.png".into(), &bytes).unwrap();
                image.id = format!("captured-image-{index}");
                image
            })
            .collect::<Vec<_>>();
        let context = Attachment::new("src/main.rs".into(), "fn main() {}".into(), None, false);
        app.sessions[0].attachments.extend(captured.clone());
        if count > 1 {
            app.sessions[0].attachments.push(context.clone());
        }
        let mut history = Vec::new();
        for (turn, text) in [text, "Follow up", "Follow up"].into_iter().enumerate() {
            assert!(app.send(0, text.into()));
            let (request, command) = app
                .commands()
                .into_iter()
                .find(|(_, c)| matches!(c, Command::Send(..)))
                .unwrap();
            let Command::Send(_, prompt, _) = command else {
                unreachable!()
            };
            let (provider_text, images) = crate::application::provider_prompt(&prompt);
            // OpenCode normalizes submitted data URIs into these inbox/history files.
            let files = images
                .iter()
                .map(|image| {
                    json!({
                        "mime":"image/png", "data":image.content.split_once(',').unwrap().1,
                        "source":{"type":"inline"}, "name":image.path,
                    })
                })
                .collect::<Vec<_>>();
            let id = format!("msg_user_{turn}");
            let at = app.sessions[0]
                .conversation
                .entries
                .last()
                .unwrap()
                .started_at
                .unwrap()
                + 1;
            app.apply(opencode(
                "session.inbox.enqueued",
                json!({
                    "sessionID":"session-0", "inboxID":id,
                    "item":{"type":"user", "payload":{"text":provider_text,"files":files}},
                }),
            ));
            app.apply(opencode(
                "session.inbox.delivered",
                json!({
                    "sessionID":"session-0", "inboxID":id,
                }),
            ));
            app.apply(Event::Completed(request, Ok(Reply::Done)));
            let conversation = &app.sessions[0].conversation;
            assert_eq!(
                conversation.entries.len(),
                turn * 2 + 1,
                "delivery of turn {turn}"
            );
            assert!(conversation.entries.iter().all(|entry| !entry.optimistic));
            let entry = conversation.entries.last().unwrap();
            assert_eq!(entry.id, format!("opencode-{id}"));
            assert_eq!(presentation::entry_text(entry), text);
            let attached = split_context(&entry.text).1;
            assert_eq!(
                attached
                    .iter()
                    .filter(|a| a.kind == AttachmentKind::Image)
                    .count()
                    + entry.media.len(),
                images.len()
            );
            if turn == 0 {
                for image in &captured {
                    assert_eq!(
                        attached
                            .iter()
                            .filter(|a| a.content == image.content)
                            .count(),
                        1
                    );
                }
                if count > 1 {
                    assert!(attached.contains(&context));
                }
            }

            let answer = format!("msg_answer_{turn}");
            app.apply(opencode(
                "session.text.delta",
                json!({
                    "sessionID":"session-0", "assistantMessageID":answer, "delta":"Answer",
                }),
            ));
            app.apply(opencode(
                "session.text.ended",
                json!({
                    "sessionID":"session-0", "assistantMessageID":answer, "text":"Answer",
                }),
            ));
            app.apply(opencode(
                "session.execution.succeeded",
                json!({"sessionID":"session-0"}),
            ));
            history.extend([
                json!({"id":id,"type":"user","text":provider_text,"files":files,"time":{"created":at}}),
                json!({"id":answer,"type":"assistant","content":[{"type":"text","text":"Answer"}],"finish":"stop"}),
            ]);
            for _ in 0..2 {
                refresh_transcript(&mut app, &history);
                let entries = &app.sessions[0].conversation.entries;
                assert_eq!(entries.len(), (turn + 1) * 2, "refresh after turn {turn}");
                assert!(entries.iter().all(|entry| !entry.optimistic));
                assert_eq!(entries[0].media.len(), count);
                let attached = split_context(&entries[0].text).1;
                assert!(attached.iter().all(|a| a.kind != AttachmentKind::Image));
                for (media, image) in entries[0].media.iter().zip(&captured) {
                    assert_eq!(media.source, image.content);
                }
                if count > 1 {
                    assert_eq!(attached, std::slice::from_ref(&context));
                }
            }
        }
    }
}
