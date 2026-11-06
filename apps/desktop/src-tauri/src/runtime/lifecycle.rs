use super::persistence::{PersistenceState, StartupProject};
use caelum_core::{
    application::ProviderRuntime, projects::Project, session_branches::SessionBranches,
    worktrees::WorktreeClient,
};
use std::path::PathBuf;
use tauri::Manager;

pub(super) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = ProviderRuntime::default();
    app.manage(runtime.codex.clone());
    app.manage(runtime.opencode.clone());
    app.manage(runtime.approvals.clone());
    app.manage(runtime.questions.clone());
    app.manage(runtime.processes.clone());
    app.manage(runtime);
    app.manage(WorktreeClient::new(app.path().home_dir()?));
    app.manage(PersistenceState::open(app.path().app_data_dir()));
    app.manage(SessionBranches::new(
        app.state::<PersistenceState>().database().ok(),
    ));
    let startup = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_dir())
        .and_then(|path| Project::open(path).ok().map(|project| project.info()));
    app.manage(StartupProject(startup));
    super::events::forward(app);
    Ok(())
}

pub(super) fn shutdown(handle: &tauri::AppHandle) {
    handle.state::<ProviderRuntime>().shutdown();
}
