use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rambledesk_core::{
    ActionInput, ApproveFeedbackInput, RequestAttachmentInput, RequestFeedbackInput,
    SaveDraftInput, SubmitFeedbackInput, TerminalOperation, TerminalOperationEvent,
    TerminalOperationObserver, WorkbenchTerminalOperations,
};
use rambledesk_local_server::{AccessToken, ServerConfig, start_server};

mod application_api_support;
use application_api_support::start_application_server;

const TEST_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REQUEST_ATTACHMENT_BYTES: &[u8] = b"# Request context\n\nInspect the transport contract.";

const APPLICATION_OPERATIONS: [&str; 23] = [
    "listFeedbackInbox",
    "listHostSessions",
    "listArchivedHostSessions",
    "listHostProfiles",
    "listFeedbackRequests",
    "getFeedbackWorkspace",
    "readPublishedFeedback",
    "saveFeedbackDraft",
    "addFeedbackAttachment",
    "removeFeedbackAttachment",
    "reorderFeedbackAttachments",
    "submitFeedback",
    "approveFeedbackRequest",
    "cancelFeedbackRequest",
    "renameHostSession",
    "setHostSessionPinned",
    "archiveHostSession",
    "unarchiveHostSession",
    "deleteHostSession",
    "deleteFeedbackRequest",
    "setHostPinned",
    "readFeedbackAttachment",
    "readRequestAttachment",
];

async fn test_application()
-> anyhow::Result<(rambledesk_core::FeedbackApplication, tempfile::TempDir)> {
    let directory = tempfile::tempdir()?;
    let store = rambledesk_storage::SqliteFeedbackStore::connect(
        &directory.path().join("rambledesk.sqlite3"),
    )
    .await?;
    Ok((store.into_application(), directory))
}

fn application_url(address: std::net::SocketAddr, operation: &str) -> String {
    format!("http://{address}/api/application/{operation}")
}

fn assert_no_path_keys(value: &serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => values.iter().for_each(assert_no_path_keys),
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                assert!(!key.contains("path"), "storage path key leaked: {key}");
                assert_no_path_keys(value);
            }
        }
        _ => {}
    }
}

fn terminal_operations(
    application: &rambledesk_core::FeedbackApplication,
) -> WorkbenchTerminalOperations {
    WorkbenchTerminalOperations::without_observer(application.clone())
}

#[derive(Default)]
struct RecordingTerminalObserver {
    events: Mutex<Vec<TerminalOperationEvent>>,
}

impl RecordingTerminalObserver {
    fn events(&self) -> Vec<TerminalOperationEvent> {
        self.events.lock().expect("observer lock").clone()
    }
}

#[async_trait]
impl TerminalOperationObserver for RecordingTerminalObserver {
    async fn observe(&self, event: &TerminalOperationEvent) {
        self.events
            .lock()
            .expect("observer lock")
            .push(event.clone());
    }
}

async fn seed_request(application: &rambledesk_core::FeedbackApplication) -> String {
    let request_id = uuid::Uuid::now_v7().to_string();
    application
        .request_feedback(RequestFeedbackInput {
            request_id: Some(request_id.clone()),
            host_id: Some("codex".into()),
            host_session_id: "application-api-session".into(),
            title: Some("Review the application API".into()),
            what_happened: "The HTTP transport needs shared read contracts.".into(),
            actions: vec![ActionInput {
                id: "verify".into(),
                instruction: "Verify the read projections.".into(),
            }],
            context_refs: vec![],
            attachments: vec![RequestAttachmentInput {
                file_name: "request-context.md".into(),
                markdown: Some(String::from_utf8_lossy(REQUEST_ATTACHMENT_BYTES).into_owned()),
                contents_base64: None,
                path: None,
            }],
            source_hint: Some("application API test".into()),
            allow_finish: false,
            final_summary: None,
        })
        .await
        .expect("request should be created");
    request_id
}

async fn seed_final_summary_request(
    application: &rambledesk_core::FeedbackApplication,
    host_session_id: &str,
) -> String {
    let request_id = uuid::Uuid::now_v7().to_string();
    application
        .request_feedback(RequestFeedbackInput {
            request_id: Some(request_id.clone()),
            host_id: Some("codex".into()),
            host_session_id: host_session_id.into(),
            title: Some("Approve the final summary".into()),
            what_happened: "The host supplied a final summary.".into(),
            actions: vec![ActionInput {
                id: "verify".into(),
                instruction: "Verify the final summary.".into(),
            }],
            context_refs: vec![],
            attachments: vec![],
            source_hint: Some("application API test".into()),
            allow_finish: true,
            final_summary: Some("The requested work is complete.".into()),
        })
        .await
        .expect("final summary request should be created");
    request_id
}

#[tokio::test]
async fn application_routes_use_an_independent_bearer_wrapper() -> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;

    let response = reqwest::Client::new()
        .post(application_url(server.address(), "listHostProfiles"))
        .send()
        .await?;

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn local_integration_server_does_not_mount_application_routes() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;

    let response = reqwest::Client::new()
        .post(application_url(server.address(), "listHostProfiles"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn application_router_exposes_the_complete_23_operation_surface() -> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();

    for operation in APPLICATION_OPERATIONS {
        let response = client
            .post(application_url(server.address(), operation))
            .bearer_auth(TEST_TOKEN)
            .send()
            .await?;
        let expected = if matches!(
            operation,
            "listFeedbackInbox" | "listHostSessions" | "listHostProfiles"
        ) {
            reqwest::StatusCode::OK
        } else {
            reqwest::StatusCode::BAD_REQUEST
        };
        assert_eq!(response.status(), expected, "operation {operation}");
        if expected == reqwest::StatusCode::BAD_REQUEST {
            assert_eq!(
                response.json::<serde_json::Value>().await?["code"],
                "INVALID_ARGUMENT",
                "operation {operation}"
            );
        }
    }

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn missing_and_malformed_json_are_typed_invalid_argument_errors() -> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();

    let missing = client
        .post(application_url(server.address(), "getFeedbackWorkspace"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await?;
    assert_eq!(missing.status(), reqwest::StatusCode::BAD_REQUEST);
    let missing_error = missing.json::<serde_json::Value>().await?;
    assert_eq!(missing_error["code"], "INVALID_ARGUMENT");
    assert_eq!(missing_error["retryable"], false);

    let malformed = client
        .post(application_url(server.address(), "getFeedbackWorkspace"))
        .bearer_auth(TEST_TOKEN)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body("{")
        .send()
        .await?;
    assert_eq!(malformed.status(), reqwest::StatusCode::BAD_REQUEST);
    let malformed_error = malformed.json::<serde_json::Value>().await?;
    assert_eq!(malformed_error["code"], "INVALID_ARGUMENT");
    assert_eq!(malformed_error["retryable"], false);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn read_and_list_routes_use_shared_request_and_response_shapes() -> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let request_id = seed_request(&application).await;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();

    let inbox = client
        .post(application_url(server.address(), "listFeedbackInbox"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(inbox[0]["request_id"], request_id);

    let sessions = client
        .post(application_url(server.address(), "listHostSessions"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(sessions[0]["host_id"], "codex");
    assert_eq!(sessions[0]["host_session_id"], "application-api-session");

    let archived = client
        .post(application_url(
            server.address(),
            "listArchivedHostSessions",
        ))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "search": null }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(archived, serde_json::json!([]));

    let profiles = client
        .post(application_url(server.address(), "listHostProfiles"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert!(
        profiles
            .as_array()
            .is_some_and(|profiles| { profiles.iter().any(|profile| profile["id"] == "codex") })
    );

    let requests = client
        .post(application_url(server.address(), "listFeedbackRequests"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "host_id": "codex",
            "host_session_id": "application-api-session",
            "status": ["waiting", "in_progress"],
            "archived": false,
            "search": null,
            "limit": 100,
            "cursor": null
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(requests["requests"][0]["request_id"], request_id);
    assert!(requests["next_cursor"].is_null());

    let workspace = client
        .post(application_url(server.address(), "getFeedbackWorkspace"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": request_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(workspace["request"]["host_id"], "codex");
    assert!(workspace.get("draft").is_some());
    assert!(workspace.get("attachments").is_some());

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn published_projection_hides_storage_paths_and_errors_stay_structured() -> anyhow::Result<()>
{
    let (application, _directory) = test_application().await?;
    let request_id = seed_request(&application).await;
    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            document_json: "{}".into(),
            body_markdown: "Operator feedback".into(),
            expected_revision: 0,
        })
        .await
        .expect("draft should save");
    application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .expect("feedback should publish");

    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();
    let published = client
        .post(application_url(server.address(), "readPublishedFeedback"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": request_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert!(published.get("manifest").is_some());
    assert!(published.get("markdown").is_some());
    assert!(published.get("attachment_paths").is_none());
    assert!(published.get("request_attachment_paths").is_none());

    let invalid = client
        .post(application_url(server.address(), "getFeedbackWorkspace"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": "not-a-uuid" }))
        .send()
        .await?;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid.json::<serde_json::Value>().await?,
        serde_json::json!({
            "code": "INVALID_ARGUMENT",
            "message": "request_id must be a UUID",
            "retryable": false
        })
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn terminal_facade_notifies_once_for_concurrent_success_and_never_for_failure()
-> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let first_request = seed_final_summary_request(&application, "facade-first").await;
    let second_request = seed_final_summary_request(&application, "facade-second").await;
    let invalid_request = seed_request(&application).await;
    let observer = Arc::new(RecordingTerminalObserver::default());
    let operations = WorkbenchTerminalOperations::new(application, observer.clone());

    let first = operations.approve_feedback(ApproveFeedbackInput {
        request_id: first_request.clone(),
    });
    let repeated = operations.approve_feedback(ApproveFeedbackInput {
        request_id: first_request.clone(),
    });
    let (first, repeated) = tokio::join!(first, repeated);
    assert_eq!(first?.request_id, first_request);
    assert_eq!(repeated?.request_id, first_request);
    assert_eq!(observer.events().len(), 1);

    let failed = operations
        .approve_feedback(ApproveFeedbackInput {
            request_id: invalid_request,
        })
        .await;
    assert_eq!(
        failed.expect_err("approval should fail").code(),
        "REQUEST_TERMINAL"
    );
    assert_eq!(observer.events().len(), 1);

    operations
        .approve_feedback(ApproveFeedbackInput {
            request_id: second_request.clone(),
        })
        .await?;
    let events = observer.events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].operation, TerminalOperation::ApproveFeedback);
    assert_eq!(events[1].operation, TerminalOperation::ApproveFeedback);
    assert_eq!(events[1].request.request_id, second_request);
    Ok(())
}

#[tokio::test]
async fn http_terminal_mutations_share_the_observer_and_project_terminal_state()
-> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let submitted_request = seed_request(&application).await;
    let submitted_draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: submitted_request.clone(),
            document_json: "{}".into(),
            body_markdown: "Submit through HTTP".into(),
            expected_revision: 0,
        })
        .await?;
    let approved_request = seed_final_summary_request(&application, "http-approve").await;
    let cancelled_request = seed_final_summary_request(&application, "http-cancel").await;
    let observer = Arc::new(RecordingTerminalObserver::default());
    let operations = WorkbenchTerminalOperations::new(application.clone(), observer.clone());
    let server = start_application_server(application, operations).await?;
    let client = reqwest::Client::new();

    for _ in 0..2 {
        let submitted = client
            .post(application_url(server.address(), "submitFeedback"))
            .bearer_auth(TEST_TOKEN)
            .json(&serde_json::json!({
                "request_id": submitted_request,
                "expected_revision": submitted_draft.saved_revision
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        assert_eq!(submitted["status"], "completed");
        assert_no_path_keys(&submitted);
    }

    let approved = client
        .post(application_url(server.address(), "approveFeedbackRequest"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": approved_request }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(approved["resolution"], "approved");
    assert_no_path_keys(&approved);

    let cancelled = client
        .post(application_url(server.address(), "cancelFeedbackRequest"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": cancelled_request,
            "reason": "No longer needed"
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(cancelled["status"], "cancelled");
    assert_no_path_keys(&cancelled);

    let workspace = client
        .post(application_url(server.address(), "getFeedbackWorkspace"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": submitted_request }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(workspace["request"]["status"], "completed");
    assert_eq!(workspace["draft"]["body_markdown"], "Submit through HTTP");
    assert_eq!(workspace["feedback"]["available"], true);
    assert_no_path_keys(&workspace);

    let events = observer.events();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].operation, TerminalOperation::SubmitFeedback);
    assert_eq!(events[1].operation, TerminalOperation::ApproveFeedback);
    assert_eq!(events[2].operation, TerminalOperation::CancelFeedback);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn draft_cas_error_is_structured_and_preserves_the_saved_projection() -> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let request_id = seed_request(&application).await;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();

    let first = client
        .post(application_url(server.address(), "saveFeedbackDraft"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": request_id,
            "document_json": "{\"type\":\"doc\"}",
            "body_markdown": "First saved draft",
            "expected_revision": 0
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(first["saved_revision"], 1);

    let stale = client
        .post(application_url(server.address(), "saveFeedbackDraft"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": request_id,
            "document_json": "{\"type\":\"doc\"}",
            "body_markdown": "Stale overwrite",
            "expected_revision": 0
        }))
        .send()
        .await?;
    assert_eq!(stale.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        stale.json::<serde_json::Value>().await?["code"],
        "DRAFT_CONFLICT"
    );

    let workspace = client
        .post(application_url(server.address(), "getFeedbackWorkspace"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": request_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(workspace["draft"]["body_markdown"], "First saved draft");
    assert_eq!(workspace["draft"]["saved_revision"], 1);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn host_session_mutations_update_projection_and_delete_routes_return_no_content()
-> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let request_id = seed_request(&application).await;
    let whole_session_request =
        seed_final_summary_request(&application, "delete-whole-session").await;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();

    let renamed = client
        .post(application_url(server.address(), "renameHostSession"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "host_id": "codex",
            "host_session_id": "application-api-session",
            "title": "Renamed session"
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(renamed["title"], "Renamed session");

    let pinned = client
        .post(application_url(server.address(), "setHostSessionPinned"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "host_id": "codex",
            "host_session_id": "application-api-session",
            "pinned": true
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert!(pinned["pinned_at"].is_string());

    let host_pinned = client
        .post(application_url(server.address(), "setHostPinned"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "host_id": "codex", "pinned": true }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert!(host_pinned[0]["host_pinned_at"].is_string());

    client
        .post(application_url(server.address(), "cancelFeedbackRequest"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": request_id,
            "reason": "Close the test request"
        }))
        .send()
        .await?
        .error_for_status()?;

    client
        .post(application_url(server.address(), "cancelFeedbackRequest"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": whole_session_request,
            "reason": "Delete the whole test session"
        }))
        .send()
        .await?
        .error_for_status()?;

    for operation in [
        "archiveHostSession",
        "unarchiveHostSession",
        "archiveHostSession",
    ] {
        client
            .post(application_url(server.address(), operation))
            .bearer_auth(TEST_TOKEN)
            .json(&serde_json::json!({
                "host_id": "codex",
                "host_session_id": "application-api-session"
            }))
            .send()
            .await?
            .error_for_status()?;
    }

    let deleted_request = client
        .post(application_url(server.address(), "deleteFeedbackRequest"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": request_id }))
        .send()
        .await?;
    assert_eq!(deleted_request.status(), reqwest::StatusCode::NO_CONTENT);
    assert_eq!(deleted_request.bytes().await?.len(), 0);

    client
        .post(application_url(server.address(), "archiveHostSession"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "host_id": "codex",
            "host_session_id": "delete-whole-session"
        }))
        .send()
        .await?
        .error_for_status()?;

    let deleted_session = client
        .post(application_url(server.address(), "deleteHostSession"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "host_id": "codex",
            "host_session_id": "delete-whole-session"
        }))
        .send()
        .await?;
    assert_eq!(deleted_session.status(), reqwest::StatusCode::NO_CONTENT);
    assert_eq!(deleted_session.bytes().await?.len(), 0);

    server.shutdown().await?;
    Ok(())
}
