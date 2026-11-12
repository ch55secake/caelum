use caelum_core::application::Task;
use caelum_protocol::GitHubPullRequestDetails;
use rune::{El, TextStyle, col, icon, row, text};
use rune_kit::Kit;

use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    quiet_button, ui,
};

pub(super) fn view(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let target = l.panels.review.as_ref();
    let details = target
        .and_then(|key| l.app.services.reviews.get(key))
        .and_then(|review| review.details.as_ref());
    let mut navigation = row().w_full().center_y().gap(8.0).child(
        quiet_button(&ui::BACK, "Back to session")
            .on_press(|s: &mut Workspace| {
                let l = live_mut(s);
                l.panels.screen = Screen::Conversation;
                l.focus();
            })
            .build(k)
            .key("back-to-session"),
    );
    if let Some((project, url)) = target {
        // Identity comes from the reviewed repository, which can differ from the
        // selected session's project or the local checkout directory's name.
        let (repository, number) = url
            .split_once("://")
            .and_then(|(_, rest)| rest.split_once('/'))
            .and_then(|(_, path)| path.split_once("/pull/"))
            .unwrap_or(("Pull request", ""));
        navigation = navigation
            .child(
                ui::caption(repository, k)
                    .truncate()
                    .min_w(0.0)
                    .shrink()
                    .help(url)
                    .key("review-repository"),
            )
            .children((!number.is_empty()).then(|| {
                ui::caption(format!("#{number}"), k)
                    .color(k.c.text_base)
                    .key("review-number")
            }))
            .children(details.map(|details| state(details, k)))
            .child(rune::el().grow(1.0));
        let link = url.clone();
        let project = project.clone();
        let url = url.clone();
        navigation = navigation
            .child(
                quiet_button(&ui::EXTERNAL, "Open pull request on GitHub")
                    .on_press_tx(move |_: &mut Workspace, tx| {
                        let _ = tx.open_url(&link);
                    })
                    .build(k)
                    .key("review-github"),
            )
            .child(
                quiet_button(&ui::REFRESH, "Refresh review")
                    .on_press(move |s: &mut Workspace| {
                        let l = live_mut(s);
                        l.app.services.refresh_review(&project, &url);
                        l.app.task(Task::Details {
                            project: project.clone(),
                            url: url.clone(),
                        });
                    })
                    .build(k)
                    .key("review-refresh"),
            );
    } else {
        navigation = navigation.child(ui::caption("Pull requests", k));
    }
    let mut header = col()
        .key("review-header")
        .w_full()
        .gap(6.0)
        .child(navigation);
    let Some(details) = details else {
        return header;
    };
    let author = details
        .author
        .as_ref()
        .map_or("Unknown author", |author| author.login.as_str());
    header = header
        .child(
            text(&details.pull_request.title)
                .key("review-title")
                .font(TextStyle::new(20.0, 550.0).lh(26.0))
                .color(k.c.text_base)
                .wrap()
                .max_lines(2)
                .help_when_truncated(&details.pull_request.title),
        )
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    ui::caption(author, k)
                        .max_w(120.0)
                        .shrink()
                        .min_w(0.0)
                        .truncate()
                        .help(format!("Opened by {author}")),
                )
                .child(ui::caption("·", k).a11y_hidden())
                .child(
                    row()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(6.0)
                        .center_y()
                        .child(branch(&details.head_ref_name, true, k))
                        .child(icon(&rune_kit::glyphs::ARROW_RIGHT, 12.0).color(k.c.icon_muted))
                        .child(branch(&details.pull_request.base_ref_name, false, k)),
                ),
        );
    let mut tabs = row().gap(4.0);
    for (tab, name) in [
        "Overview".to_owned(),
        format!("Files changed ({})", details.changed_files),
    ]
    .into_iter()
    .enumerate()
    {
        tabs = tabs.child(
            ui::tab(&name, l.panels.review_tab == tab, k)
                .on_press(move |s: &mut Workspace| live_mut(s).panels.review_tab = tab)
                .key(format!("review-tab-{tab}")),
        );
    }
    header
        .child(
            row()
                .w_full()
                .flex_wrap()
                .center_y()
                .justify_between()
                .gap(6.0)
                .child(tabs)
                .child(
                    row()
                        .gap(6.0)
                        .child(
                            ui::caption(format!("+{}", details.additions), k).color(k.c.success.fg),
                        )
                        .child(
                            ui::caption(format!("−{}", details.deletions), k).color(k.c.danger.fg),
                        ),
                ),
        )
        .child(ui::divider(k))
}

fn branch(name: &str, head: bool, k: &Kit) -> El {
    text(name)
        .key(if head { "review-head" } else { "review-base" })
        .font(TextStyle::new(11.0, 450.0).mono())
        .color(if head {
            k.c.text_accent
        } else {
            k.c.text_muted
        })
        .truncate()
        .shrink()
        .min_w(0.0)
        .max_w(280.0)
        .px(6.0)
        .py(3.0)
        .radius(4.0)
        .fill(k.c.layer_02)
        .help(format!(
            "{} branch: {name}",
            if head { "Head" } else { "Base" }
        ))
}

fn state(details: &GitHubPullRequestDetails, k: &Kit) -> El {
    let (name, color) = match details.state.as_str() {
        "MERGED" => ("Merged", k.c.text_accent),
        "CLOSED" => ("Closed", k.c.danger.fg),
        _ if details.pull_request.is_draft => ("Draft", k.c.text_muted),
        "OPEN" => ("Open", k.c.success.fg),
        other => (other, k.c.text_muted),
    };
    ui::caption(name, k)
        .key("review-state")
        .color(color)
        .px(7.0)
        .py(2.0)
        .radius(5.0)
        .fill(k.alpha(color, 0.10))
        .border(1.0, k.alpha(color, 0.18))
}
