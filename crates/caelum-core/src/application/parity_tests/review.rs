use super::*;

#[test]
fn review_summaries_distinguish_unknown_checks_and_unmergeable_states() {
    use crate::application::presentation::{CheckSummary, MergeReadiness, Outcome};
    let checks: Vec<caelum_protocol::GitHubStatusCheck> = serde_json::from_value(json!([
        {"__typename":"StatusContext", "context":"CI", "state":"SUCCESS"},
        {"__typename":"CheckRun", "name":"Optional", "status":"COMPLETED", "conclusion":"SKIPPED"}
    ]))
    .unwrap();
    assert_eq!(CheckSummary::new(None).outcome, Outcome::Running);
    assert_eq!(CheckSummary::new(Some(&[])).outcome, Outcome::Neutral);
    assert_eq!(
        CheckSummary::new(Some(&checks[..1])).outcome,
        Outcome::Success
    );
    assert_ne!(CheckSummary::new(Some(&checks)).outcome, Outcome::Success);
    let mut details: caelum_protocol::GitHubPullRequestDetails = serde_json::from_value(json!({"number":1,"title":"Review","url":"https://github.com/example/repo/pull/1","isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"head","mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":1,"deletions":0,"changedFiles":1,"reviewDecision":null,"statusCheckRollup":[]})).unwrap();
    assert!(MergeReadiness::new(&details).can_merge);
    for state in ["BLOCKED", "DIRTY", "UNKNOWN", "FUTURE_STATE"] {
        details.merge_state_status = state.into();
        assert!(!MergeReadiness::new(&details).can_merge, "{state}");
    }
    details.merge_state_status = "BEHIND".into();
    assert!(MergeReadiness::new(&details).behind);
    details.state = "MERGED".into();
    assert!(!MergeReadiness::new(&details).behind);
    assert!(!MergeReadiness::new(&details).can_merge);
    details.state = "OPEN".into();
    details.merge_state_status = "CLEAN".into();
    details.mergeable = "UNKNOWN".into();
    assert!(!MergeReadiness::new(&details).can_merge);
    details.mergeable = "MERGEABLE".into();
    details.pull_request.is_draft = true;
    assert!(!MergeReadiness::new(&details).can_merge);
}

#[test]
fn addressing_review_feedback_submits_to_the_target_and_preserves_its_draft() {
    let mut app = workspace();
    let url = "https://github.com/example/project/pull/1";
    let r = app
        .services
        .reviews
        .entry(("/project".into(), url.into()))
        .or_default();
    r.details = Some(serde_json::from_value(json!({"number":1,"title":"Review","url":url,"isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"abc","mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":1,"deletions":0,"changedFiles":1,"reviewDecision":null,"statusCheckRollup":[]})).unwrap());
    r.discussion.push(serde_json::from_value(json!({"id":"comment","body":"Preserve the newer draft.","url":url,"author":null,"createdAt":"2026-11-07","state":null,"diffHunk":null})).unwrap());
    r.selected_comments.insert("comment".into());
    app.sessions[1].draft = "Independent draft".into();
    app.sessions[1].attachments.push(Attachment::new(
        "draft.rs".into(),
        "not feedback".into(),
        None,
        false,
    ));
    assert!(!app.address_review(1, "/other", url));
    assert!(app.address_review(1, "/project", url));
    let requests = app.commands();
    assert!(requests.iter().any(|(_, c)| matches!(c, Command::Send(key, prompt, _) if key.id == "session-1" && prompt.contains("Preserve the newer draft.") && !prompt.contains("not feedback"))));
    assert_eq!(app.selected, Some(0));
    assert_eq!(app.sessions[1].draft, "Independent draft");
    assert_eq!(app.sessions[1].attachments.len(), 1);
    assert!(!app.address_review(1, "/project", url));
}
