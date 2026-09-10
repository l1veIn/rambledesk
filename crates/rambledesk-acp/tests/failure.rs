use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

struct Fixture {
    dir: tempfile::TempDir,
    store: Arc<SqliteFeedbackStore>,
    app: SessionApplication,
    config: String,
}
impl Fixture {
    async fn new(mode: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
                .await
                .unwrap(),
        );
        let app = SessionApplication::new(
            store.clone(),
            store.clone(),
            Arc::new(AcpSessionDriver::with_feedback_companion(
                std::env::current_exe().unwrap(),
            )),
        );
        let config = app
            .save_agent_config(SaveAgentConfigInput {
                id: None,
                catalog_id: None,
                name: "Failure fixture".into(),
                host_id: "fixture".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: "node".into(),
                args: vec![
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/failure.mjs")
                        .to_string_lossy()
                        .into(),
                    mode.into(),
                ],
                env: BTreeMap::from([
                    (
                        "FIXTURE_CALL_LOG".into(),
                        dir.path().join("calls.jsonl").to_string_lossy().into(),
                    ),
                    (
                        "FIXTURE_FAILURE_MARKER".into(),
                        dir.path().join("attempted").to_string_lossy().into(),
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
            config,
        }
    }
    async fn prepare(&self) -> ManagedSessionSnapshot {
        self.app
            .prepare_session(PrepareManagedSessionInput {
                agent_config_id: self.config.clone(),
                cwd: self.dir.path().to_string_lossy().into(),
            })
            .await
            .unwrap()
    }
    fn calls(&self, method: &str) -> Vec<Value> {
        std::fs::read_to_string(self.dir.path().join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|call| call["method"] == method)
            .collect()
    }
    async fn settled(&self, id: &ManagedSessionInput) -> ManagedSessionSnapshot {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let snapshot = self.app.get_session(id.clone()).await.unwrap();
                if snapshot.runtime.activity == SessionActivityState::Idle {
                    return snapshot;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn close(self) {
        let stopped = self.app.shutdown().await;
        self.store.close().await;
        stopped.expect("all owned ACP processes must close cleanly");
    }
}
fn assert_failure(
    snapshot: &ManagedSessionSnapshot,
    stage: AgentFailureStage,
    reason: AgentFailureReason,
) {
    let failure = snapshot
        .runtime
        .failure
        .as_ref()
        .expect("typed failure must be retained");
    assert_eq!(failure.stage, stage);
    assert_eq!(failure.reason, reason);
    assert!(
        !serde_json::to_string(snapshot)
            .unwrap()
            .contains("fixture-secret")
    );
}

#[tokio::test]
async fn ordinary_check_stops_after_initialize_and_auth_methods_do_not_prove_missing_login() {
    let fixture = Fixture::new("session-auth").await;
    let checked = fixture
        .app
        .check_agent_config(AgentConfigInput {
            agent_config_id: fixture.config.clone(),
        })
        .await
        .unwrap();
    assert_eq!(checked.connection, AgentCheckConnection::Connected);
    assert!(checked.ok, "{}", checked.message);
    assert!(
        checked
            .failure
            .as_ref()
            .is_none_or(|failure| failure.reason != AgentFailureReason::Authentication)
    );
    assert!(fixture.calls("session/new").is_empty());
    assert!(fixture.calls("session/prompt").is_empty());
    assert!(
        fixture
            .store
            .list_managed_sessions()
            .await
            .unwrap()
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn missing_launch_entry_is_a_launch_failure_without_authentication_advice() {
    let fixture = Fixture::new("session-auth").await;
    let mut config = fixture
        .store
        .get_agent_config(&fixture.config)
        .await
        .unwrap();
    config.command = fixture
        .dir
        .path()
        .join("missing-agent")
        .to_string_lossy()
        .into();
    let checked = AcpSessionDriver.check_connection(&config).await;
    assert_eq!(checked.connection, AgentCheckConnection::Failed);
    let failure = checked.failure.unwrap();
    assert_eq!(failure.stage, AgentFailureStage::Launch);
    assert_ne!(failure.reason, AgentFailureReason::Authentication);
    assert!(!failure.message.contains("credentials"));
    assert!(fixture.calls("initialize").is_empty());
    fixture.close().await;
}

#[tokio::test]
async fn prepare_auth_failure_retains_the_same_local_draft_for_explicit_retry() {
    let fixture = Fixture::new("session-auth").await;
    let failed = fixture.prepare().await;
    assert_failure(
        &failed,
        AgentFailureStage::Session,
        AgentFailureReason::Authentication,
    );
    assert!(failed.session.is_prepared());
    let retried = fixture
        .app
        .start_session(ManagedSessionInput {
            session_id: failed.session.session_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(retried.session.session_id, failed.session.session_id);
    assert_eq!(
        retried.runtime.connection,
        SessionConnectionState::Connected
    );
    assert!(retried.runtime.failure.is_none());
    assert!(
        fixture
            .store
            .list_managed_sessions()
            .await
            .unwrap()
            .is_empty(),
        "prepared sessions stay out of navigation"
    );
    assert_eq!(fixture.calls("session/new").len(), 2);
    assert!(fixture.calls("session/prompt").is_empty());
    fixture.close().await;
}

#[tokio::test]
async fn model_configuration_rejection_is_typed_and_a_confirmed_change_clears_it() {
    let fixture = Fixture::new("config-model").await;
    let snapshot = fixture.prepare().await;
    let id = ManagedSessionInput {
        session_id: snapshot.session.session_id.clone(),
    };
    let change = SetManagedSessionConfigInput {
        session_id: id.session_id.clone(),
        change: SessionConfigChange {
            config_id: snapshot.runtime.configuration.options[0].id.clone(),
            value: SessionConfigValue::Select {
                value: "two".into(),
            },
        },
    };
    assert!(
        fixture
            .app
            .set_session_config(change.clone())
            .await
            .is_err()
    );
    let failed = fixture.app.get_session(id.clone()).await.unwrap();
    assert_failure(
        &failed,
        AgentFailureStage::Configuration,
        AgentFailureReason::Model,
    );
    assert_eq!(failed.runtime.connection, SessionConnectionState::Connected);
    assert!(fixture.calls("session/prompt").is_empty());
    let fixed = fixture.app.set_session_config(change).await.unwrap();
    assert!(fixed.runtime.failure.is_none());
    fixture.close().await;
}

#[tokio::test]
async fn prompt_authentication_error_keeps_original_session_and_never_resends_automatically() {
    let fixture = Fixture::new("prompt-auth").await;
    let snapshot = fixture.prepare().await;
    let id = ManagedSessionInput {
        session_id: snapshot.session.session_id,
    };
    let prompt = SendManagedPromptInput {
        session_id: id.session_id.clone(),
        text: "Hello".into(),
    };
    fixture.app.send_prompt(prompt.clone()).await.unwrap();
    let failed = fixture.settled(&id).await;
    assert_failure(
        &failed,
        AgentFailureStage::Prompt,
        AgentFailureReason::Authentication,
    );
    assert_eq!(failed.runtime.connection, SessionConnectionState::Connected);
    assert_eq!(fixture.calls("session/prompt").len(), 1);
    fixture.app.send_prompt(prompt).await.unwrap();
    assert!(fixture.settled(&id).await.runtime.failure.is_none());
    assert_eq!(fixture.calls("session/new").len(), 1);
    assert_eq!(fixture.calls("session/prompt").len(), 2);
    fixture.close().await;
}

#[tokio::test]
async fn unknown_prompt_failure_never_guesses_login_from_agent_text() {
    let fixture = Fixture::new("prompt-unknown").await;
    let snapshot = fixture.prepare().await;
    let id = ManagedSessionInput {
        session_id: snapshot.session.session_id,
    };
    fixture
        .app
        .send_prompt(SendManagedPromptInput {
            session_id: id.session_id.clone(),
            text: "Hello".into(),
        })
        .await
        .unwrap();
    assert_failure(
        &fixture.settled(&id).await,
        AgentFailureStage::Prompt,
        AgentFailureReason::Unknown,
    );
    fixture.close().await;
}
