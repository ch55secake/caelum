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
