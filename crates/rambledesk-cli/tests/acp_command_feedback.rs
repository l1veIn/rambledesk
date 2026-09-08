use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_local_server::{
    AccessToken, LocalManagedFeedbackProvider, ServerConfig, ServerHandle,
    start_server_with_managed,
};
use rambledesk_storage::SqliteFeedbackStore;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

struct Fixture {
    dir: tempfile::TempDir,
    store: Arc<SqliteFeedbackStore>,
    app: SessionApplication,
    feedback: FeedbackApplication,
    server: ServerHandle,
    config: String,
}
impl Fixture {
    async fn new(http: bool, companion: PathBuf) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
                .await
                .unwrap(),
        );
        let feedback = (*store).clone().into_application();
        let provider = Arc::new(LocalManagedFeedbackProvider::new(feedback.clone()));
        let server = start_server_with_managed(
            ServerConfig::new(AccessToken::generate()).with_port(0),
            feedback.clone(),
            provider.clone(),
        )
        .await
        .unwrap();
        let driver = AcpSessionDriver::with_feedback_companion(companion);
        let app = SessionApplication::new(store.clone(), store.clone(), Arc::new(driver))
            .with_feedback_provider(provider)
            .with_deliveries(store.clone());
        app.start_delivery_worker().await.unwrap();
        let config = app
            .save_agent_config(SaveAgentConfigInput {
                catalog_id: None,
                id: None,
                name: "Command fixture".into(),
                host_id: "fixture".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: "node".into(),
                args: vec![
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/acp_command_feedback.mjs")
                        .to_string_lossy()
                        .into(),
                    if http { "http" } else { "no-mcp" }.into(),
                ],
                env: BTreeMap::from([
                    (
                        "RAMBLEDESK_FEEDBACK_CHANNEL".into(),
                        "persisted-channel-must-not-be-trusted".into(),
                    ),
                    (
                        "RAMBLEDESK_FEEDBACK_TOKEN".into(),
                        "persisted-value-must-not-be-trusted".into(),
                    ),
                    (
                        "RAMBLEDESK_COMMAND".into(),
                        "persisted-command-must-not-run".into(),
                    ),
                    (
                        "RAMBLEDESK_MANAGED_MCP_TOKEN".into(),
                        "persisted-value-must-not-be-trusted".into(),
                    ),
                    (
                        "RAMBLEDESK_MANAGED_MCP_URL".into(),
                        "http://invalid.test".into(),
                    ),
                    ("RAMBLEDESK_MANAGED_PI_WRAPPER".into(), "1".into()),
                ]),
            })
            .await
            .unwrap()
            .id;
        Self {
            dir,
            store,
            app,
            feedback,
            server,
            config,
        }
    }
    async fn create(&self) -> ManagedSessionSnapshot {
        self.app
            .create_session(CreateManagedSessionInput {
                agent_config_id: self.config.clone(),
                cwd: self.dir.path().to_string_lossy().into(),
                title: "Owned session".into(),
            })
            .await
            .unwrap()
    }
    async fn snapshot(&self, id: &str) -> ManagedSessionSnapshot {
        self.app
            .get_session(ManagedSessionInput {
                session_id: id.into(),
            })
            .await
            .unwrap()
    }
    async fn settled(&self, id: &str, marker: &str) -> ManagedSessionSnapshot {
        tokio::time::timeout(Duration::from_secs(12), async {
            loop {
                let snapshot = self.snapshot(id).await;
                if snapshot.runtime.activity == SessionActivityState::Idle
                    && snapshot
                        .activities
                        .iter()
                        .any(|row| row.text.contains(marker))
                {
                    return snapshot;
                }
                assert_eq!(
                    snapshot.runtime.connection,
                    SessionConnectionState::Connected,
                    "{:?}",
                    snapshot.runtime.last_error
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn close(self) {
        self.app.shutdown().await.unwrap();
        self.server.shutdown().await.unwrap();
        self.store.close().await;
    }
}

#[tokio::test]
async fn configured_driver_runs_real_scoped_companion_chain_and_original_context() {
    let fixture = Fixture::new(false, PathBuf::from(env!("CARGO_BIN_EXE_rambledesk"))).await;
    let check = fixture
        .app
        .check_agent_config(AgentConfigInput {
            agent_config_id: fixture.config.clone(),
        })
        .await
        .unwrap();
    assert!(check.ok, "{}", check.message);
    assert!(check.details[0].contains("managed feedback: command"));
    let first = fixture.create().await;
    let second = fixture.create().await;
    for snapshot in [&first, &second] {
        assert_eq!(
            snapshot.runtime.connection,
            SessionConnectionState::Connected,
            "{:?}",
            snapshot.runtime.last_error
        );
        assert!(!snapshot.runtime.capabilities.http_mcp);
        assert_eq!(
            snapshot.runtime.capabilities.feedback_transport,
            Some(FeedbackTransport::Command)
        );
    }
    let one = &first.session.session_id;
    let two = &second.session.session_id;
    let request = "b66a2bc2-474e-43e8-b8ea-ad638bda53bc";
    fixture
        .app
        .send_prompt(SendManagedPromptInput {
            session_id: one.clone(),
            text: format!("request:{request}"),
        })
        .await
        .unwrap();
    fixture.settled(one, "REQUEST").await;
    let stored = fixture.store.get_request(request).await.unwrap();
    assert_eq!(stored.managed_session_id.as_deref(), Some(one.as_str()));
    assert_eq!(stored.host_id, "fixture");
    fixture
        .app
        .send_prompt(SendManagedPromptInput {
            session_id: two.clone(),
            text: format!("get:{request}"),
        })
        .await
        .unwrap();
    let failed = fixture.settled(two, "REQUEST_NOT_FOUND").await;
    assert!(
        failed
            .runtime
            .last_error
            .as_deref()
            .unwrap()
            .contains("without a Ramble handoff")
    );
    assert!(
        !failed
            .activities
            .iter()
            .any(|row| row.text.contains("HANDOFF_RETRY"))
    );
    let saved = fixture
        .feedback
        .save_feedback_draft(SaveDraftInput {
            request_id: request.into(),
            expected_revision: 0,
            document_json: r#"{"schemaVersion":2,"doc":{"type":"doc"}}"#.into(),
            body_markdown: "Continue this original session.".into(),
        })
        .await
        .unwrap();
    fixture
        .feedback
        .submit_feedback(SubmitFeedbackInput {
            request_id: request.into(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .unwrap();
    fixture.settled(one, "RESULT feedback_submitted").await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if fixture.snapshot(one).await.deliveries[0].state == FeedbackDeliveryState::Delivered {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    fixture
        .app
        .stop_session(ManagedSessionInput {
            session_id: one.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        fixture.snapshot(two).await.runtime.connection,
        SessionConnectionState::Connected
    );
    let restored = fixture
        .app
        .start_session(ManagedSessionInput {
            session_id: one.clone(),
        })
        .await
        .unwrap();
    assert_eq!(restored.session.management, first.session.management);
    fixture
        .app
        .send_prompt(SendManagedPromptInput {
            session_id: one.clone(),
            text: format!("get:{request}"),
        })
        .await
        .unwrap();
    fixture.settled(one, "RESULT feedback_submitted").await;
    fixture.close().await;
}

#[tokio::test]
async fn mcp_capabilities_do_not_change_workflow_and_missing_command_fails_explicitly() {
    let fixture = Fixture::new(true, PathBuf::from(env!("CARGO_BIN_EXE_rambledesk"))).await;
    assert!(
        fixture
            .app
            .check_agent_config(AgentConfigInput {
                agent_config_id: fixture.config.clone()
            })
            .await
            .unwrap()
            .ok
    );
    let snapshot = fixture.create().await;
    assert_eq!(
        snapshot.runtime.connection,
        SessionConnectionState::Connected
    );
    assert_eq!(
        snapshot.runtime.capabilities.feedback_transport,
        Some(FeedbackTransport::Command)
    );
    fixture.close().await;
    let fixture = Fixture::new(false, PathBuf::from("missing-relative-companion")).await;
    let check = fixture
        .app
        .check_agent_config(AgentConfigInput {
            agent_config_id: fixture.config.clone(),
        })
        .await
        .unwrap();
    assert!(!check.ok);
    let snapshot = fixture.create().await;
    assert_eq!(snapshot.runtime.connection, SessionConnectionState::Failed);
    fixture.close().await;
}

#[tokio::test]
async fn missing_handoff_is_reminded_once_without_repeating_work_or_creating_a_user_turn() {
    let fixture = Fixture::new(false, PathBuf::from(env!("CARGO_BIN_EXE_rambledesk"))).await;
    let session = fixture.create().await;
    let id = &session.session.session_id;
    let request = "01992658-1250-7000-8000-000000000071";
    fixture
        .app
        .send_prompt(SendManagedPromptInput {
            session_id: id.clone(),
            text: format!("ignore:{request}"),
        })
        .await
        .unwrap();
    let result = fixture.settled(id, "REQUEST").await;
    assert!(
        result.runtime.last_error.is_none(),
        "{:?}",
        result.runtime.last_error
    );
    let transcript = result
        .activities
        .iter()
        .map(|row| row.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(transcript.matches("ORIGINAL_ANSWER").count(), 1);
    assert_eq!(transcript.matches("HANDOFF_RETRY").count(), 1);
    assert_eq!(
        result
            .activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::UserMessage)
            .count(),
        1
    );
    assert_eq!(
        fixture
            .store
            .get_request(request)
            .await
            .unwrap()
            .managed_session_id
            .as_deref(),
        Some(id.as_str())
    );
    fixture.close().await;
}

#[tokio::test]
async fn repeated_omission_is_visible_and_explicit_skip_is_only_valid_for_its_turn() {
    let fixture = Fixture::new(false, PathBuf::from(env!("CARGO_BIN_EXE_rambledesk"))).await;
    let session = fixture.create().await;
    let id = &session.session.session_id;
    fixture
        .app
        .send_prompt(SendManagedPromptInput {
            session_id: id.clone(),
            text: "skip:user_opt_out".into(),
        })
        .await
        .unwrap();
    let skipped = fixture.settled(id, "SKIP skipped").await;
    assert!(skipped.runtime.last_error.is_none());
    assert!(
        !skipped
            .activities
            .iter()
            .any(|row| row.text.contains("HANDOFF_RETRY"))
    );
    fixture
        .app
        .send_prompt(SendManagedPromptInput {
            session_id: id.clone(),
            text: "silent".into(),
        })
        .await
        .unwrap();
    let failed = fixture.settled(id, "HANDOFF_RETRY").await;
    assert!(
        failed
            .runtime
            .last_error
            .as_deref()
            .unwrap()
            .contains("without a Ramble handoff")
    );
    assert!(
        failed
            .activities
            .iter()
            .any(|row| row.kind == SessionActivityKind::Error)
    );
    assert_eq!(
        failed
            .activities
            .iter()
            .map(|row| row.text.matches("HANDOFF_RETRY").count())
            .sum::<usize>(),
        1
    );
    assert_eq!(failed.runtime.connection, SessionConnectionState::Connected);
    fixture.close().await;
}

#[tokio::test]
async fn user_cancellation_does_not_start_a_handoff_retry_even_if_agent_reports_end_turn() {
    let fixture = Fixture::new(false, PathBuf::from(env!("CARGO_BIN_EXE_rambledesk"))).await;
    let session = fixture.create().await;
    let id = &session.session.session_id;
    fixture
        .app
        .send_prompt(SendManagedPromptInput {
            session_id: id.clone(),
            text: "cancel".into(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !fixture
            .snapshot(id)
            .await
            .activities
            .iter()
            .any(|row| row.text.contains("WAIT_CANCEL"))
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    fixture
        .app
        .cancel_prompt(ManagedSessionInput {
            session_id: id.clone(),
        })
        .await
        .unwrap();
    let cancelled = fixture.settled(id, "CANCELLED").await;
    assert!(
        !cancelled
            .activities
            .iter()
            .any(|row| row.text.contains("HANDOFF_RETRY"))
    );
    assert!(cancelled.runtime.last_error.is_none());
    fixture.close().await;
}
