use caelum_core::approvals::ApprovalBroker;
use caelum_core::codex::CodexProvider;
use caelum_core::opencode::OpenCodeProvider;
use caelum_core::questions::QuestionBroker;
use caelum_core::session_branches::SessionBranches;
use tauri::{Emitter, Manager};

pub(super) fn forward(app: &tauri::App) {
    let updates = app.state::<QuestionBroker>().subscribe();
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        while let Ok(prompts) = updates.recv() {
            if handle.emit("question-requests", prompts).is_err() {
                break;
            }
        }
    });

    let updates = app.state::<ApprovalBroker>().subscribe();
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        while let Ok(prompts) = updates.recv() {
            if handle.emit("approval-requests", prompts).is_err() {
                break;
            }
        }
    });

    let codex_updates = app.state::<CodexProvider>().subscribe();
    let branches = app.state::<SessionBranches>().inner().clone();
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        while let Ok(event) = codex_updates.recv() {
            if let Some(update) = branches.codex_event(&event) {
                let _ = handle.emit("session-branch-updated", update);
            }
            if handle.emit("codex-event", event).is_err() {
                break;
            }
        }
    });

    let opencode_updates = app.state::<OpenCodeProvider>().subscribe();
    let branches = app.state::<SessionBranches>().inner().clone();
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        while let Ok(event) = opencode_updates.recv() {
            if let Some(update) = branches.opencode_event(&event) {
                let _ = handle.emit("session-branch-updated", update);
            }
            if handle.emit("opencode-event", event).is_err() {
                break;
            }
        }
    });
}
