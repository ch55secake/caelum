use caelum_core::approvals::ApprovalBroker;
use caelum_core::opencode::OpenCodeProvider;
use caelum_core::questions::{Answer, QuestionBroker};
use caelum_protocol::{ApprovalDecision, ApprovalPrompt, QuestionPrompt};
use tauri::State;

#[tauri::command]
pub(crate) fn pending_approvals(approvals: State<'_, ApprovalBroker>) -> Vec<ApprovalPrompt> {
    approvals.pending()
}

#[tauri::command]
pub(crate) fn pending_questions(questions: State<'_, QuestionBroker>) -> Vec<QuestionPrompt> {
    questions.pending()
}

#[tauri::command]
pub(crate) async fn refresh_questions(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .refresh_questions(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn resolve_question(
    questions: State<'_, QuestionBroker>,
    provider: String,
    project_path: String,
    session_id: String,
    request_id: String,
    answer: Option<Answer>,
) -> Result<(), String> {
    let questions = questions.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        questions.resolve(&provider, &project_path, &session_id, &request_id, answer)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn resolve_approval(
    approvals: State<'_, ApprovalBroker>,
    provider: String,
    project_path: String,
    request_id: String,
    decision: ApprovalDecision,
) -> Result<(), String> {
    approvals
        .resolve(&provider, &project_path, &request_id, decision)
        .map_err(|error| error.to_string())
}
