#![forbid(unsafe_code)]

use std::{path::PathBuf, sync::Arc};

use caelum_core::{
    approvals::ApprovalBroker, codex::CodexProvider, opencode::OpenCodeProvider,
    processes::ProcessRegistry, projects::Project,
};
use caelum_protocol::{
    AppInfo, ApprovalDecision, ApprovalPrompt, CodexSessionInfo, CodexSessionList,
    OpenCodeConversationEntry, OpenCodeSessionInfo, OpenCodeSessionList, ProjectInfo,
};
use tauri::{Emitter, Manager, State};

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Caelum".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[tauri::command]
fn open_project(path: String) -> Result<ProjectInfo, String> {
    Project::open(PathBuf::from(path))
        .map(|project| project.info())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn pending_approvals(approvals: State<'_, ApprovalBroker>) -> Vec<ApprovalPrompt> {
    approvals.pending()
}

#[tauri::command]
fn resolve_approval(
    approvals: State<'_, ApprovalBroker>,
    provider: String,
    request_id: String,
    decision: ApprovalDecision,
) -> Result<(), String> {
    approvals
        .resolve(&provider, &request_id, decision)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn codex_list_sessions(
    codex: State<'_, CodexProvider>,
    project_path: String,
) -> Result<CodexSessionList, String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .list_sessions(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn codex_start_session(
    codex: State<'_, CodexProvider>,
    project_path: String,
) -> Result<CodexSessionInfo, String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .start_session(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn codex_resume_session(
    codex: State<'_, CodexProvider>,
    project_path: String,
    thread_id: String,
) -> Result<CodexSessionInfo, String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .resume_session(project_path, &thread_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn codex_send_prompt(
    codex: State<'_, CodexProvider>,
    project_path: String,
    thread_id: String,
    prompt: String,
) -> Result<(), String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .send_prompt(project_path, &thread_id, &prompt)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn codex_cancel_turn(
    codex: State<'_, CodexProvider>,
    project_path: String,
    thread_id: String,
) -> Result<(), String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .cancel_turn(project_path, &thread_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn opencode_list_sessions(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<OpenCodeSessionList, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .list_sessions(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn opencode_start_session(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<OpenCodeSessionInfo, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .start_session(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn opencode_resume_session(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
) -> Result<OpenCodeSessionInfo, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .resume_session(project_path, &session_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn opencode_session_history(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
) -> Result<Vec<OpenCodeConversationEntry>, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .session_history(project_path, &session_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn opencode_send_prompt(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
    prompt: String,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .send_prompt(project_path, &session_id, &prompt)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn opencode_cancel_turn(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .cancel_turn(project_path, &session_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn run() {
    let app = tauri::Builder::default()
        .manage(ApprovalBroker::default())
        .manage(Arc::new(ProcessRegistry::default()))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let approvals = app.state::<ApprovalBroker>().inner().clone();
            let processes = app.state::<Arc<ProcessRegistry>>().inner().clone();
            app.manage(CodexProvider::new(processes, approvals));

            let approvals = app.state::<ApprovalBroker>().inner().clone();
            let processes = app.state::<Arc<ProcessRegistry>>().inner().clone();
            app.manage(OpenCodeProvider::new(processes, approvals));

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
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(event) = codex_updates.recv() {
                    if handle.emit("codex-event", event).is_err() {
                        break;
                    }
                }
            });

            let opencode_updates = app.state::<OpenCodeProvider>().subscribe();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(event) = opencode_updates.recv() {
                    if handle.emit("opencode-event", event).is_err() {
                        break;
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            open_project,
            pending_approvals,
            resolve_approval,
            codex_list_sessions,
            codex_start_session,
            codex_resume_session,
            codex_send_prompt,
            codex_cancel_turn,
            opencode_list_sessions,
            opencode_start_session,
            opencode_resume_session,
            opencode_session_history,
            opencode_send_prompt,
            opencode_cancel_turn
        ])
        .build(tauri::generate_context!())
        .expect("error while running Caelum desktop application");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            handle.state::<ApprovalBroker>().shutdown();
            handle.state::<CodexProvider>().shutdown();
            handle.state::<OpenCodeProvider>().shutdown();
            handle.state::<Arc<ProcessRegistry>>().shutdown();
        }
    });
}
