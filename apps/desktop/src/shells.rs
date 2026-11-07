//! Bounded command previews and a captured-session output reader.
use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    quiet_button, ui,
};
use caelum_core::application::{
    Scope, Task, Workspace as Application,
    presentation::{Outcome, duration, now},
};
use caelum_protocol::OpenCodeBackgroundShell;
use rune::{El, col, row, text};
use rune_kit::{ButtonVariant, Kit, ScrollBar};

pub struct Panel {
    pub index: usize,
    scope: Scope,
    remote: String,
    commands: Vec<OpenCodeBackgroundShell>,
    selected: String,
    output: std::collections::HashMap<String, String>,
    revision: u64,
}
impl Panel {
    pub fn sync(&mut self, app: &Application) {
        if self.revision == app.services.revision {
            return;
        }
        self.revision = app.services.revision;
        for shell in &self.commands {
            if let Some(output) = output(app, &self.scope.project, &self.remote, &shell.id) {
                self.output.insert(shell.id.clone(), output.into());
            }
        }
    }
}
fn output<'a>(app: &'a Application, project: &str, remote: &str, id: &str) -> Option<&'a str> {
    app.services
        .checkouts
        .get(project)?
        .shell_output
        .get(&(remote.into(), id.into()))
        .map(String::as_str)
}
fn task(project: &str, remote: &str, id: &str, stop: bool) -> Task {
    Task::Shell {
        project: project.into(),
        session: remote.into(),
        id: id.into(),
        stop,
    }
}
pub fn open(s: &mut Workspace, index: usize, id: String) {
    let l = live_mut(s);
    let session = &l.app.sessions[index];
    let Some(remote) = session.remote_id.clone() else {
        return;
    };
    let mut panel = Panel {
        index,
        scope: session.scope.clone(),
        remote,
        commands: session.shells.clone(),
        selected: id,
        output: Default::default(),
        revision: u64::MAX,
    };
    panel.sync(&l.app);
    l.app.task(task(
        &panel.scope.project,
        &panel.remote,
        &panel.selected,
        false,
    ));
    l.shell_panel = Some(panel);
    l.panels.overlay = Some(Screen::Shells);
}
fn header(
    app: &Application,
    shell: &OpenCodeBackgroundShell,
    project: &str,
    remote: &str,
    running: bool,
    prefix: &str,
    k: &Kit,
) -> El {
    let read = task(project, remote, &shell.id, false);
    let stop = task(project, remote, &shell.id, true);
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(ui::outcome(
            if running {
                Outcome::Running
            } else {
                Outcome::Neutral
            },
            k,
        ))
        .child(
            text(&shell.command)
                .font(k.ty.mono)
                .color(k.c.text_base)
                .truncate()
                .min_w(0.0)
                .grow(1.0)
                .help(&shell.command),
        )
        .child(ui::caption(
            if running {
                duration(now().saturating_sub(shell.started_at))
            } else {
                "Finished".into()
            },
            k,
        ))
        .child(
            quiet_button(&ui::REFRESH, "Refresh command output")
                .disabled(!running || app.services.busy(&read))
                .on_press(move |s: &mut Workspace| live_mut(s).app.task(read.clone()))
                .build(k)
                .key(format!("{prefix}refresh-shell-{}", shell.id)),
        )
        .child(
            ui::Button::new(if app.services.busy(&stop) {
                "Stopping…"
            } else {
                "Stop"
            })
            .variant(ButtonVariant::GhostMuted)
            .disabled(!running || app.services.busy(&stop))
            .on_press(move |s: &mut Workspace| live_mut(s).app.task(stop.clone()))
            .build(k)
            .key(format!("{prefix}stop-shell-{}", shell.id)),
        )
}
pub fn view(s: &Workspace, k: &Kit, index: usize) -> El {
    let l = live(s);
    let session = &l.app.sessions[index];
    let (Some(shell), Some(remote)) = (session.shells.first(), session.remote_id.as_ref()) else {
        return col();
    };
    let mut preview = col()
        .w_full()
        .gap(1.0)
        .key("shell-preview")
        .max_h(50.0)
        .clip_rounded(0.0);
    let value = output(&l.app, &session.scope.project, remote, &shell.id)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("Waiting for output…");
    let lines: Vec<_> = value.lines().rev().take(3).collect();
    for line in lines.into_iter().rev() {
        preview = preview.child(
            text(line)
                .font(rune::TextStyle {
                    size: 11.0,
                    line_height: 16.0,
                    ..k.ty.mono
                })
                .color(k.c.text_muted)
                .truncate()
                .w_full()
                .h(16.0),
        );
    }
    let id = shell.id.clone();
    let mut body = col()
        .key("background-shells")
        .w_full()
        .gap(4.0)
        .padding(9.0)
        .radius(8.0)
        .fill(k.c.layer_01)
        .child(header(
            &l.app,
            shell,
            &session.scope.project,
            remote,
            true,
            "",
            k,
        ))
        .child(preview)
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    ui::caption(
                        if session.shells.len() > 1 {
                            format!("{} background commands", session.shells.len())
                        } else {
                            "Background command".into()
                        },
                        k,
                    )
                    .grow(1.0),
                )
                .child(
                    ui::Button::new(if session.shells.len() > 1 {
                        "View all"
                    } else {
                        "View output"
                    })
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(move |s: &mut Workspace| open(s, index, id.clone()))
                    .build(k)
                    .key("view-shell-output"),
                ),
        );
    for stop in [true, false] {
        if let Some(error) =
            l.app
                .services
                .error(&task(&session.scope.project, remote, &shell.id, stop))
        {
            body = body.child(
                ui::caption(error, k)
                    .truncate()
                    .color(k.c.danger.fg)
                    .help(error),
            );
            break;
        }
    }
    body
}
pub fn panel(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let Some(panel) = &l.shell_panel else {
        return col();
    };
    let mut body = col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(10.0)
        .child(ui::caption(&l.app.sessions[panel.index].title, k));
    if panel.commands.len() > 1 {
        let mut list = col().w_full().gap(2.0).focus_group();
        for command in &panel.commands {
            let id = command.id.clone();
            list = list.child(
                ui::choice(&command.command, &command.id, panel.selected == id, k).on_press(
                    move |s: &mut Workspace| {
                        let l = live_mut(s);
                        let panel = l.shell_panel.as_mut().unwrap();
                        panel.selected.clone_from(&id);
                        if l.app.sessions[panel.index]
                            .shells
                            .iter()
                            .any(|c| c.id == id)
                        {
                            l.app
                                .task(task(&panel.scope.project, &panel.remote, &id, false));
                        }
                    },
                ),
            );
        }
        body = body.child(
            col()
                .w_full()
                .max_h(110.0)
                .scroll(rune::ScrollSpec {
                    stick_bottom: false,
                })
                .child(list)
                .scroll_bar(k),
        );
    }
    let Some(command) = panel.commands.iter().find(|c| c.id == panel.selected) else {
        return body;
    };
    let running = l.app.sessions[panel.index].scope == panel.scope
        && l.app.sessions[panel.index]
            .shells
            .iter()
            .any(|c| c.id == command.id);
    let output = output(&l.app, &panel.scope.project, &panel.remote, &command.id)
        .or_else(|| panel.output.get(&command.id).map(String::as_str))
        .unwrap_or("Waiting for output…");
    body = body.child(header(
        &l.app,
        command,
        &panel.scope.project,
        &panel.remote,
        running,
        "output-",
        k,
    ));
    if let Some(error) = l
        .app
        .services
        .error(&task(
            &panel.scope.project,
            &panel.remote,
            &command.id,
            false,
        ))
        .or_else(|| {
            l.app.services.error(&task(
                &panel.scope.project,
                &panel.remote,
                &command.id,
                true,
            ))
        })
    {
        body = body.child(ui::caption(error, k).color(k.c.danger.fg));
    }
    body.child(
        col()
            .key(format!("shell-output-{}", command.id))
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .radius(8.0)
            .fill(k.c.bg_deep)
            .scroll(rune::ScrollSpec { stick_bottom: true })
            .child(
                text(output)
                    .font(k.ty.mono)
                    .color(k.c.text_base)
                    .wrap()
                    .selectable(true)
                    .padding(12.0),
            )
            .scroll_bar(k),
    )
}
