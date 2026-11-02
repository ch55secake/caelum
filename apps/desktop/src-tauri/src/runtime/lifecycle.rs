use super::persistence::{PersistenceState, StartupProject};
use caelum_core::{
    approvals::ApprovalBroker, codex::CodexProvider, opencode::OpenCodeProvider,
    processes::ProcessRegistry, projects::Project, questions::QuestionBroker,
    session_branches::SessionBranches, worktrees::WorktreeClient,
};
use std::{path::PathBuf, sync::Arc};
use tauri::Manager;

pub(super) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let approvals = ApprovalBroker::default();
    let questions = QuestionBroker::default();
    let processes = Arc::new(ProcessRegistry::default());
    app.manage(CodexProvider::new(
        processes.clone(),
        approvals.clone(),
        questions.clone(),
    ));
    app.manage(OpenCodeProvider::new(
        processes.clone(),
        approvals.clone(),
        questions.clone(),
    ));
    app.manage(approvals);
    app.manage(questions);
    app.manage(processes);
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
    handle.state::<ApprovalBroker>().shutdown();
    handle.state::<CodexProvider>().shutdown();
    handle.state::<OpenCodeProvider>().shutdown();
    handle.state::<Arc<ProcessRegistry>>().shutdown();
}
