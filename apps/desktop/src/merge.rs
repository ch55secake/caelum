//! GitHub-style review readiness, expandable checks and a joined merge action.
use crate::{
    Workspace,
    live::{live, live_mut},
    select::Select,
    ui,
};
use caelum_core::application::{
    Task,
    presentation::{Check, CheckSummary, MergeReadiness, Outcome, now},
};
use caelum_protocol::{GitHubMergeMethod, GitHubPullRequestDetails};
use rune::{El, col, row, text};
use rune_kit::{ButtonVariant, Kit, SelectAppearance, glyphs};

fn status(title: &str, detail: &str, outcome: Outcome, k: &Kit) -> El {
    row()
        .w_full()
        .gap(12.0)
        .px(16.0)
        .py(12.0)
        .center_y()
        .child(if outcome == Outcome::Running {
            rune::icon(&crate::ORBIT, 14.0).color(k.c.icon_muted)
        } else {
            ui::outcome(outcome, k)
        })
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(3.0)
                .child(if outcome == Outcome::Running {
                    ui::busy_text(title, k.ty.ui_medium, k)
                } else {
                    text(title).font(k.ty.ui_medium).color(k.c.text_base).wrap()
                })
                .child(ui::caption(detail, k)),
        )
}

pub fn view(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    details: &GitHubPullRequestDetails,
    layers: &mut Vec<El>,
) -> El {
    let l = live(s);
    let owner = (project.to_owned(), url.to_owned());
    let open = l.panels.expanded_checks.contains(&owner);
    let summary = CheckSummary::new(details.status_check_rollup.as_deref());
    let (review, explanation, outcome) = match details.review_decision.as_deref() {
        Some("APPROVED") => (
            "Review approved",
            "The latest review decision is approved.",
            Outcome::Success,
        ),
        Some("CHANGES_REQUESTED") => (
            "Changes requested",
            "Address the requested changes before merging.",
            Outcome::Failure,
        ),
        Some("REVIEW_REQUIRED") => (
            "Review required",
            "An approving review is required for this pull request.",
            Outcome::Interrupted,
        ),
        _ => (
            "No review decision reported",
            "Review requirements are determined by this repository.",
            Outcome::Neutral,
        ),
    };
    let mut body = col()
        .key("review-merge-box")
        .w_full()
        .radius(8.0)
        .clip_rounded(8.0)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_base)
        .child(status(review, explanation, outcome, k).key("review-approval"))
        .child(ui::divider(k));
    let mut checks =
        status(summary.title, &summary.detail, summary.outcome, k).key("review-check-summary");
    if details
        .status_check_rollup
        .as_ref()
        .is_some_and(|checks| !checks.is_empty())
    {
        checks = checks.child(
            ui::Button::new(if open { "Hide" } else { "Details" })
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| {
                    let expanded = &mut live_mut(s).panels.expanded_checks;
                    if !expanded.remove(&owner) {
                        expanded.insert(owner.clone());
                    }
                })
                .build(k)
                .key("review-checks-toggle")
                .label("Check details")
                .expanded(open),
        );
    }
    body = body.child(checks);
    if open {
        let mut reported: Vec<_> = details
            .status_check_rollup
            .iter()
            .flatten()
            .map(|c| Check::new(c, now()))
            .collect();
        reported.sort_by_key(|c| match c.outcome {
            Outcome::Failure | Outcome::Interrupted => 0,
            Outcome::Running => 1,
            Outcome::Success => 2,
            _ => 3,
        });
        let mut list = col().w_full().pb(8.0);
        for (i, check) in reported.into_iter().enumerate() {
            let url = check.url.map(str::to_owned);
            list = list.child(
                k.interactive(
                    row()
                        .w_full()
                        .gap(8.0)
                        .px(16.0)
                        .py(8.0)
                        .center_y()
                        .child(if check.outcome == Outcome::Running {
                            rune::icon(&crate::ORBIT, 14.0).color(k.c.icon_muted)
                        } else {
                            ui::outcome(check.outcome, k)
                        })
                        .child(
                            text(check.name)
                                .font(k.ty.small_medium)
                                .color(k.c.text_base)
                                .truncate()
                                .min_w(0.0)
                                .grow(1.0)
                                .key(format!("check-name-{i}")),
                        )
                        .child(if check.outcome == Outcome::Running {
                            ui::busy_text("Running", k.ty.small, k)
                                .w(62.0)
                                .help(check.label)
                        } else {
                            ui::caption(
                                match check.outcome {
                                    Outcome::Success => "Passed",
                                    Outcome::Failure => "Failed",
                                    Outcome::Running => "Running",
                                    Outcome::Interrupted => "Cancelled",
                                    Outcome::Neutral => "Skipped",
                                    Outcome::Unknown => "Unknown",
                                },
                                k,
                            )
                            .help(check.label)
                            .w(62.0)
                        })
                        .child(ui::caption(check.timing, k).w(60.0))
                        .child(
                            rune::icon(&ui::EXTERNAL, 12.0)
                                .color(k.c.icon_muted)
                                .opacity(if url.is_some() { 1.0 } else { 0.0 }),
                        ),
                )
                .key(format!("check-{i}"))
                .role("link")
                .label(check.name)
                .focusable(url.is_some())
                .focus_ring(1.0, 0.0, k.c.border_focus)
                .on_press_tx(move |_: &mut Workspace, tx| {
                    if let Some(url) = &url {
                        let _ = tx.open_url(url);
                    }
                }),
            );
        }
        body = body.child(list);
    }
    let readiness = MergeReadiness::new(details);
    let mut branch =
        status(readiness.title, readiness.detail, readiness.outcome, k).key("review-branch");
    if readiness.behind {
        let task = Task::UpdateBranch {
            project: project.into(),
            url: url.into(),
            head: details.head_ref_oid.clone(),
        };
        let busy = l.app.services.busy(&task);
        branch = branch.child(
            ui::Button::new(if busy { "Updating…" } else { "Update branch" })
                .disabled(busy)
                .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
                .build(k)
                .key("update-review-branch"),
        );
    }
    body = body.child(ui::divider(k)).child(branch);
    if details.state != "OPEN" || details.pull_request.is_draft {
        return body;
    }

    let methods = [
        ("Squash and merge", GitHubMergeMethod::Squash),
        ("Create merge commit", GitHubMergeMethod::Merge),
        ("Rebase and merge", GitHubMergeMethod::Rebase),
    ];
    let (name, method) = methods[l.panels.merge_method];
    let task = Task::Merge {
        project: project.into(),
        url: url.into(),
        head: details.head_ref_oid.clone(),
        method,
    };
    let busy = l.app.services.busy(&task);
    let green = rune::color::hex(if k.dark { "#238636" } else { "#1f883d" }).unwrap();
    let mut merge_kit = k.clone();
    merge_kit.c.bg_accent = green;
    merge_kit.c.icon_muted = rune::Color::WHITE;
    let names: Vec<_> = methods.iter().map(|m| m.0).collect();
    let selector = Select::new("merge-method", &names)
        .current(Some(name))
        .display("")
        .label("Merge method")
        .appearance(SelectAppearance::Field)
        .width(34.0)
        .indicator(&glyphs::CHEVRON_DOWN)
        .disabled(busy)
        .open(l.panels.merge_menu)
        .on_toggle(|s: &mut Workspace| live_mut(s).panels.merge_menu = !live(s).panels.merge_menu)
        .on_close(|s: &mut Workspace| live_mut(s).panels.merge_menu = false)
        .on_choose(move |s: &mut Workspace, value, _| {
            let p = &mut live_mut(s).panels;
            p.merge_method = methods.iter().position(|m| m.0 == value).unwrap_or(0);
            p.merge_menu = false;
        })
        .build(&merge_kit, layers)
        .h(34.0)
        .radius(0.0)
        .fill(green)
        .border(0.0, rune::Color::TRANSPARENT);
    body.child(ui::divider(k)).child(
        row().w_full().px(16.0).py(12.0).child(
            row()
                .key("merge-controls")
                .h(34.0)
                .radius(6.0)
                .clip_rounded(6.0)
                .fill(green)
                .child(
                    ui::Button::new(if busy { "Working…" } else { name })
                        .variant(ButtonVariant::Contrast)
                        .disabled(busy || !readiness.can_merge)
                        .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
                        .build(&merge_kit)
                        .key("merge-pull-request")
                        .h(34.0)
                        .radius(0.0),
                )
                .child(
                    rune::el()
                        .w(1.0)
                        .h_full()
                        .fill(k.alpha(rune::Color::WHITE, 0.25)),
                )
                .child(selector),
        ),
    )
}
