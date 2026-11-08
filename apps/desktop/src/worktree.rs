//! A creation form owns its checkout and provider across navigation and async reads.
use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    select::Select,
    ui,
};
use caelum_core::application::{Provider, Task, Workspace as Application};
use rune::{El, Key, TextEdit, col, row, text};
use rune_kit::{ButtonVariant, Kit, ScrollBar, SelectAppearance};

pub struct Form {
    project: String,
    provider: Provider,
    branch: TextEdit,
    base: String,
    search: TextEdit,
    menu: bool,
    initialized: bool,
    submitted: bool,
}

pub struct RenameForm {
    project: String,
    path: String,
    name: TextEdit,
}

impl Form {
    pub fn sync(&mut self, app: &Application) -> bool {
        if self.initialized {
            return false;
        }
        let Some(listing) = app
            .services
            .checkouts
            .get(&self.project)
            .and_then(|c| c.worktrees.as_ref())
        else {
            return false;
        };
        self.base.clone_from(&listing.default_base);
        self.branch
            .replace_text(caelum_core::worktrees::suggested_branch(listing));
        self.branch.set_selection(0, self.branch.text.len());
        self.branch.focused = true;
        self.initialized = true;
        true
    }
    fn task(&self, app: &Application) -> Task {
        Task::CreateWorktree {
            project: self.project.clone(),
            branch: self.branch.text.trim().into(),
            base: self.base.clone(),
            directory: app.preferences.worktree_directory.clone(),
        }
    }
}

pub fn open(s: &mut Workspace, project: String, provider: Provider) {
    let l = live_mut(s);
    let mut form = Form {
        project: project.clone(),
        provider,
        branch: TextEdit::new(""),
        base: String::new(),
        search: TextEdit::new(""),
        menu: false,
        initialized: false,
        submitted: false,
    };
    form.sync(&l.app);
    l.app.task(Task::Worktrees {
        project,
        directory: l.app.preferences.worktree_directory.clone(),
    });
    l.panels.worktree = Some(form);
    l.panels.overlay = Some(Screen::CreateWorktree);
}

pub fn rename_control(
    s: &Workspace,
    k: &Kit,
    project: String,
    path: &str,
    current_name: &str,
) -> El {
    let editing = live(s)
        .panels
        .rename_worktree
        .as_ref()
        .is_some_and(|form| form.path == path);
    if !editing {
        let path = path.to_owned();
        let current_name = current_name.to_owned();
        return ui::Button::new("Rename")
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                let mut name = TextEdit::new(&current_name);
                name.focused = true;
                live_mut(s).panels.rename_worktree = Some(RenameForm {
                    project: project.clone(),
                    path: path.clone(),
                    name,
                });
            })
            .build(k)
            .key("rename-worktree");
    }

    let form = live(s).panels.rename_worktree.as_ref().unwrap();
    let task = Task::RenameWorktree {
        project: form.project.clone(),
        path: form.path.clone(),
        name: form.name.text.trim().to_owned(),
        directory: live(s).app.preferences.worktree_directory.clone(),
    };
    let busy = live(s).app.services.busy(&task);
    row()
        .w_full()
        .gap(6.0)
        .center_y()
        .child(
            ui::field(&form.name, "Worktree name", "A short descriptive name", k)
                .key("rename-worktree-name")
                .grow(1.0)
                .on_edit(|s: &mut Workspace| {
                    &mut live_mut(s).panels.rename_worktree.as_mut().unwrap().name
                })
                .on_input_key(|s: &mut Workspace, ev, _| {
                    if ev.key != Key::Enter {
                        return false;
                    }
                    submit_rename(s);
                    true
                }),
        )
        .child(
            ui::Button::new(if busy { "Saving…" } else { "Save" })
                .variant(ButtonVariant::Contrast)
                .disabled(busy || task_name_empty(&task))
                .on_press(submit_rename)
                .build(k)
                .key("save-worktree-name"),
        )
        .child(
            ui::Button::new("Cancel")
                .variant(ButtonVariant::GhostMuted)
                .on_press(|s: &mut Workspace| live_mut(s).panels.rename_worktree = None)
                .build(k)
                .key("cancel-worktree-name"),
        )
}

fn task_name_empty(task: &Task) -> bool {
    matches!(task, Task::RenameWorktree { name, .. } if name.is_empty())
}

fn submit_rename(s: &mut Workspace) {
    let l = live_mut(s);
    let Some(form) = &l.panels.rename_worktree else {
        return;
    };
    let task = Task::RenameWorktree {
        project: form.project.clone(),
        path: form.path.clone(),
        name: form.name.text.trim().to_owned(),
        directory: l.app.preferences.worktree_directory.clone(),
    };
    if task_name_empty(&task) || l.app.services.busy(&task) {
        return;
    }
    l.app.task(task);
    l.panels.rename_worktree = None;
}

fn submit(s: &mut Workspace) {
    let l = live_mut(s);
    let Some(form) = &mut l.panels.worktree else {
        return;
    };
    let task = form.task(&l.app);
    if !form.initialized
        || form.branch.text.trim().is_empty()
        || l.app.services.busy(&task)
        || (form.submitted && l.app.services.error(&task).is_none())
    {
        return;
    }
    l.app.launch_worktree(task, form.provider);
    form.submitted = true;
}

pub fn view(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let l = live(s);
    let Some(form) = &l.panels.worktree else {
        return col();
    };
    let task = form.task(&l.app);
    let busy = l.app.services.busy(&task);
    let refresh = Task::Worktrees {
        project: form.project.clone(),
        directory: l.app.preferences.worktree_directory.clone(),
    };
    let listing = l
        .app
        .services
        .checkouts
        .get(&form.project)
        .and_then(|c| c.worktrees.as_ref());
    let mut fields = col().w_full().gap(10.0).child(ui::caption(
        format!(
            "{} · {}",
            form.project.rsplit('/').next().unwrap_or(&form.project),
            ui::provider_name(form.provider)
        ),
        k,
    ));
    if let Some(error) = l
        .app
        .services
        .error(&task)
        .or_else(|| l.app.services.error(&refresh))
    {
        fields = fields.child(ui::notice("Couldn't create worktree", error, k));
    }
    if let Some(listing) = listing {
        let query = form.search.text.to_lowercase();
        let mut branches = listing.branches.clone();
        if !branches.contains(&listing.default_base) {
            branches.insert(0, listing.default_base.clone());
        }
        branches.retain(|b| b.to_lowercase().contains(&query));
        let picker = Select::new("worktree-base", &branches)
            .label("Start from")
            .current(Some(&form.base))
            .appearance(SelectAppearance::Field)
            .width(360.0)
            .disabled(busy)
            .open(form.menu)
            .on_toggle(|s: &mut Workspace| {
                let f = live_mut(s).panels.worktree.as_mut().unwrap();
                f.menu = !f.menu;
                f.search.clear();
                f.search.focused = f.menu;
            })
            .on_close(|s: &mut Workspace| {
                let f = live_mut(s).panels.worktree.as_mut().unwrap();
                f.menu = false;
                f.search.focused = false;
            })
            .on_choose(|s: &mut Workspace, value, _| {
                let f = live_mut(s).panels.worktree.as_mut().unwrap();
                f.base = value.into();
                f.menu = false;
                f.search.focused = false;
            })
            .search(
                ui::field(&form.search, "Find base branch", "Search branches…", k)
                    .key("worktree-base-search")
                    .on_edit(|s: &mut Workspace| {
                        &mut live_mut(s).panels.worktree.as_mut().unwrap().search
                    }),
            )
            .build(k, layers)
            .w_full();
        fields = fields
            .child(ui::caption("New branch", k))
            .child(
                ui::field(&form.branch, "New branch", "work/my-change", k)
                    .key("worktree-branch")
                    .disabled(busy)
                    .on_edit(|s: &mut Workspace| {
                        &mut live_mut(s).panels.worktree.as_mut().unwrap().branch
                    })
                    .on_input_key(|s: &mut Workspace, ev, _| {
                        if ev.key != Key::Enter {
                            return false;
                        }
                        submit(s);
                        true
                    }),
            )
            .child(ui::caption("Start from", k))
            .child(picker)
            .child(ui::caption("Location", k))
            .child(
                text(
                    caelum_core::worktrees::destination_preview(listing, &form.branch.text)
                        .to_string_lossy(),
                )
                .font(k.ty.mono)
                .color(k.c.text_muted)
                .wrap()
                .selectable(true)
                .key("worktree-location"),
            );
    } else if l.app.services.busy(&refresh) {
        fields = fields.child(ui::skeleton("Loading branches", 3, k));
    } else {
        fields = fields
            .child(ui::caption(
                "Choose a Git repository with at least one commit.",
                k,
            ))
            .child(
                ui::Button::new("Reload branches")
                    .on_press(move |s: &mut Workspace| live_mut(s).app.task(refresh.clone()))
                    .build(k),
            );
    }
    if form.submitted && !busy && l.app.services.error(&task).is_none() {
        fields = fields.child(ui::caption(
            "Worktree created. It's available in the checkout chooser.",
            k,
        ));
    }
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(14.0)
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(rune::ScrollSpec {
                    stick_bottom: false,
                })
                .child(fields)
                .scroll_bar(k),
        )
        .child(ui::divider(k))
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(rune::el().grow(1.0))
                .child(
                    ui::Button::new("Cancel")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(|s: &mut Workspace| live_mut(s).panels.overlay = None)
                        .build(k),
                )
                .child(
                    ui::Button::new(if busy {
                        "Creating…"
                    } else {
                        "Create worktree"
                    })
                    .variant(ButtonVariant::Contrast)
                    .disabled(
                        busy || !form.initialized
                            || form.branch.text.trim().is_empty()
                            || (form.submitted && l.app.services.error(&task).is_none()),
                    )
                    .on_press(submit)
                    .build(k)
                    .key("create-worktree"),
                ),
        )
}
