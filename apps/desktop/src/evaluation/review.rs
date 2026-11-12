//! Offline pull-request, merge and feedback scenes.
use crate::Workspace;
use serde_json::json;

pub(super) fn scene(state: &mut Workspace, name: &str) {
    if matches!(name, "overview-long" | "overview-stale") {
        scene(state, "overview-blocked");
        let l = state.live.as_mut().unwrap();
        let owner = l.panels.review.as_ref().unwrap();
        let review = l.app.services.reviews.get_mut(owner).unwrap();
        if name == "overview-stale" {
            review.error = Some("GitHub is unavailable. Showing the last loaded review.".into());
        } else {
            let details = review.details.as_mut().unwrap();
            details.pull_request.title = "Keep navigation, drafts and review context consistent across multiple projects and linked worktrees".into();
            details.head_ref_name =
                "feature/preserve-session-context-during-background-review-refresh".into();
            details.pull_request.base_ref_name =
                "release/native-workspace-and-provider-parity".into();
            details.body = (1..=12)
                .map(|i| format!("### Review area {i}\n\nPreserve the selected session, its draft and reading position while background changes arrive. Keep long branch names readable and make failed checks easy to find before reviewing the description.\n\n- Keep each response with its requesting checkout.\n- Restore the selected file after refreshing a review.\n\n"))
                .collect();
        }
        l.app.services.revision += 1;
        return;
    }
    let l = state.live.as_mut().unwrap();
    let project = "/projects/caelum";
    let url = "https://github.com/ch55secake/caelum/pull/95";
    let review = l
        .app
        .services
        .reviews
        .entry((project.into(), url.into()))
        .or_default();
    review.details=Some(serde_json::from_value(json!({"number":95,"title":"Restore native workspace parity","url":url,"isDraft":false,"body":"Native composer, continuous review and durable session workflows.","state":"OPEN","author":{"login":"ch55secake"},"baseRefName":"feat/rune-design-parity","headRefName":"feat/rune-native-cutover","headRefOid":"a".repeat(40),"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":34,"deletions":8,"changedFiles":3,"reviewDecision":"APPROVED","statusCheckRollup":[{"__typename":"CheckRun","name":"Native interactions","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"https://github.com/ch55secake/caelum/actions","startedAt":"2026-11-07T10:00:00Z","completedAt":"2026-11-07T10:02:43Z"},{"__typename":"StatusContext","context":"Provider adapters","state":"SUCCESS","targetUrl":"https://github.com/ch55secake/caelum/actions"}]})).unwrap());
    review.files=vec![
        caelum_protocol::GitHubPullRequestFile {path:"src/workspace.rs".into(),status:"modified".into(),previous_path:None,additions:3,deletions:1,patch:Some("@@ -1,4 +1,6 @@\n pub fn restore(session: &mut Session) {\n-    session.reset();\n+    session.restore_draft();\n+    session.restore_reading_position();\n+    session.focus();\n }\n ".into())},
        caelum_protocol::GitHubPullRequestFile {path:"src/composer.rs".into(),status:"renamed".into(),previous_path:Some("src/input.rs".into()),additions:1,deletions:1,patch:Some("@@ -1 +1 @@\n-const HEIGHT: f32 = 56.0;\n+const MAX_HEIGHT: f32 = 180.0;".into())},
        caelum_protocol::GitHubPullRequestFile {path:"assets/icon.png".into(),status:"modified".into(),previous_path:None,additions:0,deletions:0,patch:None}
    ];
    review.files_loaded = true;
    review.threads=(0..2).map(|n|serde_json::from_value(json!({"id":format!("thread-{n}"),"path":"src/workspace.rs","line":n+2,"startLine":null,"originalLine":n+2,"diffSide":"RIGHT","isResolved":n==1,"isOutdated":false,"comments":[{"id":format!("comment-{n}"),"body":if n==0 {"Keep newer navigation intact while a restore is pending."} else {"Reading position now survives refresh. Looks good."},"url":url,"author":{"login":"reviewer"},"createdAt":"2026-11-07T10:00:00Z","state":null,"diffHunk":null}],"nextCursor":null})).unwrap()).collect();
    for n in 0..9 {
        review.files.push(caelum_protocol::GitHubPullRequestFile {
            path: format!("crates/core/src/session_{n}.rs"),
            status: "modified".into(),
            previous_path: None,
            additions: 24,
            deletions: 24,
            patch: Some(format!(
                "@@ -1,24 +1,24 @@\n{}",
                (0..24)
                    .map(|i| format!(
                        "-    restore_slot({i});\n+    restore_owned_slot(session, {i});\n"
                    ))
                    .collect::<String>()
            )),
        });
    }
    review.details.as_mut().unwrap().changed_files = review.files.len() as u64;
    let comments = [
        "The two-tab review flow is much easier to navigate. Please keep inline feedback visible beside each changed line.",
        "I checked the compact layout. The destination session should stay visible when selecting feedback.",
        "The default patch previews are helpful. Collapsing a patch should survive the next output event.",
        "Keyboard navigation should return to the model search after moving through favourites.",
    ];
    review.discussion = comments.iter().enumerate().map(|(n, body)| serde_json::from_value(json!({"id":format!("discussion-{n}"),"body":body,"url":url,"author":{"login":if n%2==0 {"reviewer"} else {"maintainer"}},"createdAt":"2026-11-07T10:00:00Z","state":null,"diffHunk":null})).unwrap()).collect();
    review.selected_comments.insert("comment-0".into());
    if name.starts_with("overview") {
        review.selected_comments.clear();
        let details = review.details.as_mut().unwrap();
        details.body = "## A calmer native workspace\n\nThis follow-up brings the review and conversation surfaces back together, while keeping each session's **draft, attachments and reading position** independent.\n\n### What changed\n\n- A focused worktree form with a searchable starting branch\n- Compact background commands with a full output reader\n- Content-sized user messages and temporary archive Undo\n\n### Review notes\n\nUse the files tab to review the shared Rust changes, then select feedback to send to the current session.".into();
        if name == "overview-empty" {
            details.body.clear();
            details.status_check_rollup = Some(Vec::new());
            review.discussion.clear();
            review.reviews.clear();
            review.threads.clear();
        }
        if name == "overview-blocked" {
            details.merge_state_status = "BLOCKED".into();
            details.review_decision = Some("CHANGES_REQUESTED".into());
            details.status_check_rollup = Some(serde_json::from_value(json!([
                {"__typename":"CheckRun","name":"Build native desktop / aarch64-apple-darwin","status":"COMPLETED","conclusion":"FAILURE","detailsUrl":url,"startedAt":"2026-11-07T10:00:00Z","completedAt":"2026-11-07T10:02:43Z"},
                {"__typename":"CheckRun","name":"Core regression tests","status":"IN_PROGRESS","conclusion":null,"detailsUrl":url},
                {"__typename":"CheckRun","name":"Formatting and static analysis","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":url}
            ])).unwrap());
        }
    }
    if name.starts_with("merge-") {
        review.selected_comments.clear();
        let details = review.details.as_mut().unwrap();
        details.body = "A focused native workspace refinement, ready for review.".into();
        match name {
            "merge-blocked" | "merge-auto-enabled" | "merge-auto-error" => {
                details.merge_state_status = "BLOCKED".into();
                details.review_decision = Some("REVIEW_REQUIRED".into());
                if name == "merge-auto-enabled" {
                    details.auto_merge_request = Some(caelum_protocol::GitHubAutoMergeRequest {
                        merge_method: caelum_protocol::GitHubMergeMethod::Squash,
                    });
                }
            }
            "merge-pending" => {
                details.merge_state_status = "UNKNOWN".into();
                details.mergeable = "UNKNOWN".into();
                details.status_check_rollup = None;
            }
            "merge-conflicts" => {
                details.merge_state_status = "DIRTY".into();
                details.mergeable = "CONFLICTING".into();
            }
            "merge-behind" => details.merge_state_status = "BEHIND".into(),
            "merge-draft" => details.pull_request.is_draft = true,
            "merge-merged" => details.state = "MERGED".into(),
            "merge-closed" => details.state = "CLOSED".into(),
            _ => {}
        }
    }
    for kind in ["Discussion", "Reviews", "Threads"] {
        review.cursors.insert(kind.into(), None);
    }
    l.app.sessions[0].remote_id = Some("fixture".into());
    l.app
        .services
        .checkouts
        .get_mut(project)
        .unwrap()
        .pull_requests = vec![review.details.as_ref().unwrap().pull_request.clone()];
    l.app.services.revision += 1;
    if name == "mentions" {
        l.sync_editors();
        l.drafts[0].replace_text("Review #9 before sending this draft");
        l.drafts[0].set_caret(9);
        l.drafts[0].focused = true;
    } else {
        crate::panels::open_review(state, project.into(), url.into());
        state.live.as_mut().unwrap().panels.review_tab = if name == "checks"
            || name == "discussion"
            || name.starts_with("overview")
            || name.starts_with("merge-")
        {
            0
        } else {
            1
        };
        if name == "review-loading" {
            let l = state.live.as_mut().unwrap();
            let r = l
                .app
                .services
                .reviews
                .get_mut(&(project.into(), url.into()))
                .unwrap();
            r.details = None;
            r.files.clear();
            r.files_loaded = false;
            l.app.services.revision += 1;
        }
        if matches!(name, "review-error" | "merge-auto-error") {
            let l = state.live.as_mut().unwrap();
            let r = l
                .app
                .services
                .reviews
                .get_mut(&(project.into(), url.into()))
                .unwrap();
            if name == "review-error" {
                r.details = None;
                r.error =
                    Some("GitHub is unavailable. Check your connection and try again.".into());
            } else {
                r.error = Some(
                    "GitHub request failed: Auto-merge is not enabled for this repository.".into(),
                );
            }
            l.app.services.revision += 1;
        }
        if name != "review-loading" {
            super::finish_fixture_reads(state.live.as_mut().unwrap());
        }
    }
}
