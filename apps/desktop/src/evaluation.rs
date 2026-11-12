//! Deterministic demanding workload, explicitly opt-in and isolated from user data.
mod feed;
mod review;
mod scenes;

use crate::Workspace;
use caelum_core::application::{Event, Provider, Scope};
pub(super) use scenes::SCENES;
use serde_json::json;

pub fn workspace() -> Workspace {
    let mut state = Workspace::live(None);
    let live = state.live.as_mut().unwrap();
    live.evaluation = true;
    live.app.projects.push(caelum_protocol::ProjectInfo {
        name: "Evaluation workload".into(),
        path: "/evaluation".into(),
    });
    for i in 0..6 {
        let index = live.app.new_session(Scope {
            project: "/evaluation".into(),
            provider: Provider::Codex,
        });
        let session = &mut live.app.sessions[index];
        session.title = format!("Concurrent session {}", i + 1);
        session.remote_id = Some(format!("thread-{i}"));
        let turns: Vec<_> = (0..1_000).map(|n| json!({"id":format!("turn-{n}"),"status":"completed","items":[
            {"id":format!("user-{n}"),"type":"userMessage","content":[{"text":format!("Review change {n}, keeping the draft and reading position intact.")}]},
            {"id":format!("answer-{n}"),"type":"agentMessage","text":format!("## Change {n}\n\nThe implementation preserves **session ownership** while background streams continue.\n\n```rust\nlet owner = session.key();\n```\n\n- Check stale replies\n- Keep drafts independent")}
        ]})).collect();
        session.conversation.history(Provider::Codex, &turns, false);
    }
    let url = "https://github.com/example/evaluation/pull/1";
    let review = live
        .app
        .services
        .reviews
        .entry(("/evaluation".into(), url.into()))
        .or_default();
    review.details = Some(serde_json::from_value(json!({"number":1,"title":"Large review workload","url":url,"isDraft":true,"body":"Deterministic, offline review workload.","state":"OPEN","author":null,"baseRefName":"main","headRefName":"evaluation","headRefOid":"a".repeat(40),"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","additions":10000,"deletions":10000,"changedFiles":300,"reviewDecision":null,"statusCheckRollup":[]})).unwrap());
    review.files_loaded = true;
    review.files = (0..300)
        .map(|i| caelum_protocol::GitHubPullRequestFile {
            path: format!("src/file-{i}.rs"),
            status: "modified".into(),
            previous_path: None,
            additions: 100,
            deletions: 100,
            patch: Some(format!(
                "@@ -1,100 +1,100 @@\n{}",
                (0..100)
                    .map(|n| format!("-let value_{n} = {n};\n+let value_{n} = {};\n", n + 1))
                    .collect::<String>()
            )),
        })
        .collect();
    let checkout = live
        .app
        .services
        .checkouts
        .entry("/evaluation".into())
        .or_default();
    checkout
        .pull_requests
        .push(review.details.as_ref().unwrap().pull_request.clone());
    live.app.select(0);
    state
}

pub fn tick(app: &mut caelum_core::application::Workspace) {
    for i in 0..6 {
        app.apply(Event::Codex(caelum_protocol::CodexEvent { project_path: "/evaluation".into(), method: "item/agentMessage/delta".into(), params: json!({"threadId":format!("thread-{i}"),"itemId":"active-answer","delta":"Streaming another small update. "}), request_id: None }));
    }
}

fn polish_scene(state: &mut Workspace, name: &str) -> bool {
    let Some(l) = state.live.as_mut().filter(|l| l.fixture) else {
        return false;
    };
    let project = "/projects/caelum";
    let activity = name.strip_prefix("vercel-").unwrap_or(name);
    if matches!(
        activity,
        "tool-running"
            | "tool-failure"
            | "tool-output-long"
            | "tool-search"
            | "compaction"
            | "compaction-running"
            | "compaction-empty"
            | "compaction-error"
            | "compaction-interrupted"
    ) {
        if name.starts_with("vercel-") {
            l.app.preferences.theme_family = "vercel".into();
            l.app.preferences.accent_color = "theme".into();
        }
        let conversation = &mut l.app.sessions[0].conversation;
        *conversation = Default::default();
        conversation.history(Provider::OpenCode, &[json!({"id":"prompt","type":"user","text":"Refine the tool calls and keep the conversation easy to follow."})], false);
        if activity.starts_with("tool-") {
            let failed = activity == "tool-failure";
            let running = activity == "tool-running";
            let output = match activity {
                "tool-output-long" => format!(
                    "Running workspace tests…\n{}\ntest result: ok. 320 passed; 0 failed\n",
                    (0..320).map(|i| format!("test application::session_{i:03}::restores_café_draft ... ok\n")).collect::<String>()
                ),
                "tool-failure" => "Compiling caelum-desktop…\nRunning workspace tests…\n\ntest application::session::restores_draft ... FAILED\n\nthread 'restores_draft' panicked at src/session.rs:42:9:\nassertion `left == right` failed\n  left: \"\"\n right: \"Keep café ☁️ safe\"\n\ntest result: FAILED. 319 passed; 1 failed".into(),
                "tool-search" => "Found 3 matches\n\nsrc/workspace.rs:42:    session.restore_draft();\nsrc/session.rs:128:    pub fn restore_draft(&mut self) {\ntests/session.rs:86:    fn restores_draft_after_switching_sessions() {".into(),
                _ => "Compiling caelum-desktop…\nRunning workspace tests…".into(),
            };
            let search = activity == "tool-search";
            conversation.history(Provider::OpenCode, &[json!({"id":"work","type":"assistant","content":[
                {"id":"run","type":"tool","name":if search {"grep"} else {"bash"},"state":{"status":if failed {"error"} else if running {"running"} else {"completed"},"input":if search {json!({"pattern":"restore_draft"})} else {json!({"command":"cargo test --workspace","timeout":120000,"description":"Run the workspace tests"})},"metadata":{"durationMs":2500,"exit":if failed {Some(101)} else {None}},"error":if failed {Some("A workspace test failed. Open the output for details.")} else {None},"content":[{"type":"text","text":output}]}}
            ]})], false);
            if failed {
                // Include captured stdout alongside the provider's final error.
                conversation
                    .entries
                    .iter_mut()
                    .find(|e| e.id == "opencode-tool-run")
                    .unwrap()
                    .output = output;
            }
            conversation.working = running;
            if !failed && !running {
                l.expanded.insert((0, "opencode-tool-run".into()));
            }
        } else {
            let status = match activity {
                "compaction-running" => "running",
                "compaction-error" => "failed",
                "compaction-interrupted" => "interrupted",
                _ => "completed",
            };
            let summary = if activity == "compaction" {
                "## Working context\n\nRefine the native workspace while preserving **session ownership**.\n\n- Tool calls use compact headers and output-first details.\n- Successful commands show a tick.\n- Keep drafts and reading position when switching sessions.\n\n### Next step\n\nReview `apps/desktop/src/activity.rs` and capture the light and dark layouts."
            } else {
                ""
            };
            conversation.history(Provider::OpenCode, &[json!({"id":"compact","type":"compaction","summary":summary,"status":status,"error":if status == "failed" {Some("The provider could not summarise the conversation. Your messages are still available.")} else {None}})], false);
            conversation.working = status == "running";
            if status == "completed" {
                conversation.history(Provider::OpenCode, &[json!({"id":"after","type":"assistant","content":[{"type":"text","text":"The working context is ready. I’ll continue with the tool-call layout."}],"finish":"stop"})], false);
            }
        }
        return true;
    }
    let patch_scene = name.strip_prefix("vercel-").unwrap_or(name);
    if matches!(patch_scene, "patch-short" | "patch-long" | "patch-multi") {
        if name.starts_with("vercel-") {
            l.app.preferences.theme_family = "vercel".into();
            l.app.preferences.accent_color = "theme".into();
        }
        let patch = match patch_scene {
            "patch-long" => format!(
                "--- a/src/session.rs\n+++ b/src/session.rs\n@@ -40,24 +40,24 @@\n{}",
                (0..24).map(|i| format!("-    restore_slot({i});\n+    restore_owned_slot(session, {i});\n")).collect::<String>()
            ),
            "patch-multi" => "--- a/src/session.rs\n+++ b/src/session.rs\n@@ -1 +1 @@\n-restore();\n+restore_owned_draft();\n--- a/src/composer.rs\n+++ b/src/composer.rs\n@@ -1 +1 @@\n-const HEIGHT: f32 = 56.0;\n+const MAX_HEIGHT: f32 = 180.0;\n--- a/src/theme.rs\n+++ b/src/theme.rs\n@@ -1 +1 @@\n-let surface = kit.c.layer_01;\n+let surface = kit.c.bg_base;\n".into(),
            _ => "@@ -1 +1 @@\n-restore();\n+restore_owned_draft();\n".into(),
        };
        let session = &mut l.app.sessions[0];
        session.conversation = Default::default();
        session.conversation.history(Provider::OpenCode, &[
            json!({"id":"prompt","type":"user","text":"Keep the draft and reading position when switching sessions."}),
            json!({"id":"work","type":"assistant","content":[
                {"id":"patch","type":"tool","name":"apply_patch","state":{"status":"completed","input":{"path":"src/session.rs","patch":patch},"content":[{"type":"text","text":"Updated the session workspace."}],"time":{"start":1000,"end":1200}}}
            ]}),
        ], false);
        return true;
    }
    if matches!(
        name,
        "sidebar-worktrees"
            | "sidebar-worktrees-many"
            | "sidebar-worktrees-projects"
            | "sidebar-worktrees-vercel"
    ) {
        if name == "sidebar-worktrees-vercel" {
            l.app.preferences.theme_family = "vercel".into();
            l.app.preferences.accent_color = "theme".into();
        }
        let mut trees = vec![
            json!({"path":project,"project":{"path":project,"name":"caelum"},"branch":"main","main":true,"locked":null,"prunable":null,"available":true}),
            json!({"path":"/projects/caelum-native","project":{"path":"/projects/caelum-native","name":"caelum-native"},"branch":"feat/native-workspace","main":false,"locked":null,"prunable":null,"available":true}),
            json!({"path":"/projects/caelum-providers","project":{"path":"/projects/caelum-providers","name":"caelum-providers"},"branch":"fix/provider-reconnect","main":false,"locked":null,"prunable":null,"available":true}),
            json!({"path":"/scratch/external-review","project":{"path":"/scratch/external-review","name":"external-review"},"branch":"tool/review","main":false,"locked":null,"prunable":null,"available":true}),
        ];
        if name == "sidebar-worktrees-many" {
            for i in 0..12 {
                let path = format!("/projects/caelum-task-{i}");
                trees.push(json!({"path":path,"project":{"path":path,"name":format!("task-{i}")},"branch":format!("work/task-{i}"),"main":false,"locked":if i == 10 { Some("Release checkout") } else { None },"prunable":null,"available":i != 11}));
            }
        }
        seed_worktrees(l, serde_json::from_value(json!({
            "project":{"path":project,"name":"caelum"},"storageDirectory":"/projects","branches":["main","unused-branch"],"defaultBase":"main","worktrees":trees,
        })).unwrap());
        for (path, title, provider) in [
            (
                "/projects/caelum-native",
                "Restore the worktree sidebar",
                Provider::Codex,
            ),
            (
                "/projects/caelum-providers",
                "Reconnect provider sessions",
                Provider::OpenCode,
            ),
        ] {
            let index = l.app.new_session(Scope {
                project: path.into(),
                provider,
            });
            let session = &mut l.app.sessions[index];
            session.title = title.into();
            session.draft = format!("Independent draft for {title}");
            session.loaded = true;
            if provider == Provider::Codex {
                session.conversation.working = true;
            } else {
                session.queue.push(caelum_core::application::QueuedPrompt {
                    text: "Review the reconnect changes".into(),
                    options: Default::default(),
                });
                session.queue_paused = true;
            }
        }
        if matches!(
            name,
            "sidebar-worktrees-projects" | "sidebar-worktrees-vercel"
        ) {
            l.app.projects.push(caelum_protocol::ProjectInfo {
                path: "/projects/rune".into(),
                name: "rune".into(),
            });
            seed_worktrees(l, serde_json::from_value(json!({
                "project":{"path":"/projects/rune","name":"rune"},"storageDirectory":"/worktrees/rune","branches":["main","fix/key-navigation"],"defaultBase":"main",
                "worktrees":[
                    {"path":"/projects/rune","project":{"path":"/projects/rune","name":"rune"},"branch":"main","main":true,"locked":null,"prunable":null,"available":true},
                    {"path":"/worktrees/rune/key-navigation","project":{"path":"/worktrees/rune/key-navigation","name":"rune"},"branch":"fix/key-navigation","main":false,"locked":null,"prunable":null,"available":true}
                ]
            })).unwrap());
            l.app
                .preferences
                .worktree_expanded
                .insert("/projects/rune".into(), true);
        }
        l.app.select(0);
        finish_fixture_reads(l);
        return true;
    }
    if name == "sidebar-projects" {
        for (path, name) in [
            ("/projects/rune", "rune"),
            ("/projects/website", "website"),
            ("/projects/tools", "developer-tools"),
        ] {
            l.app.projects.push(caelum_protocol::ProjectInfo {
                path: path.into(),
                name: name.into(),
            });
            let index = l.app.new_session(Scope {
                project: path.into(),
                provider: Provider::Codex,
            });
            l.app.sessions[index].draft = format!("Independent draft for {name}");
            l.app.services.checkouts.entry(path.into()).or_default().git = Some(
                serde_json::from_value(json!({
                    "projectPath":path,"repositoryRoot":path,"branch":"main","changes":[]
                }))
                .unwrap(),
            );
        }
        for n in 0..6 {
            let index = l.app.new_session(Scope {
                project: project.into(),
                provider: Provider::Codex,
            });
            l.app.sessions[index].title = format!("Workspace task {}", n + 1);
        }
        l.app
            .services
            .checkouts
            .get_mut(project)
            .unwrap()
            .pull_requests = [
            "Give projects and reviews room to breathe",
            "Restore independent session drafts",
            "Improve native keyboard navigation",
            "Polish the compact review workspace",
            "Keep provider requests scoped",
            "Refresh the desktop documentation",
        ]
        .iter()
        .enumerate()
        .map(|(i, title)| caelum_protocol::GitHubPullRequest {
            number: 101 + i as u64,
            title: (*title).into(),
            url: format!("https://github.com/ch55secake/caelum/pull/{}", 101 + i),
            is_draft: false,
            base_ref_name: "main".into(),
        })
        .collect();
        l.app.select(0);
        return true;
    }
    if matches!(name, "vercel" | "vercel-settings") {
        l.app.preferences.theme_family = "vercel".into();
        l.app.preferences.accent_color = "theme".into();
        if name == "vercel-settings" {
            crate::panels::show(state, crate::panels::Screen::Settings);
        }
        return true;
    }
    let about = name.strip_prefix("vercel-").unwrap_or(name);
    if matches!(
        about,
        "about"
            | "about-checking"
            | "about-current"
            | "about-update"
            | "about-release"
            | "about-ahead"
            | "about-error"
    ) {
        if name.starts_with("vercel-") {
            l.app.preferences.theme_family = "vercel".into();
            l.app.preferences.accent_color = "theme".into();
        }
        l.panels.settings_page = crate::settings::Page::About;
        if about != "about" {
            use caelum_protocol::{UpdateCheck, UpdateStatus};
            let (status, latest) = match about {
                "about-current" => (UpdateStatus::UpToDate, env!("CARGO_PKG_VERSION")),
                "about-ahead" => (UpdateStatus::Ahead, "0.1.0"),
                _ => (UpdateStatus::Available, "0.2.0"),
            };
            l.app.services.update = Some(UpdateCheck {
                current_version: env!("CARGO_PKG_VERSION").into(),
                latest_version: latest.into(),
                status,
                release_url: format!("{}/tag/v{latest}", caelum_core::updates::RELEASES_URL),
                download_url: (about != "about-release").then(|| {
                    format!(
                        "{}/download/v{latest}/Caelum-v{latest}-aarch64.dmg",
                        caelum_core::updates::RELEASES_URL
                    )
                }),
            });
        }
        if matches!(about, "about-checking" | "about-error") {
            l.app.task(caelum_core::application::Task::Updates);
            if about == "about-error" {
                for (id, command) in l.app.commands() {
                    if matches!(
                        command,
                        caelum_core::application::Command::Task(
                            caelum_core::application::Task::Updates
                        )
                    ) {
                        l.app.apply(Event::Completed(
                            id,
                            Err(
                                "Could not reach GitHub. Check your connection and try again."
                                    .into(),
                            ),
                        ));
                    }
                }
            }
        }
        crate::panels::show(state, crate::panels::Screen::Settings);
        return true;
    }
    if matches!(
        name,
        "providers" | "provider-error" | "notifications" | "storage"
    ) {
        l.panels.settings_page = match name {
            "providers" | "provider-error" => crate::settings::Page::Providers,
            "notifications" => crate::settings::Page::Notifications,
            "storage" => crate::settings::Page::Worktrees,
            _ => unreachable!(),
        };
        if name == "provider-error" {
            l.app.notices.insert(
                l.app.sessions[0].scope.clone(),
                "Codex CLI was not found. Install Codex CLI, then refresh provider status.".into(),
            );
        }
        crate::panels::show(state, crate::panels::Screen::Settings);
        return true;
    }
    if matches!(name, "models" | "models-empty") {
        l.app.sessions[0].options.model = Some("provider/model-000".into());
        let scope = l.app.sessions[0].scope.clone();
        l.app.choices.insert(scope, serde_json::from_value(json!({"models": (0..160).map(|i| json!({"id":format!("provider/model-{i:03}"),"name":format!("Model {i:03}"),"contextWindow":200000,"efforts":[]})).collect::<Vec<_>>(),"agents":[],"defaultModel":"provider/model-000"})).unwrap());
        crate::models::open(state, None);
        if name == "models-empty" {
            state
                .live
                .as_mut()
                .unwrap()
                .models
                .query
                .replace_text("no matching models");
        }
        return true;
    }
    if name == "launcher" {
        crate::panels::open_launcher(state);
        return true;
    }
    if name == "session" {
        crate::panels::show(state, crate::panels::Screen::Session);
        return true;
    }
    if matches!(
        name,
        "worktrees"
            | "worktrees-loading"
            | "worktrees-error"
            | "worktrees-empty"
            | "worktree-create"
    ) {
        crate::panels::show(state, crate::panels::Screen::Worktrees);
        let l = state.live.as_mut().unwrap();
        if name == "worktrees-loading" {
            return true;
        }
        let mut listing: caelum_protocol::GitWorktrees = serde_json::from_value(json!({
            "project":{"path":project,"name":"Caelum"},"storageDirectory":"/Users/oscar/.caelum/worktrees/caelum-7d0420ab","branches":["main","feat/native"],"defaultBase":"main",
            "worktrees":[
                {"path":project,"project":{"path":project,"name":"Caelum"},"branch":"main","main":true,"locked":null,"prunable":null,"available":true},
                {"path":"/Users/oscar/.caelum/worktrees/caelum-7d0420ab/feat-native","project":{"path":"/Users/oscar/.caelum/worktrees/caelum-7d0420ab/feat-native","name":"Caelum"},"branch":"feat/native","main":false,"locked":null,"prunable":null,"available":true}
            ]
        })).unwrap();
        if name == "worktrees-empty" {
            listing.worktrees.retain(|tree| tree.main);
        }
        seed_worktrees(l, listing);
        finish_fixture_reads(l);
        if name == "worktrees-error" {
            l.app.services.checkouts.get_mut(project).unwrap().error = Some(
                "This checkout has uncommitted changes. Commit or move them before removing it."
                    .into(),
            );
        }
        if name == "worktree-create" {
            crate::worktree::open(state, project.into(), Provider::OpenCode);
        }
        return true;
    }
    if matches!(
        name,
        "timeline"
            | "timeline-empty"
            | "usage"
            | "usage-loading"
            | "stats"
            | "stats-loading"
            | "stats-error"
            | "stats-empty"
    ) {
        l.commands.owner = Some(0);
        l.panels.overlay = Some(match name {
            "timeline" | "timeline-empty" => crate::panels::Screen::Timeline,
            "usage" | "usage-loading" => crate::panels::Screen::Usage,
            _ => crate::panels::Screen::Stats,
        });
        if name == "timeline" {
            let session = &mut l.app.sessions[0];
            session.conversation = Default::default();
            let prompts = [
                "Restore the continuous PR workspace",
                "Keep drafts independent when switching sessions",
                "Make the model picker open above the composer",
                "Review the patch and explain the tradeoffs",
                "Run the workspace checks and address the failures",
                "Polish keyboard focus and compact layouts",
            ];
            let turns: Vec<_> = (0..48).map(|n| json!({"id":format!("turn-{n}"),"createdAt":caelum_core::application::presentation::now().saturating_sub((48-n) * 3_600_000),"status":"completed","items":[
                {"id":format!("prompt-{n}"),"type":"userMessage","content":[{"text":format!("{}\nPlease preserve reading position and check the populated, empty and loading states.", prompts[n as usize % prompts.len()])}]},
                {"id":format!("answer-{n}"),"type":"agentMessage","text":"The changes keep session ownership intact. Ready for review."}]})).collect();
            session.conversation.history(Provider::Codex, &turns, false);
            l.current_prompt = Some("codex-prompt-0".into());
            l.commands.timeline_selected = l.current_prompt.clone();
            l.commands.timeline_current = l.current_prompt.clone();
        }
        if name == "timeline-empty" {
            l.app.sessions[0].conversation = Default::default();
        }
        if name == "usage" {
            l.app.services.checkouts.get_mut(project).unwrap().usage = Some(
                json!({"workspace":{"name":"Personal workspace"},"go":{"rolling":{"percent":24},"weekly":{"percent":68},"monthly":{"percent":12}}}),
            );
        }
        if name == "usage-loading" {
            l.app.task(caelum_core::application::Task::Usage {
                project: project.into(),
            });
        }
        if matches!(name, "stats" | "stats-empty") {
            let tokens = json!({"input":420000,"output":120000,"reasoning":80000,"cache":{"read":920000,"write":12000}});
            l.app.services.checkouts.get_mut(project).unwrap().stats = Some(serde_json::from_value(json!({
                "range":{"from":0,"to":1800000000000_u64},"sessions":24,"subagents":8,"prompts":126,"steps":372,
                "tokens":tokens,
                "cost":14.28,"activeDays":12,"streak":4,
                "activity":(1..=30).map(|i| json!({"date":format!("2026-10-{i:02}"),"steps":if i%6==0 {0} else {(i*17)%35+2}})).collect::<Vec<_>>(),
                "models":[{"model":{"providerID":"openai","id":"gpt-5.3-codex"},"steps":242,"tokens":tokens,"cost":10.14},
                    {"model":{"providerID":"anthropic","id":"claude-sonnet-4-6"},"steps":130,"tokens":tokens,"cost":4.14}]
            })).unwrap());
            let checkout = l.app.services.checkouts.get_mut(project).unwrap();
            checkout.stats_projects = vec![
                caelum_protocol::OpenCodeStatsProject {
                    id: "caelum".into(),
                    name: Some("Caelum".into()),
                    canonical: project.into(),
                },
                caelum_protocol::OpenCodeStatsProject {
                    id: "website".into(),
                    name: Some("Website".into()),
                    canonical: "/projects/website".into(),
                },
            ];
            if name == "stats-empty" {
                let stats = checkout.stats.as_mut().unwrap();
                stats.sessions = 0;
                stats.prompts = 0;
                stats.cost = 0.0;
                stats.activity.clear();
                stats.models.clear();
                stats.steps = 0;
                stats.active_days = 0;
                stats.streak = 0;
                stats.tokens = serde_json::from_value(
                    json!({"input":0,"output":0,"reasoning":0,"cache":{"read":0,"write":0}}),
                )
                .unwrap();
            }
        }
        if matches!(name, "stats-loading" | "stats-error") {
            l.app.task(caelum_core::application::Task::Stats {
                project: project.into(),
                filter: Default::default(),
            });
            if name == "stats-error" {
                for (id, _) in l.app.commands() {
                    l.app.apply(Event::Completed(
                        id,
                        Err("Statistics service is unavailable. Try again in a moment.".into()),
                    ));
                }
            }
        }
        return true;
    }
    if matches!(
        name,
        "composer-options"
            | "composer-empty"
            | "composer-multiline"
            | "composer-queued"
            | "composer-queue-error"
            | "composer-attachments"
            | "composer-long"
            | "composer-attachment-status"
            | "composer-attachment-preview"
            | "composer-image-preview"
    ) {
        let session = &mut l.app.sessions[0];
        session.scope.provider = Provider::OpenCode;
        session.options.model = Some("model-long".into());
        session.options.effort = Some("High".into());
        session.options.agent = Some("build".into());
        l.app.choices.insert(session.scope.clone(), serde_json::from_value(json!({
            "models":[{"id":"model-long","name":"A model with a particularly long name","efforts":[{"id":"High","name":"High"},{"id":"Low","name":"Low"}],"defaultEffort":"High"}],
            "agents":[{"id":"build","name":"Build"},{"id":"plan","name":"Plan"}],"defaultModel":"model-long"
        })).unwrap());
        state.inspector.set_open(name == "composer-options");
        if name != "composer-options" {
            session.options.model = Some("gpt-5.1".into());
            let choices = l.app.choices.get_mut(&session.scope).unwrap();
            choices.models[0].id = "gpt-5.1".into();
            choices.models[0].name = "GPT-5.1".into();
            choices.default_model = Some("gpt-5.1".into());
            let prompt = match name {
                "composer-empty" => "",
                "composer-multiline" => {
                    "Refine the composer’s surface and spacing.\nKeep the conversation above it.\n\nMake Send, Queue and Stop easy to tell apart.\nKeep this draft while I switch sessions."
                }
                "composer-attachments" => "Review these files and the design reference.",
                _ => "Then check the compact layout in both themes.",
            };
            session.draft = prompt.into();
            if matches!(name, "composer-queued" | "composer-queue-error") {
                session.conversation.working = true;
                session.queue = [
                    "Review keyboard focus and session switching.",
                    "Check attachments and multiline drafts.",
                ]
                .map(|text| caelum_core::application::QueuedPrompt {
                    text: text.into(),
                    options: session.options.clone(),
                })
                .into();
                if name == "composer-queue-error" {
                    session.conversation.working = false;
                    session.queue_paused = true;
                    session.error =
                        Some("The provider is unavailable. Resume when it reconnects.".into());
                }
            }
            if matches!(
                name,
                "composer-attachments"
                    | "composer-long"
                    | "composer-attachment-status"
                    | "composer-attachment-preview"
                    | "composer-image-preview"
            ) {
                session
                    .attachments
                    .push(caelum_core::application::Attachment::new(
                        "apps/desktop/src/composer.rs".into(),
                        "pub fn composer(session: &Session) -> View {\n    // Keep the attached snapshot independent of later edits.\n    let draft = &session.draft;\n    render_editor(draft)\n}\n".into(),
                        Some((40, 44)),
                        name != "composer-attachments",
                    ));
                session.attachments.push(
                    caelum_core::application::Attachment::image(
                        "Design reference.png".into(),
                        include_bytes!("../assets/icon.png"),
                    )
                    .unwrap(),
                );
                session.attachments[0].id = "source".into();
                session.attachments[1].id = "image".into();
            }
            l.sync_editors();
            l.drafts[0].replace_text(prompt);
            if name == "composer-long" {
                l.drafts[0].replace_text([
                    "Refine the long-prompt writing experience.\n\nKeep the conversation visible above the docked editor.\nPreserve the caret, selected text and undo history while resizing.\n",
                    "\nAttachment review\nPreview the captured file and image before sending.\nKeep removal separate from opening a preview.\nShow partial snapshots and per-file failures clearly.\n",
                    "\nSession ownership\nRemember each session’s draft and scroll position.\nReturn to this draft after inspecting the context.\n\nCheck the compact layout in both themes.\n",
                ].concat());
            }
            if name == "composer-attachment-status" {
                l.app.attach_file(0, "docs/design/notes.md".into());
                l.app.sessions[0].attachment_error(
                    "assets/recording.mov".into(),
                    "This file is not readable as text or a supported image.".into(),
                );
            }
            if matches!(
                name,
                "composer-attachment-preview" | "composer-image-preview"
            ) {
                crate::composer::open(
                    state,
                    0,
                    if name == "composer-image-preview" {
                        "image"
                    } else {
                        "source"
                    },
                );
            }
        }
        return true;
    }
    if matches!(
        name,
        "files" | "files-tabs" | "files-loading" | "changes-empty"
    ) {
        let checkout = l.app.services.checkouts.get_mut(project).unwrap();
        if name == "changes-empty" {
            checkout.git.as_mut().unwrap().changes.clear();
            checkout.diffs.clear();
            state.inspector.set_open(true);
            return true;
        }
        if matches!(name, "files" | "files-tabs") {
            checkout.directories.insert(String::new(), serde_json::from_value(json!({
                "path":"","entries":[{"name":"src","path":"src","kind":"directory","sizeBytes":null},{"name":"Cargo.toml","path":"Cargo.toml","kind":"file","sizeBytes":245}],"total":2,"nextOffset":null
            })).unwrap());
            checkout.files.insert("Cargo.toml".into(), caelum_protocol::ProjectFileContent {
                path:"Cargo.toml".into(),content:"[package]\nname = \"caelum-desktop\"\nversion = \"0.1.13\"\nedition = \"2024\"\n\n[dependencies]\ncaelum-core = { path = \"../../crates/caelum-core\" }\n".into(),size_bytes:245,truncated:false
            });
            checkout.tabs = vec!["Cargo.toml".into()];
            checkout.selected_file = Some("Cargo.toml".into());
            if name == "files-tabs" {
                for path in [
                    "src/application/independent_session_workspace.rs",
                    "README.md",
                ] {
                    checkout.files.insert(path.into(), caelum_protocol::ProjectFileContent {
                        path: path.into(), content: "// Each checkout retains its own draft.\nlet owner = session.key();\n".into(), size_bytes: 70, truncated: false,
                    });
                    checkout.tabs.push(path.into());
                }
            }
        }
        l.app.services.revision += 1;
        crate::panels::show(state, crate::panels::Screen::Files);
        if matches!(name, "files" | "files-tabs") {
            finish_fixture_reads(state.live.as_mut().unwrap());
        }
        return true;
    }
    if matches!(name, "approval" | "questions") {
        l.app.sessions[0].remote_id = Some("thread-fixture".into());
        l.app.sessions[0].conversation = Default::default();
        if name == "approval" {
            l.app
                .apply(Event::Approvals(vec![caelum_protocol::ApprovalPrompt {
                    provider: "codex".into(),
                    project_path: project.into(),
                    session_id: Some("thread-fixture".into()),
                    request_id: "fixture".into(),
                    summary: "Run the workspace checks?".into(),
                    description: "cargo test --workspace --locked".into(),
                }]));
        } else {
            l.app.apply(Event::Questions(vec![caelum_protocol::QuestionPrompt {provider:"codex".into(),project_path:project.into(),session_id:"thread-fixture".into(),request_id:"fixture".into(),title:"Choose the implementation scope".into(),fields:vec![json!({"key":"target","type":"multiselect","title":"Platforms","required":true,"options":[{"value":"macos","label":"macOS","description":"Native desktop experience"},{"value":"linux","label":"Linux","description":"Shared core and CLI"}],"default":["macos"],"custom":true})] }]));
        }
        return true;
    }
    if name == "commands" {
        l.sync_editors();
        l.drafts[0].replace_text("/");
        l.drafts[0].set_caret(1);
        l.drafts[0].focused = true;
        return true;
    }
    if matches!(name, "working" | "thinking" | "retrying" | "stopping") {
        l.app.sessions[0].remote_id = Some("fixture".into());
        l.app.sessions[0].conversation.working = true;
        if name == "thinking" {
            l.app.sessions[0].conversation.apply(Provider::Codex, "item/reasoning/summaryTextDelta", &json!({"itemId":"thinking","delta":"Checking the original interactions before changing the layout."}));
        }
        l.app.sessions[0].conversation.retrying = name == "retrying";
        l.app.sessions[0].stopping = name == "stopping";
        return true;
    }
    if matches!(
        name,
        "shells" | "shell-output" | "shells-many" | "shells-collapsed"
    ) {
        let session = &mut l.app.sessions[0];
        session.scope.provider = Provider::OpenCode;
        session.remote_id = Some("ses_fixture".into());
        session.shells = vec![caelum_protocol::OpenCodeBackgroundShell {
            id: "shell_fixture".into(),
            command: "cargo test --workspace".into(),
            started_at: caelum_core::application::presentation::now() - 12_000,
        }];
        l.app
            .services
            .checkouts
            .get_mut(project)
            .unwrap()
            .shell_output
            .insert(
                ("ses_fixture".into(), "shell_fixture".into()),
                format!(
                    "{}\nRunning 162 tests…\ntest application::queue_recovery ... ok\n{}",
                    "Compiling workspace…\n".repeat(180),
                    "test scoped_worktree_handoff_with_independent_sessions ... ok ".repeat(12)
                ),
            );
        if matches!(name, "shells-many" | "shells-collapsed") {
            l.app.sessions[0].shells.extend((1..8).map(|i| {
                caelum_protocol::OpenCodeBackgroundShell {
                    id: format!("shell-{i}"),
                    command: format!("cargo test --package provider-{i}"),
                    started_at: caelum_core::application::presentation::now() - 6000,
                }
            }));
        }
        if name == "shells-collapsed" {
            l.collapsed_shells.insert(0);
        }
        if name == "shell-output" {
            crate::shells::open(state, 0, "shell_fixture".into());
        }
        return true;
    }
    if name == "archive" {
        l.archive(2, true);
        return true;
    }
    if name == "messages" {
        let attachment = caelum_core::application::Attachment::image(
            "Caelum.png".into(),
            include_bytes!("../assets/icon.png"),
        )
        .unwrap();
        let files = json!({"version":1,"files":[
            {"id":"session-source","path":"src/session.rs","content":"let owner = session.key();","startLine":12,"endLine":12,"truncated":false},
            {"id":"session-fixture","path":"tests/fixtures/independent-session-drafts/input.json","content":"{}","truncated":true}
        ]});
        let context = format!("\n\n<caelum-file-context>\n{files}\n</caelum-file-context>");
        let session = &mut l.app.sessions[0];
        session.conversation = Default::default();
        session.conversation.history(Provider::OpenCode, &[
            json!({"id":"short", "type":"user", "text":"Looks good — go ahead."}),
            json!({"id":"reply", "type":"assistant", "content":[{"id":"text", "type":"text", "text":"I'll keep the draft and reading position intact."}]}),
            json!({"id":"formatted", "type":"user", "text":"Please keep the **session owner** captured.\n\n- Preserve the independent draft\n- Restore the exact reading position\n\n```rust\nlet owner = session.key();\n```"}),
            json!({"id":"attachment", "type":"user", "text":context, "files":[{"mime":"image/png", "uri":attachment.content, "name":"Caelum.png"}]})
        ], false);
        return true;
    }
    if matches!(name, "empty" | "error") {
        l.app.sessions[0].conversation = Default::default();
        l.app.sessions[0].draft.clear();
        l.sync_editors();
        l.drafts[0].clear();
        if name == "error" {
            l.app.notices.insert(
                l.app.sessions[0].scope.clone(),
                "Codex CLI was not found. Install Codex CLI, then refresh provider status.".into(),
            );
        }
        return true;
    }
    false
}

// Populated scenes seed their read results above. Release the corresponding
// loading slots too, so refresh/retry controls reflect a settled response.
fn finish_fixture_reads(live: &mut crate::live::Live) {
    use caelum_core::application::{Command, Reply, TaskReply};
    for (id, command) in live.app.commands() {
        if matches!(command, Command::Task(_)) {
            live.app
                .apply(Event::Completed(id, Ok(Reply::Task(TaskReply::Done))));
        }
    }
}

fn seed_worktrees(live: &mut crate::live::Live, listing: caelum_protocol::GitWorktrees) {
    use caelum_core::application::{Command, Reply, Task, TaskReply};
    live.app.task(Task::Worktrees {
        project: listing.project.path.clone(),
        directory: String::new(),
    });
    for (id, command) in live.app.commands() {
        if matches!(command, Command::Task(Task::Worktrees { .. })) {
            live.apply(Event::Completed(
                id,
                Ok(Reply::Task(TaskReply::Worktrees(Some(listing.clone())))),
            ));
        } else if matches!(command, Command::Task(_)) {
            live.app
                .apply(Event::Completed(id, Ok(Reply::Task(TaskReply::Done))));
        }
    }
}

/// Reference content through the actual live view, with no workers or disk writes.
pub fn design() -> Workspace {
    let mut state = Workspace::live(None);
    let live = state.live.as_mut().unwrap();
    live.fixture = true;
    let project = "/projects/caelum";
    let sample = &state.sample;
    live.app.projects.push(caelum_protocol::ProjectInfo {
        name: "caelum".into(),
        path: project.into(),
    });
    let scope = Scope {
        project: project.into(),
        provider: Provider::Codex,
    };
    for title in [&sample.title, "OpenCode session", &sample.secondary_title] {
        let i = live.app.new_session(scope.clone());
        live.app.sessions[i].title = title.into();
        live.app.sessions[i].last_activity = Some(live.feed.now - (i as u64 + 1) * 900_000);
    }
    let session = &mut live.app.sessions[0];
    session.loaded = true;
    session.options.model = Some("Test model".into());
    session.options.effort = Some("Medium".into());
    session.draft = sample.draft.clone();
    session.conversation.history(Provider::Codex, &[json!({"status":"completed", "items":[
        {"id":"prompt", "type":"userMessage", "content":[{"text":sample.prompt}]},
        {"id":"tool", "type":"commandExecution", "command":sample.command, "aggregatedOutput":sample.output, "status":"completed"},
        {"id":"answer", "type":"agentMessage", "text":sample.response}
    ]})], false);
    live.app.choices.insert(scope, serde_json::from_value(json!({"models":[{"id":"Test model","name":"Test model","efforts":[{"id":"Medium","name":"Medium"}],"defaultEffort":"Medium"}],"agents":[],"defaultModel":"Test model"})).unwrap());
    let checkout = live
        .app
        .services
        .checkouts
        .entry(project.into())
        .or_default();
    checkout.git = Some(serde_json::from_value(json!({"projectPath":project,"repositoryRoot":project,"branch":"main","changes":[{"path":"src/workspace.css","indexStatus":" ","worktreeStatus":"M"}]})).unwrap());
    checkout.diffs.insert(
        "src/workspace.css".into(),
        serde_json::from_value(
            json!({"path":"src/workspace.css","staged":"","unstaged":sample.patch,"untracked":""}),
        )
        .unwrap(),
    );
    live.app.select(0);
    crate::panels::select_change(&mut state, project.into(), "src/workspace.css".into());
    state.live.as_mut().unwrap().panels.inspector = crate::panels::Screen::Changes;
    finish_fixture_reads(state.live.as_mut().unwrap());
    state
}

/// Populated regression scenes share the production renderer and use no services.
pub fn scene(state: &mut Workspace, name: &str) {
    if feed::scene(state, name) {
        return;
    }
    if polish_scene(state, name) {
        return;
    }
    let Some(l) = state.live.as_mut().filter(|l| l.fixture) else {
        return;
    };
    let project = "/projects/caelum";
    if name == "crowded" {
        for n in 0..30 {
            let i = l.app.new_session(Scope {
                project: project.into(),
                provider: Provider::Codex,
            });
            l.app.sessions[i].title = format!("Workspace task {}", n + 1);
        }
        l.app.archive(3, true);
        l.app.select(0);
        l.app.sessions[0].draft =
            "Preserve this multiline draft.\nKeep model controls below the editor.\n".repeat(12);
        l.sync_editors();
        l.drafts[0].replace_text(&l.app.sessions[0].draft);
        return;
    }
    if matches!(name, "tools" | "tools-collapsed" | "vercel-tools") {
        if name == "vercel-tools" {
            l.app.preferences.theme_family = "vercel".into();
            l.app.preferences.accent_color = "theme".into();
        }
        let session = &mut l.app.sessions[0];
        session.conversation = Default::default();
        session.conversation.history(Provider::OpenCode,&[json!({"id":"prompt","type":"user","text":"Restore native workspace parity and verify the interactions."}),json!({"id":"work","type":"assistant","content":[
            {"id":"think","type":"reasoning","text":"Check the compact layout, preserve ownership, then verify the patch."},
            {"id":"read","type":"tool","name":"read","state":{"status":"completed","input":{"filePath":"src/workspace.rs"},"content":[{"type":"text","text":"pub fn restore_draft(session: &mut Session) {\n    session.editor.focus();\n}"}],"time":{"start":1000,"end":1250}}},
            {"id":"run","type":"tool","name":"bash","state":{"status":"completed","input":{"command":"cargo test --workspace","timeout":120000},"metadata":{"exit":0},"content":[{"type":"text","text":"test result: ok. 190 passed; 0 failed"}],"time":{"start":2000,"end":4520}}},
            {"id":"patch","type":"tool","name":"apply_patch","state":{"status":"completed","input":{"path":"src/workspace.rs","patch":"@@ -1 +1 @@\n-restore();\n+restore_owned_draft();"},"content":[{"type":"text","text":"Updated src/workspace.rs"}],"time":{"start":4600,"end":4800}}}
        ]})],false);
        for entry in &session.conversation.entries {
            if name != "tools-collapsed" && entry.kind != caelum_core::application::EntryKind::User
            {
                l.expanded.insert((0, entry.id.clone()));
            }
        }
        return;
    }
    if matches!(name, "images" | "image-preview") {
        let attachment = caelum_core::application::Attachment::image(
            "Caelum.png".into(),
            include_bytes!("../assets/icon.png"),
        )
        .unwrap();
        l.app.attach(0, attachment.clone());
        l.app.sessions[0].conversation.history(Provider::OpenCode,&[json!({"id":"image","type":"user","text":"Use this icon in the native app.","files":[{"mime":"image/png","uri":attachment.content,"name":"Caelum.png"}]})],false);
        if name == "image-preview" {
            let media = l.app.sessions[0]
                .conversation
                .entries
                .iter()
                .find_map(|e| e.media.first())
                .unwrap()
                .clone();
            l.media.selected = Some((project.into(), media));
            l.panels.overlay = Some(crate::panels::Screen::Image);
        }
        return;
    }
    review::scene(state, name);
}
