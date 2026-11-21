//! One normal Caelum window and local draft store per remote checkout. Only desktop effects
//! stay local; an unavailable VM cannot fall through to a laptop provider, file or Git worker.
use crate::{
    Workspace,
    live::{Live, live, live_mut},
    ui,
};
use caelum_core::{
    application::{Command, Provider, Runtime, Store, Task, Workspace as Application},
    remote::{
        Job, Phase,
        native::{Client, NativeTarget, Notice, Status},
    },
};
use rune::{El, Waker, icon, row, text};
use rune_kit::{ButtonVariant, Kit};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::{Command as Process, Stdio},
};

pub(crate) struct Native {
    pub target: NativeTarget,
    pub status: Status,
    client: Option<Client>,
    reconcile: bool,
}

impl Native {
    /// Bind a fresh draft locally, even when the VM is offline. Restored native IDs and
    /// settings already belong to this account/machine/repository's private store.
    pub(crate) fn restore(&self, app: &mut Application) {
        let path = self.target.root().expect("validated native checkout");
        if !app.project_is_open(&path) {
            app.projects.push(caelum_protocol::ProjectInfo {
                path: path.clone(),
                name: self.target.repository.rsplit('/').next().unwrap().into(),
            });
        }
        if app.selected.is_none() {
            app.select_project(&path);
        }
    }

    pub(crate) fn new(target: NativeTarget) -> Self {
        Self {
            target,
            status: Status::Connecting,
            client: None,
            reconcile: false,
        }
    }
    pub(crate) fn start(&mut self, parent_store: &Path, wake: Waker) {
        match Client::start(self.target.clone(), parent_store.to_owned(), move || wake()) {
            Ok(client) => self.client = Some(client),
            Err(e) => self.status = Status::Disconnected(e),
        }
    }
}

pub(crate) fn ready(l: &Live) -> bool {
    l.native
        .as_ref()
        .is_none_or(|n| n.status == Status::Connected)
}

pub(super) fn scene(s: &mut Workspace, offline: bool) {
    let target = NativeTarget {
        machine: caelum_core::remote::MachineId {
            host: caelum_core::remote::Host::Boat,
            account: "example-account".into(),
            id: "vm-example".into(),
        },
        repository: "example/project".into(),
    };
    let handle = s.handle.clone();
    *s = workspace(target);
    s.handle = handle;
    let l = live_mut(s);
    l.fixture = true;
    let n = l.native.as_mut().unwrap();
    n.restore(&mut l.app);
    let i = l.app.selected.unwrap();
    let session = &mut l.app.sessions[i];
    session.remote_id = Some("ses_example".into());
    session.title = "Review the VM checkout".into();
    session.draft = "Check the tests for this change.".into();
    session.conversation.history(Provider::OpenCode,&[
        serde_json::json!({"type":"user","id":"user-example","text":"Which branch is checked out on the VM?"}),
        serde_json::json!({"type":"assistant","id":"assistant-example","finish":"stop","agent":"build","content":[{"type":"text","text":"The VM is on `feat/health-check`. The changes are in `src/server.rs`."}]}),
    ], false);
    n.status = if offline {
        l.app.native_connection_lost(&n.target.root().unwrap());
        Status::Disconnected("VM disconnected. Your draft is saved on this computer.".into())
    } else {
        Status::Connected
    };
    l.sync_editors();
}

pub(crate) fn workspace(target: NativeTarget) -> Workspace {
    let mut workspace = Workspace::live(None);
    let l = workspace.live.as_mut().unwrap();
    l.native = Some(Native::new(target));
    l.app.preferences.default_provider = Provider::OpenCode;
    workspace
}

pub(crate) fn submit(
    native: Option<&Native>,
    local: &mut Runtime,
    id: u64,
    cmd: Command,
) -> Result<(), String> {
    match native {
        Some(n)
            if !matches!(
                cmd,
                Command::Load(_)
                    | Command::Save(..)
                    | Command::Sound(_)
                    | Command::Task(Task::Updates)
            ) =>
        {
            n.client
                .as_ref()
                .ok_or("Native workspace is not connected.")?
                .submit(id, cmd)
        }
        _ => local.submit(id, cmd),
    }
}

pub(crate) fn poll(l: &mut Live) -> bool {
    let Some(n) = l.native.as_mut() else {
        return false;
    };
    let notices = n.client.as_mut().map(Client::drain).unwrap_or_default();
    let mut changed = false;
    for notice in notices {
        match notice {
            Notice::Event(e) => changed |= l.apply_events([*e]),
            Notice::Status(status) => {
                let n = l.native.as_mut().unwrap();
                changed |= status != n.status;
                n.reconcile = status == Status::Connected;
                if matches!(status, Status::Disconnected(_)) {
                    l.app.native_connection_lost(
                        &n.target.root().expect("validated native checkout"),
                    );
                }
                n.status = status;
            }
        }
    }
    let n = l.native.as_mut().unwrap();
    if l.app.restored() && std::mem::take(&mut n.reconcile) {
        let root = n.target.root().expect("validated native checkout");
        if !l.app.project_is_open(&root) {
            l.app.open_project(root);
        } else {
            l.app.discover_open_sessions();
            l.app.refresh_open_sessions();
            l.app.refresh_project();
            l.app.refresh_files();
        }
        changed = true;
    }
    changed
}

pub(crate) fn banner(s: &Workspace, k: &Kit) -> Option<El> {
    let n = live(s).native.as_ref()?;
    let (title, pending) = match &n.status {
        Status::Connecting => ("Connecting to VM…", false),
        Status::Connected => ("VM workspace", false),
        Status::Disconnected(reason) => (reason.as_str(), n.client.is_some() || live(s).fixture),
    };
    let label = format!("{} · {title}", n.target.repository);
    let detail = format!("Boat VM {} · {label}", n.target.machine.id);
    Some(
        row()
            .key("native-workspace-status")
            .role("status")
            .label(&detail)
            .help(&detail)
            .w_full()
            .min_h(30.)
            .px(12.)
            .gap(6.)
            .center_y()
            .child(icon(&ui::SERVER, 13.).color(k.c.icon_muted))
            .child(
                text(label)
                    .font(k.ty.small)
                    .color(k.c.text_muted)
                    .min_w(0.)
                    .grow(1.)
                    .wrap(),
            )
            .children(pending.then(|| {
                ui::Button::new("Reconnect")
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(|s: &mut Workspace| {
                        if let Some(client) =
                            live_mut(s).native.as_ref().and_then(|n| n.client.as_ref())
                        {
                            client.reconnect();
                        }
                    })
                    .build(k)
                    .key("native-workspace-reconnect")
            })),
    )
}

#[derive(Default)]
pub(crate) struct Windows {
    open: HashSet<String>,
    error: Option<(String, String)>,
}

pub(crate) fn target(l: &Live, id: &str) -> Option<NativeTarget> {
    let c = l.app.remote.saved.checkouts.get(id)?;
    let Phase::Ready { machine, .. } = &c.phase else {
        return None;
    };
    if l.native.is_some()
        || !matches!(machine.state.as_str(), "idle" | "running" | "ready")
        || matches!(
            l.app.remote.jobs.get(id),
            Some(Job::Busy(_) | Job::Deleting(_) | Job::Wait(_))
        )
    {
        return None;
    }
    let target = NativeTarget {
        machine: machine.key.clone(),
        repository: c.source.repository.clone(),
    };
    target.root().ok()?;
    Some(target)
}

pub(crate) fn button(s: &Workspace, k: &Kit, id: &str) -> El {
    let l = live(s);
    let open = l.remote.windows.open.contains(id);
    let key = id.to_owned();
    let help = l
        .remote
        .windows
        .error
        .as_ref()
        .filter(|(owner, _)| owner == id)
        .map_or(
            "Open the VM checkout in a native Caelum window over SSH.",
            |(_, e)| e.as_str(),
        );
    ui::Button::new(if open {
        "Workspace open"
    } else {
        "Open native workspace"
    })
    .variant(ButtonVariant::GhostMuted)
    .disabled(open || target(l, id).is_none())
    .on_press(move |s: &mut Workspace| launch(s, &key))
    .build(k)
    .key("open-native-vm")
    .help(help)
}

pub(crate) fn launch(s: &mut Workspace, id: &str) {
    if live(s).fixture || live(s).evaluation {
        return;
    }
    let Some(target) = target(live(s), id) else {
        return;
    };
    let Some(handle) = s.handle.clone() else {
        return;
    };
    let l = live_mut(s);
    if !l.remote.windows.open.insert(id.into()) {
        return;
    }
    l.remote.windows.error = None;
    let id = id.to_owned();
    std::thread::spawn(move || {
        let result = (|| -> std::io::Result<()> {
            let executable = if cfg!(target_os = "linux") {
                std::env::var_os("APPIMAGE").map(PathBuf::from)
            } else {
                None
            };
            let mut child = Process::new(executable.map(Ok).unwrap_or_else(std::env::current_exe)?)
                .arg("--boat-workspace")
                .arg(serde_json::to_string(&target)?)
                .stdin(Stdio::null())
                .spawn()?;
            if !child.wait()?.success() {
                return Err(std::io::Error::other(
                    "The native workspace exited unexpectedly.",
                ));
            }
            Ok(())
        })();
        handle.update(move |s, _| {
            let l = live_mut(s);
            l.remote.windows.open.remove(&id);
            if let Err(e) = result {
                l.remote.windows.error = Some((id, e.to_string()));
            }
        });
    });
}

pub(crate) fn store(native: Option<&Native>) -> Store {
    let local = Store::default_path();
    Store(native.map_or_else(|| local.clone(), |n| n.target.store_path(&local)))
}
