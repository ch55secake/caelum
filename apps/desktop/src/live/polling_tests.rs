use super::*;
use caelum_core::application::{Attachment, Command, Provider, Reply};
use serde_json::json;
use std::{hint::black_box, time::Instant};

fn settle_store(live: &mut Live) {
    for (id, command) in live.app.commands() {
        if matches!(command, Command::Save(..)) {
            live.app.apply(Event::Completed(id, Ok(Reply::Done)));
        }
    }
}

/// UI-thread polling only: no renderer, providers, workers or filesystem writes.
#[test]
#[ignore = "opt-in release CPU profile; run alone"]
fn idle_polling_profile() {
    for (label, tools, populated, streaming) in [
        ("idle", false, false, false),
        ("tools", true, false, false),
        ("background-stream", true, false, true),
        ("populated", true, true, false),
    ] {
        let mut state = crate::evaluation::workspace();
        let l = state.live.as_mut().unwrap();
        l.evaluation = false;
        l.fixture = true;
        if tools {
            // Eight mounted rows at the end of a 2,000-entry history: four
            // patch inputs (100 changed lines) and four 64 KiB shell outputs.
            let c = &mut l.app.sessions[0].conversation;
            let seed = c.entries.last().unwrap().clone();
            for i in 0..8 {
                let mut entry = seed.clone();
                entry.id = format!("profile-tool-{i}");
                entry.kind = caelum_core::application::EntryKind::Tool;
                entry.text.clear();
                entry.name = if i % 2 == 0 { "apply_patch" } else { "shell" }.into();
                entry.input = if i % 2 == 0 {
                    json!({"path":format!("src/file-{i}.rs"),"patch":format!(
                        "@@ -1,100 +1,100 @@\n{}", "-old();\n+new();\n".repeat(100)
                    )})
                    .to_string()
                } else {
                    json!({"command":"cargo test","cwd":"/evaluation"}).to_string()
                };
                entry.output = "test output row\n".repeat(4096);
                l.visible_entries.insert((0, entry.id.clone()));
                c.entries.push(entry);
            }
        }
        if populated {
            for _ in l.app.sessions.len()..120 {
                l.app.new_session(Scope {
                    project: "/evaluation".into(),
                    provider: Provider::Codex,
                });
            }
            for session in &mut l.app.sessions {
                session.draft = "draft context\n".repeat(512);
                session.attachments.push(Attachment::new(
                    "src/context.rs".into(),
                    "context\n".repeat(8192),
                    None,
                    false,
                ));
            }
        }
        l.app.select(0);
        l.sync_editors();
        l.app
            .enable_store("/evaluation/unused-profile-state.json".into());
        for (id, command) in l.app.commands() {
            if matches!(command, Command::Load(_)) {
                l.app.apply(Event::Completed(id, Ok(Reply::Snapshot(None))));
            }
        }
        for _ in 0..20 {
            l.poll(&mut Tx::default());
            settle_store(l);
        }
        let mut polls = Vec::new();
        let mut persistence = Vec::new();
        for _ in 0..360 {
            if streaming {
                // Changes to background transcript output are not persisted.
                for session in &mut l.app.sessions[1..6] {
                    session
                        .conversation
                        .entries
                        .last_mut()
                        .unwrap()
                        .text
                        .push_str(" delta");
                }
            }
            let start = Instant::now();
            black_box(l.poll(&mut Tx::default()));
            polls.push(start.elapsed().as_secs_f64() * 1_000.0);
            let start = Instant::now();
            let commands = black_box(l.app.commands());
            persistence.push(start.elapsed().as_secs_f64() * 1_000.0);
            assert!(commands.is_empty());
        }
        let stats = |mut samples: Vec<f64>| {
            samples.sort_by(f64::total_cmp);
            json!({"p50_ms":samples[180],"p95_ms":samples[342],"max_ms":samples[359]})
        };
        eprintln!(
            "{}",
            json!({"workload":label,"poll":stats(polls),"persistence":stats(persistence)})
        );
        // Prevent Live::drop from flushing the synthetic store to disk.
        // All outstanding commands above were handled synchronously in memory.
        l.app = Application::default();
        l.drafts.clear();
    }
}
