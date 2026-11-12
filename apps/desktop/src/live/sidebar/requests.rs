use crate::{Workspace, ui};
use rune::{El, ScrollSpec, col};
use rune_kit::{
    Kit, ScrollBar,
    shell::{SidebarHeader, SidebarRow},
};
use std::collections::HashSet;

pub(super) fn view(s: &Workspace, k: &Kit) -> El {
    let l = super::live(s);
    let app = &l.app;
    let mut seen = HashSet::new();
    let mut rows = col()
        .key("requests-list")
        .role("list")
        .w_full()
        .gap(4.0)
        .focus_group();
    for (project, checkout) in &app.services.checkouts {
        if !app.feed_includes_project(project, l.feed.project.as_deref()) {
            continue;
        }
        for pr in &checkout.pull_requests {
            if !seen.insert(&pr.url) {
                continue;
            }
            let name = crate::checkouts::project_name(app, app.group_path(project));
            let (project, url) = (project.clone(), pr.url.clone());
            let context_project = project.clone();
            rows = rows.child(crate::context::pull_request(
                SidebarRow::new(&pr.title)
                    .icon(&ui::BRANCH)
                    .subtitle(format!(
                        "{name} · #{} · into {}",
                        pr.number, pr.base_ref_name
                    ))
                    .height(52.0)
                    .title_style(k.ty.ui)
                    .subtitle_style(k.ty.small)
                    .on_select(move |s: &mut Workspace, _| {
                        crate::panels::open_review(s, project.clone(), url.clone())
                    })
                    .build(k)
                    .help(format!(
                        "{name} · #{} · {}\ninto {}",
                        pr.number, pr.title, pr.base_ref_name
                    ))
                    .key(crate::code_view::source_key(&pr.url, "pull-request")),
                context_project,
                pr.url.clone(),
            ));
        }
    }
    col()
        .w_full()
        .min_h(0.0)
        .max_h(180.0)
        .child(ui::divider(k))
        .child(
            SidebarHeader::new("Pull requests")
                .count(seen.len())
                .build(k)
                .key("requests-heading"),
        )
        .child(
            col()
                .key("requests-scroll")
                .w_full()
                .min_h(0.0)
                .max_h(if seen.is_empty() { 32.0 } else { 140.0 })
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(if seen.is_empty() {
                    ui::caption("No pull requests in this scope", k).padding(8.0)
                } else {
                    rows
                })
                .scroll_bar(k),
        )
}
