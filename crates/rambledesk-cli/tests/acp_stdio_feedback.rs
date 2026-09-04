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
                id: None,
                name: "Stdio fixture".into(),
                host_id: "fixture".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: "node".into(),
                args: vec![
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/acp_stdio_feedback.mjs")
                        .to_string_lossy()
                        .into(),
                    if http { "http" } else { "stdio" }.into(),
                ],
                env: BTreeMap::from([
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
    assert!(check.details[0].contains("managed feedback: stdio"));
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
            Some(FeedbackTransport::Stdio)
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
    fixture.settled(two, "REQUEST_NOT_FOUND").await;
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
async fn http_remains_preferred_and_missing_companion_is_reported_without_claiming_http_support() {
    let fixture = Fixture::new(true, PathBuf::from("missing-relative-companion")).await;
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
        Some(FeedbackTransport::Http)
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
