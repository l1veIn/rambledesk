use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use axum::Router;
use rambledesk_core::{
    ActionInput, ApplicationCommandFacade, ApplicationError, ApplicationHostProfileView, Clock,
    FeedbackApplication, GetFeedbackInput, HostSessionInput, IdGenerator, ReadAttachmentInput,
    RequestFeedbackInput, SaveDraftInput, SubmitFeedbackInput, TerminalOperationEvent,
    TerminalOperationObserver, WorkbenchTerminalOperations,
};
use rambledesk_hosts::{ContinuationMode, HostAdapter, known_host_profiles};
use rambledesk_local_server::{API_PATH, application_router};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

const REQUEST_ID: &str = "01900000-0000-7000-8000-000000000010";
const HOST_SESSION_ID: &str = "transport-parity-session";
const NOW: &str = "2026-09-01T12:00:00Z";

struct FixedClock;

impl Clock for FixedClock {
    fn now_rfc3339(&self) -> String {
        NOW.to_owned()
    }
}

struct FixedIds(AtomicUsize);

impl FixedIds {
    fn new() -> Self {
        Self(AtomicUsize::new(1))
    }
}

impl IdGenerator for FixedIds {
    fn new_id(&self) -> String {
        let suffix = self.0.fetch_add(1, Ordering::Relaxed);
        format!("01900000-0000-7000-8000-{suffix:012}")
    }
}

#[derive(Default)]
struct RecordingObserver(Mutex<Vec<TerminalOperationEvent>>);

impl RecordingObserver {
    fn count(&self) -> usize {
        self.0.lock().expect("observer lock").len()
    }
}

#[async_trait]
impl TerminalOperationObserver for RecordingObserver {
    async fn observe(&self, event: &TerminalOperationEvent) {
        self.0.lock().expect("observer lock").push(event.clone());
    }
}

struct HttpServer {
    address: std::net::SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<std::io::Result<()>>,
}

impl HttpServer {
    fn url(&self, operation: &str) -> String {
        format!("http://{}/api/application/{operation}", self.address)
    }

    async fn shutdown(self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        self.task.await??;
        Ok(())
    }
}

async fn start_http(commands: Arc<ApplicationCommandFacade>) -> anyhow::Result<HttpServer> {
    let router = Router::new().nest(API_PATH, application_router(commands));
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let address = listener.local_addr()?;
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { task_cancellation.cancelled_owned().await })
            .await
    });
    Ok(HttpServer {
        address,
        cancellation,
        task,
    })
}

fn application_host_profiles() -> Vec<ApplicationHostProfileView> {
    known_host_profiles()
        .into_iter()
        .map(|profile| ApplicationHostProfileView {
            id: profile.id,
            label: profile.label,
            icon_svg: profile.icon_svg,
            default_adapter: match profile.default_adapter {
                HostAdapter::GenericMcp => "generic_mcp",
                HostAdapter::PiNative => "pi_native",
            }
            .into(),
            continuation_mode: match profile.continuation_mode {
                ContinuationMode::NotRequired => "not_required",
                ContinuationMode::Manual => "manual",
                ContinuationMode::Native => "native",
            }
            .into(),
        })
        .collect()
}

async fn test_application() -> anyhow::Result<(FeedbackApplication, tempfile::TempDir)> {
    let directory = tempfile::tempdir()?;
    let store = rambledesk_storage::SqliteFeedbackStore::connect(
        &directory.path().join("rambledesk.sqlite3"),
    )
    .await?;
    let store = Arc::new(store);
    let application = FeedbackApplication::with_runtime(
        store.clone(),
        store.clone(),
        store,
        Arc::new(FixedClock),
        Arc::new(FixedIds::new()),
    );
    Ok((application, directory))
}

async fn seed(application: &FeedbackApplication) -> Result<(), ApplicationError> {
    application
        .request_feedback(RequestFeedbackInput {
            request_id: Some(REQUEST_ID.into()),
            host_id: Some("codex".into()),
            host_session_id: HOST_SESSION_ID.into(),
            title: Some("Transport parity".into()),
            what_happened: "Compare the Tauri facade and HTTP implementation.".into(),
            actions: vec![ActionInput {
                id: "compare".into(),
                instruction: "Compare each transport outcome.".into(),
            }],
            context_refs: vec![],
            attachments: vec![],
            source_hint: Some("parity test".into()),
            allow_finish: false,
            final_summary: None,
        })
        .await?;
    Ok(())
}

fn assert_no_storage_locations(value: &serde_json::Value, roots: &[&std::path::Path]) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                assert_no_storage_locations(value, roots);
            }
        }
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                assert!(
                    !matches!(
                        key.as_str(),
                        "directory_path" | "markdown_path" | "manifest_path" | "package_uri"
                    ),
                    "storage location key leaked: {key}"
                );
                assert_no_storage_locations(value, roots);
            }
        }
        serde_json::Value::String(value) => {
            assert!(!value.starts_with("file://"), "file URI leaked: {value}");
            assert!(
                !std::path::Path::new(value).is_absolute(),
                "absolute path leaked: {value}"
            );
            for root in roots {
                assert!(
                    !value.contains(&root.to_string_lossy().into_owned()),
                    "fixture storage root leaked: {value}"
                );
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn tauri_facade_and_http_implementation_have_equivalent_contract_outcomes()
-> anyhow::Result<()> {
    let (application, directory) = test_application().await?;
    let storage_roots = [directory.path()];
    seed(&application).await?;

    let observer = Arc::new(RecordingObserver::default());
    let commands = Arc::new(ApplicationCommandFacade::new(
        application.clone(),
        WorkbenchTerminalOperations::new(application, observer.clone()),
        application_host_profiles(),
    ));
    let server = start_http(commands.clone()).await?;
    let client = reqwest::Client::new();

    let direct_profiles = commands.list_host_profiles();
    let http_profiles = client
        .post(server.url("listHostProfiles"))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(serde_json::to_value(direct_profiles)?, http_profiles);

    let direct_workspace = commands
        .get_feedback_workspace(GetFeedbackInput {
            request_id: REQUEST_ID.into(),
        })
        .await?;
    let http_workspace = client
        .post(server.url("getFeedbackWorkspace"))
        .json(&serde_json::json!({ "request_id": REQUEST_ID }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(serde_json::to_value(direct_workspace)?, http_workspace);

    let save = SaveDraftInput {
        request_id: REQUEST_ID.into(),
        document_json: "{\"type\":\"doc\"}".into(),
        body_markdown: "Equivalent draft".into(),
        expected_revision: 0,
    };
    let direct_saved = commands.save_feedback_draft(save.clone()).await?;
    let http_saved = client
        .post(server.url("saveFeedbackDraft"))
        .json(&save)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(serde_json::to_value(direct_saved)?, http_saved);

    let stale_save = SaveDraftInput {
        body_markdown: "Stale overwrite".into(),
        ..save.clone()
    };
    let direct_stale = commands
        .save_feedback_draft(stale_save.clone())
        .await
        .expect_err("direct stale save");
    let http_stale = client
        .post(server.url("saveFeedbackDraft"))
        .json(&stale_save)
        .send()
        .await?;
    assert_eq!(http_stale.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        serde_json::to_value(direct_stale)?,
        http_stale.json::<serde_json::Value>().await?
    );

    let attachment_bytes = b"transport parity bytes".to_vec();
    let http_attachment = client
        .post(server.url("addFeedbackAttachment"))
        .multipart(
            reqwest::multipart::Form::new()
                .text("request_id", REQUEST_ID)
                .text("file_name", "parity.txt")
                .text("expected_revision", "1")
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(attachment_bytes.clone()),
                ),
        )
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let direct_attachment = commands
        .get_feedback_workspace(GetFeedbackInput {
            request_id: REQUEST_ID.into(),
        })
        .await?;
    assert_eq!(serde_json::to_value(&direct_attachment)?, http_attachment);
    let attachment_id = direct_attachment.attachments[0].attachment_id.clone();
    let direct_bytes = commands
        .read_feedback_attachment(ReadAttachmentInput {
            request_id: REQUEST_ID.into(),
            attachment_id: attachment_id.clone(),
        })
        .await?;
    let http_bytes = client
        .post(server.url("readFeedbackAttachment"))
        .json(&serde_json::json!({
            "request_id": REQUEST_ID,
            "attachment_id": attachment_id
        }))
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    assert_eq!(direct_bytes, http_bytes.as_ref());

    let submit = SubmitFeedbackInput {
        request_id: REQUEST_ID.into(),
        expected_revision: 2,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    };
    let direct_submitted = commands.submit_feedback(submit.clone()).await?;
    let http_submitted = client
        .post(server.url("submitFeedback"))
        .json(&submit)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(serde_json::to_value(&direct_submitted)?, http_submitted);
    assert_no_storage_locations(&http_submitted, &storage_roots);

    commands.submit_feedback(submit.clone()).await?;
    client
        .post(server.url("submitFeedback"))
        .json(&submit)
        .send()
        .await?
        .error_for_status()?;
    assert_eq!(observer.count(), 1);

    let direct_completed_workspace = commands
        .get_feedback_workspace(GetFeedbackInput {
            request_id: REQUEST_ID.into(),
        })
        .await?;
    let http_completed_workspace = client
        .post(server.url("getFeedbackWorkspace"))
        .json(&serde_json::json!({ "request_id": REQUEST_ID }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(
        serde_json::to_value(direct_completed_workspace)?,
        http_completed_workspace
    );
    assert_no_storage_locations(&http_completed_workspace, &storage_roots);

    let direct_published = commands
        .read_published_feedback(GetFeedbackInput {
            request_id: REQUEST_ID.into(),
        })
        .await?;
    let http_published = client
        .post(server.url("readPublishedFeedback"))
        .json(&serde_json::json!({ "request_id": REQUEST_ID }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(serde_json::to_value(direct_published)?, http_published);
    assert_no_storage_locations(&http_published, &storage_roots);

    let session = HostSessionInput {
        host_id: "codex".into(),
        host_session_id: HOST_SESSION_ID.into(),
    };
    commands.archive_host_session(session.clone()).await?;
    client
        .post(server.url("archiveHostSession"))
        .json(&session)
        .send()
        .await?
        .error_for_status()?;
    let deleted = client
        .post(server.url("deleteFeedbackRequest"))
        .json(&serde_json::json!({ "request_id": REQUEST_ID }))
        .send()
        .await?;
    assert_eq!(deleted.status(), reqwest::StatusCode::NO_CONTENT);

    let direct_missing = commands
        .get_feedback_workspace(GetFeedbackInput {
            request_id: REQUEST_ID.into(),
        })
        .await
        .expect_err("direct request deleted");
    let http_missing = client
        .post(server.url("getFeedbackWorkspace"))
        .json(&serde_json::json!({ "request_id": REQUEST_ID }))
        .send()
        .await?;
    assert_eq!(http_missing.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        serde_json::to_value(direct_missing)?,
        http_missing.json::<serde_json::Value>().await?
    );

    server.shutdown().await?;
    Ok(())
}

#[test]
fn tauri_application_commands_delegate_to_the_shared_facade() {
    let source = include_str!("../src/commands.rs");
    for command in [
        "get_feedback_workspace",
        "save_feedback_draft",
        "add_feedback_attachment",
        "submit_feedback",
        "delete_feedback_request",
        "read_feedback_attachment",
    ] {
        let start = source
            .find(&format!("fn {command}"))
            .expect("command wrapper");
        let body = &source[start
            ..source[start..]
                .find("\n}\n")
                .map_or(source.len(), |end| start + end)];
        assert!(
            body.contains("application_commands"),
            "{command} must delegate to ApplicationCommandFacade"
        );
    }
}
