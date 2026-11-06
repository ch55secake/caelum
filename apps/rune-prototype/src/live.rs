//! Rune adapter: editors and disclosure state stay here; application requests,
//! provider activity and history reconciliation live in caelum-core.

use std::{cell::Cell, collections::HashSet, rc::Rc};

use caelum_core::application::{
    EntryKind, Event, Provider, Runtime, Scope, Workspace as Application,
};
use caelum_protocol::ApprovalDecision;
use rune::{
    El, FileDialogOptions, ScrollPos, ScrollSpec, TextEdit, TextStyle, Tx, Waker, col, row, text,
    text_input,
};
use rune_kit::shell::{RowStatus, Sidebar, SidebarHeader, SidebarRow};
use rune_kit::{Button, ButtonVariant, Kit, Markdown, Select, SelectAppearance, glyphs};

use crate::{PLUS, SEND, STOP, Workspace, on_key, quiet_button, theme};

pub struct Live {
    pub app: Application,
    pub drafts: Vec<TextEdit>,
    runtime: Option<Runtime>,
    expanded: HashSet<(usize, String)>,
    menu: Option<&'static str>,
    reading: Vec<Rc<Cell<ScrollPos>>>,
    last_selected: Option<usize>,
}

impl Live {
    pub fn new(project: Option<String>) -> Self {
        let mut app = Application::default();
        if let Some(project) = project {
            app.open_project(project);
        }
        Self {
            app,
            drafts: Vec::new(),
            runtime: None,
            expanded: HashSet::new(),
            menu: None,
            reading: Vec::new(),
            last_selected: None,
        }
    }

    fn sync_editors(&mut self) {
        self.drafts
            .resize_with(self.app.sessions.len(), || TextEdit::new(""));
        self.reading.resize_with(self.app.sessions.len(), || {
            Rc::new(Cell::new(ScrollPos::default()))
        });
    }

    pub fn new_session(&mut self) {
        let Some(project) = self.app.active_project.clone() else {
            return;
        };
        let provider = self
            .app
            .selected
            .map(|i| self.app.sessions[i].scope.provider)
            .unwrap_or(Provider::Codex);
        let index = self.app.new_session(Scope { project, provider });
        self.sync_editors();
        self.drafts[index].focused = true;
        self.menu = None;
    }

    pub fn focus(&mut self) {
        if let Some(index) = self.app.selected {
            self.drafts[index].focused = true;
        }
    }

    pub fn working(&self) -> bool {
        self.app
            .selected
            .is_some_and(|index| self.app.sessions[index].working())
    }

    pub fn stop(&mut self) {
        if let Some(index) = self.app.selected {
            self.app.cancel(index);
        }
    }

    pub fn send(&mut self, tx: &mut Tx) {
        let Some(index) = self.app.selected else {
            return;
        };
        if self.app.send(index, self.drafts[index].text.clone()) {
            self.drafts[index].clear();
            self.drafts[index].focused = true;
            tx.scroll_to_bottom(format!("live-transcript-{index}"));
        }
    }

    pub fn poll(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(runtime) = &mut self.runtime {
            let events = runtime.drain();
            changed = !events.is_empty();
            for event in events {
                self.app.apply(event);
            }
            for (id, command) in self.app.commands() {
                if let Err(error) = runtime.submit(id, command) {
                    self.app.apply(Event::Completed(id, Err(error)));
                    changed = true;
                }
            }
        }
        self.sync_editors();
        if self.last_selected != self.app.selected {
            self.last_selected = self.app.selected;
            if let Some(index) = self.app.selected {
                let position = self.reading[index].get();
                let key = format!("live-transcript-{index}");
                if position.offset[1] >= position.max[1] - 1.0 {
                    tx.scroll_to_bottom_instant(key);
                } else {
                    tx.scroll_to_y(key, position.offset[1]);
                }
            }
            changed = true;
        }
        changed
    }
}

pub fn setup(s: &mut Workspace, wake: Waker) {
    if let Some(live) = &mut s.live {
        live.runtime = Some(Runtime::new(move || wake()));
    }
}

fn live(s: &Workspace) -> &Live {
    s.live.as_ref().expect("live workspace")
}
fn live_mut(s: &mut Workspace) -> &mut Live {
    s.live.as_mut().expect("live workspace")
}

pub fn open_project(_: &mut Workspace, tx: &mut Tx) {
    tx.open_folder(
        FileDialogOptions {
            title: "Open a project".into(),
            ..Default::default()
        },
        |s: &mut Workspace, result, _| match result {
            Ok(Some(path)) => live_mut(s)
                .app
                .open_project(path.to_string_lossy().into_owned()),
            Ok(None) => {}
            Err(error) => live_mut(s).app.error = Some(error.to_string()),
        },
    );
}

pub fn sidebar(s: &Workspace, k: &Kit) -> El {
    let app = &live(s).app;
    let mut sidebar = Sidebar::new()
        .label("Projects and sessions")
        .inset(0.0)
        .row_gap(5.0)
        .header_gap(8.0)
        .header(
            text("caelum")
                .font(k.ty.body_medium)
                .color(k.c.text_base)
                .padding(8.0),
        )
        .header(
            Button::new(if app.opening {
                "Opening project…"
            } else {
                "Open project"
            })
            .icon(&PLUS)
            .variant(ButtonVariant::Outline)
            .on_press_tx(open_project)
            .build(k)
            .key("open-project")
            .w_full(),
        );
    for (index, project) in app.projects.iter().enumerate() {
        let path = project.path.clone();
        let activity = app
            .sessions
            .iter()
            .filter(|session| session.scope.project == path && session.working())
            .count();
        let requests = app
            .approvals
            .iter()
            .filter(|p| p.project_path == path)
            .count()
            + app
                .questions
                .iter()
                .filter(|p| p.project_path == path)
                .count();
        sidebar = sidebar.header(
            SidebarRow::new(project.name.clone())
                .subtitle(if requests > 0 {
                    format!("{requests} pending request(s)")
                } else if activity > 0 {
                    format!("{activity} active session(s)")
                } else {
                    path.clone()
                })
                .selected(app.active_project.as_ref() == Some(&path))
                .on_select(move |s: &mut Workspace, _| {
                    let live = live_mut(s);
                    live.app.select_project(&path);
                    live.sync_editors();
                    live.menu = None;
                })
                .build(k)
                .key(format!("project-{index}")),
        );
    }
    sidebar = sidebar
        .header(
            Button::new("New session")
                .icon(&PLUS)
                .variant(ButtonVariant::Outline)
                .disabled(app.active_project.is_none())
                .on_press(|s: &mut Workspace| s.new_session())
                .build(k)
                .key("new-session")
                .w_full(),
        )
        .header(
            Button::new("Refresh sessions")
                .variant(ButtonVariant::GhostMuted)
                .disabled(app.active_project.is_none())
                .on_press(|s: &mut Workspace| live_mut(s).app.refresh_project())
                .build(k)
                .key("refresh-sessions"),
        )
        .header(SidebarHeader::new("Sessions").build(k));
    for (index, session) in app
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| Some(&session.scope.project) == app.active_project.as_ref())
    {
        let pending = app.approvals.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && p.session_id == session.remote_id
        }) || app.questions.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && Some(&p.session_id) == session.remote_id.as_ref()
        });
        let mut item = SidebarRow::new(session.title.clone())
            .height(52.0)
            .subtitle(format!(
                "{} · {}",
                session.scope.provider.as_str(),
                if pending {
                    "Needs an answer"
                } else if session.stopping {
                    "Stopping…"
                } else if session.working() {
                    "Working…"
                } else if session.loading {
                    "Loading history…"
                } else {
                    "Ready"
                }
            ))
            .selected(app.selected == Some(index))
            .on_select(move |s: &mut Workspace, _| {
                let live = live_mut(s);
                live.app.select(index);
                live.menu = None;
            });
        if session.working() {
            item = item.status(RowStatus::Busy);
        }
        sidebar = sidebar.child(
            item.build(k)
                .key(format!("live-session-{index}"))
                .radius(10.0),
        );
    }
    let mut footer = col().gap(6.0).padding(8.0).child(
        text("Rune · real sessions")
            .font(k.ty.small_medium)
            .color(k.c.text_accent),
    );
    for provider in Provider::ALL {
        if let Some(path) = &app.active_project
            && let Some(error) = app.notices.get(&Scope {
                provider,
                project: path.clone(),
            })
        {
            footer = footer.child(
                text(format!("{}: {error}", provider.as_str()))
                    .font(k.ty.small)
                    .color(k.c.text_muted)
                    .wrap(),
            );
        }
    }
    sidebar.footer(footer).build(k)
}

pub fn transcript(s: &Workspace, k: &Kit, reduced: bool, padding: f32) -> El {
    let live = live(s);
    let mut content = col().w_full().max_w(840.0).px(padding).py(24.0).gap(24.0);
    if let Some(error) = &live.app.error {
        content = content.child(
            text(error.clone())
                .font(k.ty.ui)
                .color(k.c.text_accent)
                .wrap()
                .selectable(true),
        );
    }
    let Some(index) = live.app.selected else {
        return content
            .child(
                text("Open a project to continue")
                    .font(TextStyle::new(24.0, 550.0))
                    .color(k.c.text_base),
            )
            .child(
                text("Connect to your installed Codex and OpenCode providers.")
                    .font(k.ty.ui)
                    .color(k.c.text_muted)
                    .wrap(),
            )
            .child(
                Button::new("Open project")
                    .variant(ButtonVariant::Outline)
                    .on_press_tx(open_project)
                    .build(k)
                    .key("welcome-open"),
            )
            .grow(1.0);
    };
    let session = &live.app.sessions[index];
    if session.loading {
        content = content.child(
            text("Loading history…")
                .font(k.ty.small)
                .color(k.c.text_muted),
        );
    }
    if session.next_cursor.is_some() {
        content = content.child(
            Button::new("Load older messages")
                .variant(ButtonVariant::GhostMuted)
                .disabled(session.loading)
                .on_press(move |s: &mut Workspace| live_mut(s).app.older(index))
                .build(k)
                .key("load-older")
                .self_start(),
        );
    }
    if session.conversation.entries.is_empty() && !session.loading {
        content = content
            .child(
                text("A little room to think")
                    .font(TextStyle::new(22.0, 550.0))
                    .color(k.c.text_base)
                    .wrap(),
            )
            .child(
                text("Choose your provider in Session options, then send a message.")
                    .font(k.ty.ui)
                    .color(k.c.text_muted)
                    .wrap(),
            );
    }
    for entry in &session.conversation.entries {
        let mut item = col()
            .w_full()
            .gap(8.0)
            .key(format!("live-entry-{index}-{}", entry.id));
        match entry.kind {
            EntryKind::User => {
                item = item
                    .padding(16.0)
                    .radius(10.0)
                    .fill(k.c.layer_01)
                    .border(1.0, k.c.border_base)
                    .child(
                        text(entry.text.clone())
                            .font(TextStyle::new(14.0, 500.0).lh(23.0))
                            .color(k.c.text_base)
                            .wrap()
                            .selectable(true),
                    );
            }
            EntryKind::Assistant => {
                let id = entry.id.clone();
                item = item
                    .child(
                        row()
                            .w_full()
                            .center_y()
                            .justify_between()
                            .child(
                                text(if entry.complete {
                                    session.scope.provider.as_str()
                                } else {
                                    "Responding…"
                                })
                                .font(k.ty.small)
                                .color(k.c.text_muted),
                            )
                            .child(
                                quiet_button(&glyphs::COPY, "Copy response")
                                    .on_press_tx(move |s: &mut Workspace, tx| {
                                        if let Some(entry) = live_mut(s).app.sessions[index]
                                            .conversation
                                            .entries
                                            .iter()
                                            .find(|entry| entry.id == id)
                                        {
                                            tx.copy(entry.text.clone());
                                        }
                                    })
                                    .build(k),
                            ),
                    )
                    .child(
                        Markdown::new(&entry.text)
                            .selectable(true)
                            .streaming(!entry.complete && session.working())
                            .reveal(if reduced { 0.0 } else { 0.12 })
                            .render(&theme::markdown(k)),
                    );
            }
            EntryKind::Tool | EntryKind::Reasoning => {
                let id = entry.id.clone();
                let expanded = live.expanded.contains(&(index, id.clone()));
                item = item.child(
                    Button::new(if entry.kind == EntryKind::Reasoning {
                        "Thinking".into()
                    } else if entry.name.is_empty() {
                        "Tool".into()
                    } else {
                        entry.name.clone()
                    })
                    .icon(if expanded {
                        &glyphs::CHEVRON_DOWN
                    } else {
                        &glyphs::CARET_RIGHT
                    })
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(move |s: &mut Workspace| {
                        let expanded = &mut live_mut(s).expanded;
                        let key = (index, id.clone());
                        if !expanded.remove(&key) {
                            expanded.insert(key);
                        }
                    })
                    .build(k)
                    .expanded(expanded)
                    .self_start(),
                );
                if expanded {
                    item = item.child(
                        text(if entry.kind == EntryKind::Reasoning {
                            entry.text.clone()
                        } else {
                            format!("{}\n{}", entry.input, entry.output)
                        })
                        .font(k.ty.mono)
                        .color(k.c.text_muted)
                        .wrap()
                        .selectable(true)
                        .padding(12.0)
                        .radius(8.0)
                        .fill(k.c.layer_01),
                    );
                }
            }
            EntryKind::Status => {
                item = item.child(
                    text(entry.text.clone())
                        .font(k.ty.small)
                        .color(k.c.text_muted)
                        .wrap(),
                );
            }
        }
        content = content.child(item);
    }
    if let Some(error) = &session.error {
        content = content.child(
            text(error.clone())
                .font(k.ty.ui)
                .color(k.c.text_accent)
                .wrap()
                .selectable(true),
        );
    }
    if let Some(prompt) = &session.failed_prompt {
        let prompt = prompt.clone();
        content = content.child(
            col()
                .w_full()
                .gap(8.0)
                .child(
                    text(format!("Unsent message\n{prompt}"))
                        .font(k.ty.ui)
                        .color(k.c.text_base)
                        .wrap()
                        .selectable(true),
                )
                .child(
                    Button::new("Restore unsent message")
                        .variant(ButtonVariant::Outline)
                        .disabled(!live.drafts[index].text.is_empty())
                        .on_press(move |s: &mut Workspace| {
                            let live = live_mut(s);
                            if live.drafts[index].text.is_empty() {
                                live.drafts[index] = TextEdit::new(prompt.clone());
                                live.drafts[index].focused = true;
                                live.app.sessions[index].failed_prompt = None;
                            }
                        })
                        .build(k)
                        .key("restore-message")
                        .self_start(),
                ),
        );
    }
    for prompt in live.app.approvals.iter().filter(|p| {
        p.project_path == session.scope.project
            && p.provider == session.scope.provider.as_str()
            && (p.session_id.is_none() || p.session_id == session.remote_id)
    }) {
        let allow = prompt.clone();
        let deny = prompt.clone();
        content = content.child(
            col()
                .w_full()
                .gap(10.0)
                .padding(16.0)
                .radius(10.0)
                .border(1.0, k.c.border_focus)
                .child(
                    text(prompt.summary.clone())
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .wrap(),
                )
                .child(
                    text(prompt.description.clone())
                        .font(k.ty.mono)
                        .color(k.c.text_muted)
                        .wrap()
                        .selectable(true),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(
                            Button::new("Allow once")
                                .variant(ButtonVariant::Outline)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s)
                                        .app
                                        .approve(allow.clone(), ApprovalDecision::AllowOnce)
                                })
                                .build(k)
                                .key(format!("allow-once-{}", prompt.request_id)),
                        )
                        .child(
                            Button::new("Deny")
                                .variant(ButtonVariant::GhostMuted)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s)
                                        .app
                                        .approve(deny.clone(), ApprovalDecision::Deny)
                                })
                                .build(k)
                                .key(format!("deny-{}", prompt.request_id)),
                        ),
                ),
        );
    }
    for prompt in live.app.questions.iter().filter(|p| {
        p.project_path == session.scope.project
            && p.provider == session.scope.provider.as_str()
            && Some(&p.session_id) == session.remote_id.as_ref()
    }) {
        let dismiss = prompt.clone();
        content = content.child(col().w_full().gap(8.0).padding(16.0).radius(10.0).border(1.0, k.c.border_focus)
            .child(text(prompt.title.clone()).font(k.ty.ui_medium).color(k.c.text_base).wrap())
            .child(text("A question is pending. Answer it in the provider client, or dismiss it here. Native question forms are the next migration stage.").font(k.ty.ui).color(k.c.text_muted).wrap())
            .child(Button::new("Dismiss question").variant(ButtonVariant::Outline).on_press(move |s: &mut Workspace| live_mut(s).app.answer(dismiss.clone(), None)).build(k).self_start()));
    }
    let reading = live.reading[index].clone();
    col()
        .key(format!("live-transcript-{index}"))
        .role("region")
        .label("Conversation")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .items_center()
        .scroll(ScrollSpec { stick_bottom: true })
        .on_scroll_offset(move |position| reading.set(position))
        .child(content)
}

pub fn composer(s: &Workspace, k: &Kit, padding: f32, wide: bool) -> El {
    let live = live(s);
    let Some(index) = live.app.selected else {
        return col();
    };
    let session = &live.app.sessions[index];
    let draft = &live.drafts[index];
    let input = text_input(draft, TextStyle::new(14.0, 400.0).lh(23.0))
        .key(format!("live-draft-{index}"))
        .label("Message")
        .placeholder("Message your session…", k.c.text_muted)
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .min_h(56.0)
        .max_h(180.0)
        .on_edit(move |s: &mut Workspace| &mut live_mut(s).drafts[index])
        .on_input_key(on_key)
        .on_submit(Workspace::send);
    let submit = Button::new("")
        .icon(if session.working() { &STOP } else { &SEND })
        .variant(ButtonVariant::Submit)
        .disabled(
            session.stopping
                || (!session.working() && (!session.can_send() || draft.text.trim().is_empty())),
        )
        .on_press_tx(|s: &mut Workspace, tx| {
            if live_mut(s).working() {
                live_mut(s).stop();
            } else {
                s.send(tx);
            }
        })
        .build(k)
        .key("send")
        .label(if session.working() {
            "Stop generation"
        } else {
            "Send message"
        })
        .size(32.0, 32.0);
    let mut controls = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            Button::new(format!(
                "{} · {}",
                session.scope.provider.as_str(),
                if session.stopping {
                    "Stopping…"
                } else if session.working() {
                    "Working…"
                } else {
                    "Session options"
                }
            ))
            .variant(ButtonVariant::GhostMuted)
            .on_press(|s: &mut Workspace| s.inspector.set_open(true))
            .build(k)
            .key("session-options"),
        )
        .child(rune::el().grow(1.0));
    if wide {
        controls = controls.child(
            text("↵ Send · ⇧↵ Newline")
                .font(k.ty.small)
                .color(k.c.text_muted),
        );
    }
    col().w_full().max_w(840.0).px(padding).pb(24.0).child(
        col()
            .w_full()
            .gap(8.0)
            .padding(14.0)
            .radius(14.0)
            .fill(k.c.layer_02)
            .border(
                1.0,
                if draft.focused {
                    k.c.border_focus
                } else {
                    k.c.border_base
                },
            )
            .child(input)
            .child(controls.child(submit)),
    )
}

pub fn inspector(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let mut content = col().w_full().gap(16.0).padding(16.0).child(
        row()
            .w_full()
            .center_y()
            .justify_between()
            .child(
                text("Session options")
                    .font(k.ty.ui_medium)
                    .color(k.c.text_base),
            )
            .child(
                quiet_button(&glyphs::CLOSE, "Hide session options")
                    .on_press(|s: &mut Workspace| s.inspector.set_open(false))
                    .build(k)
                    .key("close-inspector"),
            ),
    );
    let Some(index) = live.app.selected else {
        return content;
    };
    let session = &live.app.sessions[index];
    content = content.child(
        text(session.scope.project.clone())
            .font(k.ty.small)
            .color(k.c.text_muted)
            .wrap()
            .selectable(true),
    );
    let mut layers = Vec::new();
    content = content
        .child(
            text("Provider")
                .font(k.ty.small_medium)
                .color(k.c.text_muted),
        )
        .child(
            Select::new("provider", &["codex", "opencode"])
                .current(Some(session.scope.provider.as_str()))
                .label("Provider")
                .appearance(SelectAppearance::Field)
                .width(240.0)
                .open(live.menu == Some("provider"))
                .disabled(session.remote_id.is_some() || !session.can_send())
                .on_toggle(|s: &mut Workspace| toggle_menu(s, "provider"))
                .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                .on_choose(move |s: &mut Workspace, value, _| {
                    let live = live_mut(s);
                    let session = &mut live.app.sessions[index];
                    session.scope.provider = if value == "codex" {
                        Provider::Codex
                    } else {
                        Provider::OpenCode
                    };
                    session.options = Default::default();
                    if let Some(choices) = live.app.choices.get(&session.scope) {
                        session.options.model = choices.default_model.clone();
                    }
                    live.menu = None;
                })
                .build(k, &mut layers),
        );
    if let Some(choices) = live.app.choices.get(&session.scope) {
        let models: Vec<_> = choices
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect();
        content = content
            .child(text("Model").font(k.ty.small_medium).color(k.c.text_muted))
            .child(
                Select::new("model", &models)
                    .current(session.options.model.as_deref())
                    .placeholder("Provider default")
                    .label("Model")
                    .appearance(SelectAppearance::Field)
                    .width(240.0)
                    .open(live.menu == Some("model"))
                    .disabled(!session.can_send())
                    .on_toggle(|s: &mut Workspace| toggle_menu(s, "model"))
                    .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                    .on_choose(move |s: &mut Workspace, value, _| {
                        let live = live_mut(s);
                        let session = &mut live.app.sessions[index];
                        session.options.model = Some(value.into());
                        session.options.effort = live
                            .app
                            .choices
                            .get(&session.scope)
                            .and_then(|c| c.models.iter().find(|model| model.id == value))
                            .and_then(|model| model.default_effort.clone());
                        live.menu = None;
                    })
                    .build(k, &mut layers),
            );
        if let Some(model) = choices
            .models
            .iter()
            .find(|model| Some(&model.id) == session.options.model.as_ref())
        {
            let efforts: Vec<_> = model
                .efforts
                .iter()
                .map(|effort| effort.id.as_str())
                .collect();
            if !efforts.is_empty() {
                content = content
                    .child(
                        text("Reasoning effort")
                            .font(k.ty.small_medium)
                            .color(k.c.text_muted),
                    )
                    .child(
                        Select::new("effort", &efforts)
                            .current(
                                session
                                    .options
                                    .effort
                                    .as_deref()
                                    .or(model.default_effort.as_deref()),
                            )
                            .label("Reasoning effort")
                            .appearance(SelectAppearance::Field)
                            .width(240.0)
                            .open(live.menu == Some("effort"))
                            .disabled(!session.can_send())
                            .on_toggle(|s: &mut Workspace| toggle_menu(s, "effort"))
                            .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                            .on_choose(move |s: &mut Workspace, value, _| {
                                let live = live_mut(s);
                                live.app.sessions[index].options.effort = Some(value.into());
                                live.menu = None;
                            })
                            .build(k, &mut layers),
                    );
            }
        }
        let agents: Vec<_> = choices
            .agents
            .iter()
            .map(|agent| agent.id.as_str())
            .collect();
        if !agents.is_empty() {
            content = content
                .child(text("Agent").font(k.ty.small_medium).color(k.c.text_muted))
                .child(
                    Select::new("agent", &agents)
                        .current(session.options.agent.as_deref())
                        .placeholder("Provider default")
                        .label("Agent")
                        .appearance(SelectAppearance::Field)
                        .width(240.0)
                        .open(live.menu == Some("agent"))
                        .disabled(!session.can_send())
                        .on_toggle(|s: &mut Workspace| toggle_menu(s, "agent"))
                        .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                        .on_choose(move |s: &mut Workspace, value, _| {
                            let live = live_mut(s);
                            live.app.sessions[index].options.agent = Some(value.into());
                            live.menu = None;
                        })
                        .build(k, &mut layers),
                );
        }
    }
    content = content.child(
        Button::new("Refresh / reconnect session")
            .variant(ButtonVariant::Outline)
            .disabled(session.remote_id.is_none() || session.loading || session.submitting)
            .on_press(move |s: &mut Workspace| live_mut(s).app.refresh(index))
            .build(k)
            .key("refresh-history"),
    );
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(content)
        .children(layers)
}

fn toggle_menu(s: &mut Workspace, name: &'static str) {
    let menu = &mut live_mut(s).menu;
    *menu = if *menu == Some(name) {
        None
    } else {
        Some(name)
    };
}
