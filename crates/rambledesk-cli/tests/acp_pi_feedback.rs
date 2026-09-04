use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_local_server::{
    AccessToken, LocalManagedFeedbackProvider, ServerConfig, ServerHandle,
    start_server_with_managed,
};
use rambledesk_storage::SqliteFeedbackStore;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct Fixture {
    dir: tempfile::TempDir,
    store: Arc<SqliteFeedbackStore>,
    app: SessionApplication,
    feedback: FeedbackApplication,
    provider: Arc<LocalManagedFeedbackProvider>,
    server: ServerHandle,
    config: String,
}
impl Fixture {
    async fn new(resume: bool, http: bool, resources: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("installation");
        for (name, bin, source) in [
            (
                "pi-acp",
                "pi-acp",
                include_str!("fixtures/pi_acp_feedback.mjs"),
            ),
            (
                "@earendil-works/pi-coding-agent",
                "pi",
                include_str!("fixtures/pi_rpc.mjs"),
            ),
        ] {
            let package = prefix.join("node_modules").join(name);
            tokio::fs::create_dir_all(&package).await.unwrap();
            tokio::fs::write(
                package.join("package.json"),
                serde_json::to_vec(
                    &serde_json::json!({"name":name,"version":"0.0.33","bin":{bin:"index.mjs"}}),
                )
                .unwrap(),
            )
            .await
            .unwrap();
            tokio::fs::write(package.join("index.mjs"), source)
                .await
                .unwrap();
        }
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
        let mut driver = AcpSessionDriver::with_feedback_companion(PathBuf::from(env!(
            "CARGO_BIN_EXE_rambledesk"
        )));
        if resources {
            driver = driver.with_pi_extension_root(dir.path().join("pi-runtime"));
        }
        let app = SessionApplication::new(store.clone(), store.clone(), Arc::new(driver))
            .with_feedback_provider(provider.clone())
            .with_deliveries(store.clone())
            .with_deletions(store.clone())
            .with_recovery(store.clone());
        app.start_delivery_worker().await.unwrap();
        let config = app
            .save_agent_config(SaveAgentConfigInput {
                id: None,
                name: "Pi recipe".into(),
                host_id: "fixture".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: "node".into(),
                args: vec![
                    prefix
                        .join("node_modules/pi-acp/index.mjs")
                        .to_string_lossy()
                        .into(),
                ],
                env: BTreeMap::from([
                    (
                        "FIXTURE_PI_LOG".into(),
                        dir.path().join("events.jsonl").to_string_lossy().into(),
                    ),
                    (
                        "FIXTURE_RESUME".into(),
                        if resume { "1" } else { "0" }.into(),
                    ),
                    ("FIXTURE_HTTP".into(), if http { "1" } else { "0" }.into()),
                    (
                        "RAMBLEDESK_MANAGED_MCP_TOKEN".into(),
                        "persisted-value-is-not-authorization".into(),
                    ),
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
            provider,
            server,
            config,
        }
    }
    async fn create(&self, project: &str) -> ManagedSessionSnapshot {
        let cwd = self.dir.path().join(project);
        tokio::fs::create_dir_all(&cwd).await.unwrap();
        self.app
            .create_session(CreateManagedSessionInput {
                agent_config_id: self.config.clone(),
                cwd: cwd.to_string_lossy().into(),
                title: project.into(),
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
    async fn prompt(&self, id: &str, text: String, marker: &str) {
        self.app
            .send_prompt(SendManagedPromptInput {
                session_id: id.into(),
                text,
            })
            .await
            .unwrap();
        self.settled(id, marker).await;
    }
    async fn settled(&self, id: &str, marker: &str) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let snapshot = self.snapshot(id).await;
                if snapshot.runtime.activity == SessionActivityState::Idle
                    && snapshot
                        .activities
                        .iter()
                        .any(|row| row.text.contains(marker))
                {
                    break;
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
        .unwrap();
    }
    async fn close(self) {
        self.app.shutdown().await.unwrap();
        self.server.shutdown().await.unwrap();
        self.store.close().await;
    }
}

async fn assert_revoked(endpoint: ManagedFeedbackEndpoint) {
    let authority = endpoint
        .url
        .strip_prefix("http://")
        .unwrap()
        .split('/')
        .next()
        .unwrap();
    let mut socket = tokio::net::TcpStream::connect(authority).await.unwrap();
    socket.write_all(format!("POST /mcp-managed HTTP/1.1\r\nHost: {authority}\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", endpoint.bearer_token).as_bytes()).await.unwrap();
    let mut status = String::new();
    BufReader::new(socket).read_line(&mut status).await.unwrap();
    assert!(
        status.contains("401"),
        "Revoked scope should be unauthorized"
    );
}

async fn heartbeat(path: &Path) -> String {
    tokio::fs::read_to_string(path).await.unwrap()
}

#[tokio::test]
async fn pi_recipe_runs_private_extension_loop_loads_original_context_and_deletes_only_owned_tree()
{
    let fixture = Fixture::new(false, false, true).await;
    let check = fixture
        .app
        .check_agent_config(AgentConfigInput {
            agent_config_id: fixture.config.clone(),
        })
        .await
        .unwrap();
    assert!(check.ok, "{}", check.message);
    assert!(check.details[0].contains("managed feedback: pi_extension"));
    let a = fixture.create("project-a").await;
    let b = fixture.create("project-b").await;
    for snapshot in [&a, &b] {
        assert_eq!(
            snapshot.runtime.connection,
            SessionConnectionState::Connected,
            "{:?}",
            snapshot.runtime.last_error
        );
        assert_eq!(
            snapshot.runtime.capabilities.feedback_transport,
            Some(FeedbackTransport::PiExtension)
        );
        assert!(!snapshot.runtime.capabilities.http_mcp);
    }
    let one = &a.session.session_id;
    let two = &b.session.session_id;
    let request = "78fc13da-7777-4888-8888-7573aa44bb55";
    fixture
        .prompt(one, format!("request:{request}"), "REQUEST")
        .await;
    assert_eq!(
        fixture
            .store
            .get_request(request)
            .await
            .unwrap()
            .managed_session_id
            .as_deref(),
        Some(one.as_str())
    );
    fixture
        .prompt(two, format!("get:{request}"), "REQUEST_NOT_FOUND")
        .await;
    let saved = fixture
        .feedback
        .save_feedback_draft(SaveDraftInput {
            request_id: request.into(),
            expected_revision: 0,
            document_json: r#"{"schemaVersion":2,"doc":{"type":"doc"}}"#.into(),
            body_markdown: "Private Pi marker".into(),
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
        while fixture.snapshot(one).await.deliveries[0].state != FeedbackDeliveryState::Delivered {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let original_endpoint = fixture.provider.bind(&a.session).await.unwrap();
    fixture
        .app
        .stop_session(ManagedSessionInput {
            session_id: one.clone(),
        })
        .await
        .unwrap();
    assert_revoked(original_endpoint).await;
    let restored = fixture
        .app
        .start_session(ManagedSessionInput {
            session_id: one.clone(),
        })
        .await
        .unwrap();
    assert_eq!(restored.session.management, a.session.management);
    fixture
        .prompt(one, format!("get:{request}"), "RESULT feedback_submitted")
        .await;
    let replacement = fixture.provider.bind(&a.session).await.unwrap();
    fixture
        .app
        .delete_managed_session(ManagedSessionInput {
            session_id: one.clone(),
        })
        .await
        .unwrap();
    assert_revoked(replacement).await;
    assert!(fixture.store.get_request(request).await.is_err());
    assert!(
        fixture
            .store
            .list_session_deliveries(one)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture.snapshot(two).await.runtime.connection,
        SessionConnectionState::Connected
    );
    let stopped = fixture.dir.path().join("project-a/heartbeat");
    let live = fixture.dir.path().join("project-b/heartbeat");
    let a_before = heartbeat(&stopped).await;
    let b_before = heartbeat(&live).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(heartbeat(&stopped).await, a_before);
    assert_ne!(heartbeat(&live).await, b_before);
    let events = tokio::fs::read_to_string(fixture.dir.path().join("events.jsonl"))
        .await
        .unwrap();
    let events: Vec<serde_json::Value> = events
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["method"] == "session/new")
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["method"] == "session/load")
            .count(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn pi_resume_preserves_identity_and_missing_extension_is_not_mislabeled_stdio() {
    let fixture = Fixture::new(true, false, true).await;
    let session = fixture.create("resume").await;
    assert_eq!(
        session.runtime.connection,
        SessionConnectionState::Connected,
        "{:?}",
        session.runtime.last_error
    );
    let input = ManagedSessionInput {
        session_id: session.session.session_id.clone(),
    };
    fixture.app.stop_session(input.clone()).await.unwrap();
    let restored = fixture.app.start_session(input).await.unwrap();
    assert_eq!(restored.session.management, session.session.management);
    let events = tokio::fs::read_to_string(fixture.dir.path().join("events.jsonl"))
        .await
        .unwrap();
    assert!(events.contains("session/resume"));
    fixture.close().await;
    let fixture = Fixture::new(false, false, false).await;
    assert!(
        !fixture
            .app
            .check_agent_config(AgentConfigInput {
                agent_config_id: fixture.config.clone()
            })
            .await
            .unwrap()
            .ok
    );
    assert_eq!(
        fixture.create("missing").await.runtime.connection,
        SessionConnectionState::Failed
    );
    fixture.close().await;
    let fixture = Fixture::new(false, true, false).await;
    let session = fixture.create("http").await;
    assert_eq!(
        session.runtime.connection,
        SessionConnectionState::Connected
    );
    assert_eq!(
        session.runtime.capabilities.feedback_transport,
        Some(FeedbackTransport::Http)
    );
    fixture.close().await;
}

#[tokio::test]
async fn stopping_pi_during_open_revokes_scope_and_terminates_wrapper_descendants() {
    let fixture = Fixture::new(false, false, true).await;
    let mut config = fixture
        .store
        .get_agent_config(&fixture.config)
        .await
        .unwrap();
    config.env.insert("FIXTURE_BLOCK_OPEN".into(), "1".into());
    fixture.store.save_agent_config(config).await.unwrap();
    let cwd = fixture.dir.path().join("pending-open");
    tokio::fs::create_dir(&cwd).await.unwrap();
    let session = fixture
        .store
        .create_managed_session(NewManagedSession {
            session_id: "pending-pi-session".into(),
            agent_config_id: fixture.config.clone(),
            cwd: cwd.to_string_lossy().into(),
            title: "Opening Pi".into(),
            created_at: "2026-09-05T00:00:00Z".into(),
        })
        .await
        .unwrap();
    let input = ManagedSessionInput {
        session_id: session.session_id.clone(),
    };
    let app = fixture.app.clone();
    let task_input = input.clone();
    let start = tokio::spawn(async move { app.start_session(task_input).await });
    let descendant = cwd.join("heartbeat");
    tokio::time::timeout(Duration::from_secs(10), async {
        while !descendant.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let endpoint = fixture.provider.bind(&session).await.unwrap();
    fixture.app.stop_session(input).await.unwrap();
    assert!(start.await.unwrap().is_err());
    assert_revoked(endpoint).await;
    let final_heartbeat = heartbeat(&descendant).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(heartbeat(&descendant).await, final_heartbeat);
    fixture.close().await;
}
