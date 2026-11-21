use super::{checkout, conversation};
use crate::{Workspace, live::live_mut, panels::Screen, settings::Page};
use caelum_core::remote::*;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) fn scene(s: &mut Workspace, name: &str) {
    if matches!(name, "native-connected" | "native-offline") {
        super::native::scene(s, name == "native-offline");
        return;
    }
    let l = live_mut(s);
    let source = Source {
        project: l.app.projects[0].path.clone(),
        account: "example-account".into(),
        repository: "example/project".into(),
        base: if name == "remote-handoff" {
            l.app
                .session_branch(&l.app.sessions[0])
                .expect("fixture session branch")
        } else {
            "main"
        }
        .into(),
        published_tip: (name == "remote-handoff").then(|| "a".repeat(40)),
        environment: (name != "remote-create").then(|| "project".into()),
    };
    let machine = Machine {
        key: MachineId {
            host: Host::Boat,
            account: source.account.clone(),
            id: "vm-example".into(),
        },
        name: "dev-01".into(),
        state: "idle".into(),
        auto_stop_unix: Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                + 42 * 60,
        ),
    };
    l.app.remote.inventory = Some(Inventory {
        account: source.account.clone(),
        machines: vec![machine.clone()],
    });
    if name == "remote-handoff" {
        use caelum_core::application::{Command, Event, HandoffSource, Reply};
        l.app.sessions[0].remote_id = Some("source-session".into());
        l.panels.overlay = Some(Screen::VmHandoff);
        l.app.begin_vm_handoff(HandoffSource::Composer(0));
        let (request, _) = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::HandoffChat(_)))
            .expect("fixture session");
        l.apply(Event::Completed(
            request,
            Ok(Reply::HandoffChat(Chat {
                messages: vec![
                    Message::User("Check the rollout plan.".into()),
                    Message::Assistant("The rollout is ready to review.".into()),
                ],
                clipped: false,
            })),
        ));
        l.app.remote.picker = Some(Picker::Ready(source));
        return;
    }
    if name == "remote-settings" {
        l.panels.settings_page = Page::Remote;
        l.panels.overlay = Some(Screen::Settings);
        return;
    }
    if matches!(name, "remote-picker" | "remote-create") {
        l.app.remote.picker = Some(Picker::Ready(source));
        l.panels.overlay = Some(if name == "remote-create" {
            Screen::CreateVm
        } else {
            Screen::RemotePicker
        });
        return;
    }
    let id = "checkout-example".to_owned();
    l.app.leave_conversation();
    l.app.active_project = None;
    l.app.remote.saved.selected = Some(id.clone());
    let mut c = Checkout {
        source,
        phase: Phase::Ready {
            machine: machine.clone(),
            branch: "feat/remote-checkout".into(),
        },
        pinned_stats: true,
        other_threads: vec![],
        thread: Thread {
            conversation: Some("thread-example".into()),
            last_activity: Some(l.feed.now - 120_000),
            lines: vec![
                Line::new(Speaker::You, "Add a health check and show me the changes."),
                Line::new(
                    Speaker::OpenCode,
                    "I added a health check to `src/server.rs`.",
                ),
            ],
            ..Default::default()
        },
    };
    match name {
        "remote-trace" => trace::setup(&mut c),
        "remote-model-draft" => {
            use caelum_core::application::Provider;
            use caelum_protocol::{ProviderChoice, ProviderModelChoice};
            c.thread = Thread {
                draft: "Review the changes on this branch.".into(),
                ..Default::default()
            };
            let models = [
                ("example/fast", "Example Fast"),
                ("example/code", "Example Code"),
                ("example/reasoning", "Example Reasoning"),
            ]
            .map(|(id, name)| ProviderModelChoice {
                id: id.into(),
                name: name.into(),
                provider: None,
                context_window: None,
                efforts: vec![],
                default_effort: None,
            });
            let reasoner = ProviderModelChoice {
                id: "example/reasoner".into(),
                name: "Example Reasoner".into(),
                provider: None,
                context_window: None,
                efforts: ["low", "medium", "high", "xhigh"]
                    .map(|id| ProviderChoice {
                        id: id.into(),
                        name: id.into(),
                    })
                    .into(),
                default_effort: Some("medium".into()),
            };
            l.app.remote.models = ModelCatalogue::Ready(
                [
                    (Provider::OpenCode, models.into()),
                    (Provider::Codex, [reasoner].into()),
                ]
                .into(),
            );
        }
        "remote-working" => {
            l.app.remote.feeds.insert(
                id.clone(),
                Job::Wait(Instant::now() + Duration::from_secs(60)),
            );
            c.thread.lines.extend([
                Line {
                    tool_detail: Some("src/server.rs".into()),
                    ..Line::new(Speaker::Tool(ToolState::Complete), "Edit")
                },
                Line {
                    tool_detail: Some("cargo".into()),
                    ..Line::new(Speaker::Tool(ToolState::Running), "Bash")
                },
            ]);
            c.thread.turn = Turn::Following(Run {
                provider: c.thread.provider,
                conversation: "thread-example".into(),
                prompt: "prompt-example".into(),
                id: "run-example".into(),
            });
        }
        "remote-creating" | "remote-setup-failed" => {
            c.phase = Phase::Create(Plan {
                token: id.clone(),
                branch: "feat/health".into(),
                size: Size::Small,
                lifetime: Lifetime::Hour,
            });
            c.thread = Thread::default();
            let job = if name == "remote-creating" {
                c.thread.continued = Some(Continuation {
                    id: "health-check-thread".into(),
                    title: "Add a health check".into(),
                    chat: Chat {
                        messages: vec![
                            Message::User("Where should the health check go?".into()),
                            Message::Assistant("Add a `/health` route in `src/server.rs`.".into()),
                        ],
                        clipped: false,
                    },
                    base: None,
                });
                c.thread.draft = "Implement it on this branch.".into();
                Job::Busy(1)
            } else {
                Job::Failed("Create checkout environment: Boat returned HTTP 403. Automatic setup needs environment.write.".into())
            };
            l.app.remote.jobs.insert(id.clone(), job);
        }
        "remote-stopped" => {
            let mut stopped = machine.clone();
            stopped.state = "archived".into();
            c.phase = Phase::Prepare {
                machine: stopped,
                new_branch: None,
            };
        }
        _ => {}
    }
    l.app.remote.saved.checkouts.insert(id.clone(), c);
    if !matches!(
        name,
        "remote-creating" | "remote-setup-failed" | "remote-stopped"
    ) {
        l.app.remote.monitor = Monitor::Showing {
            owner: id.clone(),
            sample: Sample {
                machine,
                vcpu: Some(4),
                memory_gb: Some(8),
                activity: Activity::Live(Some(Process {
                    name: "cargo".into(),
                    cpu_percent: 82.,
                    memory_mb: Some(610.),
                })),
            },
            next: Instant::now() + Duration::from_secs(60),
        };
    }
    l.remote.restored = true;
    conversation::sync_editor(l);
    checkout::enter(s);
    let l = live_mut(s);
    if name == "remote-trace" {
        trace::open(l, &id);
    }
    if name == "remote-machine" {
        super::popup::inspect_remote(l, &id);
    }
    if name == "remote-changes" {
        l.app.remote.changes.insert(id,Changes::Ready(Review{revision:Revision{branch:"feat/remote-checkout".into(),head:"a".repeat(40),index:"b".repeat(40),tree:"c".repeat(40)},
            status:"## feat/remote-checkout\n M src/server.rs".into(),patch:"diff --git a/src/server.rs b/src/server.rs\n--- a/src/server.rs\n+++ b/src/server.rs\n@@ -1,3 +1,4 @@\n fn routes() {\n+    add_health_check();\n     start_server();\n }\n".into()}));
        super::changes::sync(l);
        s.inspector.set_open(true);
    }
}
mod trace;
