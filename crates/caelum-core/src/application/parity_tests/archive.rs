use super::*;

#[test]
fn archive_notice_expires_only_after_unengaged_time_and_recovers_after_undo_failure() {
    let mut app = workspace();
    app.archive(0, true);
    let request = app.commands().pop().unwrap().0;
    assert_eq!(app.last_archived, None, "notice waits for provider success");
    app.apply(Event::Completed(request, Ok(Reply::Done)));
    let now = presentation::now();
    app.tick_archive_notice(now + 4_000, false);
    app.tick_archive_notice(now + 24_000, true);
    assert_eq!(app.last_archived, Some(0));
    app.undo_archive();
    let undo = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Archive(_, false)))
        .unwrap()
        .0;
    app.tick_archive_notice(now + 60_000, false);
    assert_eq!(
        app.last_archived,
        Some(0),
        "restoration keeps its notice alive"
    );
    app.apply(Event::Completed(undo, Err("Offline, try again".into())));
    let failed = presentation::now();
    app.tick_archive_notice(failed + 7_000, false);
    assert_eq!(app.last_archived, Some(0));
    assert!(app.tick_archive_notice(failed + 8_001, false));
    assert_eq!(app.last_archived, None);
    assert!(
        app.sessions[0].archived,
        "expiration does not restore the session"
    );
}

#[test]
fn late_archive_and_restore_callbacks_cannot_replace_a_newer_notice() {
    let mut app = workspace();
    app.archive(0, true);
    let first = app.commands().pop().unwrap().0;
    app.archive(1, true);
    let second = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(second, Ok(Reply::Done)));
    app.apply(Event::Completed(first, Ok(Reply::Done)));
    assert_eq!(app.last_archived, Some(1));
    app.undo_archive();
    let undo = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Archive(_, false)))
        .unwrap()
        .0;
    let draft = app.selected.unwrap();
    app.archive(draft, true);
    assert_eq!(app.last_archived, Some(draft));
    app.apply(Event::Completed(undo, Ok(Reply::Done)));
    assert_eq!(app.last_archived, Some(draft));
    app.dismiss_archive(1);
    assert_eq!(app.last_archived, Some(draft));
}
