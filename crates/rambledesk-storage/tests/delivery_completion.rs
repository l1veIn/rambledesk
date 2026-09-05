use async_trait::async_trait;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

struct Driver(Arc<Connection>);

#[derive(Default)]
struct Connection {
    closed: AtomicBool,
    fail_prompt: AtomicBool,
    prompts: Mutex<Vec<String>>,
}

#[async_trait]
impl AgentSessionConnection for Connection {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
    async fn prompt(&self, text: &str) -> Result<String, AgentDriverError> {
        self.prompts.lock().unwrap().push(text.into());
        if self.fail_prompt.load(Ordering::SeqCst) {
            Err(AgentDriverError::new(
                "fixture disconnected after receiving prompt",
            ))
        } else {
            Ok("EndTurn".into())
        }
    }
    async fn cancel(&self) -> Result<(), AgentDriverError> {
        Ok(())
    }
    async fn respond_permission(&self, _: &str, _: Option<&str>) -> Result<(), AgentDriverError> {
        Ok(())
    }
    async fn stop(&self) -> Result<(), AgentDriverError> {
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[async_trait]
impl AgentSessionDriver for Driver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        Ok(StartedAgentSession {
            connection: self.0.clone(),
            remote_session_id: format!("remote-{}", launch.session.session_id),
            capabilities: AgentSessionCapabilities::default(),
        })
    }
    async fn check(&self, _: &AgentConfig) -> Result<AgentSessionCapabilities, AgentDriverError> {
        Ok(AgentSessionCapabilities::default())
    }
}

struct Fixture {
    _directory: tempfile::TempDir,
    store: Arc<SqliteFeedbackStore>,
    sql: sqlx::SqlitePool,
    app: SessionApplication,
    connection: Arc<Connection>,
    session_id: String,
    requests: Vec<String>,
}

impl Fixture {
    async fn new(fail_prompt: bool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("state.sqlite");
        let store = Arc::new(SqliteFeedbackStore::connect(&database).await.unwrap());
        let sql = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(database),
        )
        .await
        .unwrap();
        let connection = Arc::new(Connection::default());
        connection.fail_prompt.store(fail_prompt, Ordering::SeqCst);
        let app = SessionApplication::new(
            store.clone(),
            store.clone(),
            Arc::new(Driver(connection.clone())),
        )
        .with_deliveries(store.clone());
        let config = app
            .save_agent_config(SaveAgentConfigInput {
                catalog_id: None,
                id: None,
                name: "Fixture".into(),
                host_id: "fixture".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: "fixture".into(),
                args: vec![],
                env: BTreeMap::new(),
            })
            .await
            .unwrap();
        let session = app
            .create_session(CreateManagedSessionInput {
                agent_config_id: config.id,
                cwd: directory.path().to_string_lossy().into_owned(),
                title: "Completion retry".into(),
            })
            .await
            .unwrap();
        let session_id = session.session.session_id;
        let mut requests = vec![];
        for _ in 0..2 {
            let request_id = uuid::Uuid::now_v7().to_string();
            store
                .create_or_get_request(NewFeedbackRequest {
                    request_id: request_id.clone(),
                    host_session_record_id: session_id.clone(),
                    managed_session_id: Some(session_id.clone()),
                    host_id: "fixture".into(),
                    host_session_id: session_id.clone(),
                    title: "Approve".into(),
                    what_happened: "Review".into(),
                    actions: vec![],
                    context_refs: vec![],
                    attachments: vec![],
                    source_hint: None,
                    allow_finish: true,
                    final_summary: Some("Done".into()),
                    created_at: "2026-09-04T02:00:00Z".into(),
                })
                .await
                .unwrap();
            store
                .approve_request(&request_id, "2026-09-04T02:00:00Z")
                .await
                .unwrap();
            requests.push(request_id);
        }
        // Failure is at the actual SQLite completion write, after a prompt result
        // exists. Claiming and reading the queue remain available.
        sqlx::query("CREATE TRIGGER completion_unavailable BEFORE UPDATE OF state ON feedback_deliveries WHEN OLD.state='sending' AND NEW.state IN ('delivered','uncertain') BEGIN SELECT RAISE(ABORT,'completion unavailable'); END")
            .execute(&sql).await.unwrap();
        app.start_delivery_worker().await.unwrap();
        Self {
            _directory: directory,
            store,
            sql,
            app,
            connection,
            session_id,
            requests,
        }
    }

    async fn deliveries(&self) -> Vec<FeedbackDelivery> {
        self.store
            .list_session_deliveries(&self.session_id)
            .await
            .unwrap()
    }

    async fn wait_for_failed_completion(&self) {
        eventually(|| async {
            let snapshot = self
                .app
                .get_session(ManagedSessionInput {
                    session_id: self.session_id.clone(),
                })
                .await
                .unwrap();
            snapshot.runtime.activity == SessionActivityState::Idle
                && snapshot.runtime.last_error.is_some()
                && snapshot.deliveries[0].state == FeedbackDeliveryState::Sending
        })
        .await;
    }

    async fn repair_storage(&self) {
        sqlx::query("DROP TRIGGER completion_unavailable")
            .execute(&self.sql)
            .await
            .unwrap();
    }

    async fn close(self) {
        self.app.shutdown().await.unwrap();
        self.sql.close().await;
        self.store.close().await;
    }
}

async fn eventually<F, Fut>(mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition().await {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("condition did not converge");
}

#[tokio::test]
async fn transient_completion_write_retries_same_attempt_before_next_queued_prompt() {
    let fixture = Fixture::new(false).await;
    fixture.wait_for_failed_completion().await;
    let attempt = fixture.deliveries().await[0].attempt_id.clone();
    // Allow more than one worker/retry interval while the database rejects the
    // write: the first turn is idle, but the second delivery must stay queued.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(fixture.connection.prompts.lock().unwrap().len(), 1);
    assert_eq!(
        fixture.deliveries().await[1].state,
        FeedbackDeliveryState::Pending
    );
    fixture.repair_storage().await;
    eventually(|| async {
        fixture
            .deliveries()
            .await
            .iter()
            .all(|item| item.state == FeedbackDeliveryState::Delivered)
    })
    .await;
    assert_eq!(fixture.deliveries().await[0].attempt_id, attempt);
    let prompts = fixture.connection.prompts.lock().unwrap().clone();
    assert_eq!(prompts.len(), 2);
    for request in &fixture.requests {
        assert_eq!(
            prompts.iter().filter(|text| text.contains(request)).count(),
            1
        );
    }
    fixture.close().await;
}

#[tokio::test]
async fn uncertain_completion_is_saved_after_storage_recovers_without_resending() {
    let fixture = Fixture::new(true).await;
    fixture.wait_for_failed_completion().await;
    let attempt = fixture.deliveries().await[0].attempt_id.clone();
    fixture.repair_storage().await;
    eventually(|| async {
        fixture.deliveries().await[0].state == FeedbackDeliveryState::Uncertain
    })
    .await;
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert_eq!(fixture.deliveries().await[0].attempt_id, attempt);
    assert_eq!(
        fixture.deliveries().await[1].state,
        FeedbackDeliveryState::Pending
    );
    assert_eq!(fixture.connection.prompts.lock().unwrap().len(), 1);
    fixture.close().await;
}

#[tokio::test]
async fn shutdown_leaves_unwritten_completion_for_startup_uncertain_recovery() {
    let fixture = Fixture::new(false).await;
    fixture.wait_for_failed_completion().await;
    fixture.app.shutdown().await.unwrap();
    fixture.repair_storage().await;
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert_eq!(
        fixture.deliveries().await[0].state,
        FeedbackDeliveryState::Sending
    );
    assert_eq!(fixture.connection.prompts.lock().unwrap().len(), 1);
    fixture
        .store
        .recover_interrupted_deliveries("2026-09-04T03:00:00Z")
        .await
        .unwrap();
    assert_eq!(
        fixture.deliveries().await[0].state,
        FeedbackDeliveryState::Uncertain
    );
    fixture.close().await;
}
