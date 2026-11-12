mod managed;
mod registered;
mod smoke;
mod titles;

use super::*;
use super::{
    permissions::permission_reply,
    server::{discover_service, SERVER_PASSWORD_ENV},
    transport::{decode_json_response, parse_event, read_sse_frames},
};
use caelum_protocol::{ApprovalDecision, LineChanges, PermissionRequest};
use reqwest::StatusCode;
use std::{
    env, fs,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Instant,
};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "caelum-opencode-test-{}-{}",
            std::process::id(),
            NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn sse_reader_handles_comments_multiline_data_and_final_unterminated_frame() {
    let input = b": keepalive\nevent: ignored\ndata: {\ndata: \"type\":\"session.text.delta\"}\n\ndata: final";
    let mut reader = BufReader::new(&input[..]);
    let mut frames = Vec::new();

    read_sse_frames(&mut reader, || true, |frame| frames.push(frame)).unwrap();

    assert_eq!(frames, ["{\n\"type\":\"session.text.delta\"}", "final"]);
}

#[test]
fn session_ids_cannot_escape_the_session_route() {
    assert!(valid_session_id("ses_e7b5213dbffeaVPrGggxUYkwrK"));
    assert!(!valid_session_id("ses/other"));
    assert!(!valid_session_id("ses"));
}

#[test]
fn event_parser_maps_open_code_sse_payload_to_the_desktop_contract() {
    let event = parse_event(
        r#"{"id":"evt_1","created":123,"type":"session.text.delta","location":{"directory":"/workspace"},"data":{"sessionID":"ses_1","delta":"hi"}}"#,
        Path::new("/workspace"),
    )
    .unwrap();

    assert_eq!(event.event_type, "session.text.delta");
    assert_eq!(event.id.as_deref(), Some("evt_1"));
    assert_eq!(event.created, Some(123));
    assert_eq!(event.project_path, "/workspace");
    assert_eq!(event.data["sessionID"], "ses_1");
    assert_eq!(event.location.as_ref().unwrap()["directory"], "/workspace");
}

#[test]
fn unauthorized_responses_have_actionable_setup_guidance() {
    let error = decode_json_response(StatusCode::UNAUTHORIZED, b"{}")
        .unwrap_err()
        .to_string();
    assert!(error.contains(SERVER_PASSWORD_ENV));
}

#[test]
fn permission_replies_never_grant_persistent_access() {
    assert_eq!(permission_reply(ApprovalDecision::AllowOnce), "once");
    assert_eq!(permission_reply(ApprovalDecision::Deny), "reject");
}

#[test]
fn server_disconnect_denies_only_its_opencode_approval_requests() {
    let approvals = ApprovalBroker::default();
    let disconnected_waiter = approvals
        .request(PermissionRequest {
            provider: PROVIDER_NAME.to_owned(),
            project_path: "/workspace/one".to_owned(),
            session_id: Some("ses_1".to_owned()),
            request_id: "ses_1:per_1".to_owned(),
            summary: "Run a tool".to_owned(),
            description: "OpenCode requested permission.".to_owned(),
            provider_details: json!({ "projectPath": "/workspace/one" }),
        })
        .unwrap();
    let other_project_waiter = approvals
        .request(PermissionRequest {
            provider: PROVIDER_NAME.to_owned(),
            project_path: "/workspace/two".to_owned(),
            session_id: Some("ses_2".to_owned()),
            request_id: "ses_2:per_2".to_owned(),
            summary: "Run another tool".to_owned(),
            description: "Another OpenCode project requested permission.".to_owned(),
            provider_details: json!({ "projectPath": "/workspace/two" }),
        })
        .unwrap();
    let other_waiter = approvals
        .request(PermissionRequest {
            provider: "other".to_owned(),
            project_path: "/workspace/other".to_owned(),
            session_id: None,
            request_id: "request-1".to_owned(),
            summary: "Run another tool".to_owned(),
            description: "Another provider requested permission.".to_owned(),
            provider_details: Value::Null,
        })
        .unwrap();

    let pending_ids = HashSet::from(["ses_1:per_1".to_owned()]);
    deny_pending_approvals(&approvals, Some("/workspace/one"), Some(&pending_ids));

    assert_eq!(
        disconnected_waiter.wait(Duration::from_millis(10)),
        ApprovalDecision::Deny
    );
    assert_eq!(approvals.pending().len(), 2);
    approvals
        .resolve(
            PROVIDER_NAME,
            "/workspace/two",
            "ses_2:per_2",
            ApprovalDecision::AllowOnce,
        )
        .unwrap();
    assert_eq!(
        other_project_waiter.wait(Duration::from_millis(10)),
        ApprovalDecision::AllowOnce
    );
    approvals
        .resolve(
            "other",
            "/workspace/other",
            "request-1",
            ApprovalDecision::AllowOnce,
        )
        .unwrap();
    assert_eq!(
        other_waiter.wait(Duration::from_millis(10)),
        ApprovalDecision::AllowOnce
    );
}

#[cfg(unix)]
#[test]
fn shutdown_terminates_and_unregisters_the_owned_server_process() {
    let processes = Arc::new(ProcessRegistry::default());
    let approvals = ApprovalBroker::default();
    let provider =
        OpenCodeProvider::new(Arc::clone(&processes), approvals, QuestionBroker::default());
    let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
    let process_id = processes.register(child).unwrap();
    let client = Client::builder().build().unwrap();
    let project_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = Arc::new(OpenCodeServer {
        process_id: Some(process_id),
        base_url: "http://127.0.0.1:1".to_owned(),
        client: client.clone(),
        event_client: client,
        password: Some("unused-test-password".to_owned()),
        alive: Arc::new(AtomicBool::new(true)),
        session_ids: Mutex::new(HashSet::new()),
        pending_permission_ids: Arc::new(Mutex::new(HashSet::new())),
    });
    provider
        .inner
        .lock_servers()
        .insert(project_path, Arc::clone(&server));

    provider.shutdown();

    assert!(!server.alive.load(Ordering::Acquire));
    assert!(matches!(
        processes.terminate(process_id),
        Err(crate::processes::ProcessError::UnknownProcess)
    ));
}

#[test]
fn reserve_local_port_returns_a_loopback_port_that_can_be_rebound() {
    let port = reserve_local_port().unwrap();
    assert_ne!(port, 0);
}
