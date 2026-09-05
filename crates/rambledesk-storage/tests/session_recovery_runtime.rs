use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use tokio::sync::Notify;

struct Driver {
    store: Arc<SqliteFeedbackStore>,
    connections: Mutex<Vec<Arc<Connection>>>,
    restored: Mutex<Vec<Option<String>>>,
    fail: AtomicBool,
    starts: AtomicUsize,
}
struct Connection {
    store: Arc<SqliteFeedbackStore>,
    id: String,
    instance: String,
    closed: AtomicBool,
    ignore_cancel: AtomicBool,
    started: Notify,
    finish: Notify,
}

#[async_trait]
impl AgentSessionConnection for Connection {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
    async fn prompt(&self, text: &str) -> Result<String, AgentDriverError> {
        let checkpoint = self.store.get_session_recovery(&self.id).await.unwrap();
        assert_eq!(checkpoint.run_id.as_deref(), Some(self.instance.as_str()));
        assert!(
            checkpoint.active_turn_id.is_some(),
            "checkpoint exists before protocol prompt"
        );
        self.started.notify_one();
        if text == "hold" {
            self.finish.notified().await;
        }
        Ok("EndTurn".into())
    }
    async fn cancel(&self) -> Result<(), AgentDriverError> {
        if !self.ignore_cancel.load(Ordering::SeqCst) {
            self.finish.notify_one();
        }
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
        let checkpoint = self
            .store
            .get_session_recovery(&launch.session.session_id)
            .await
            .unwrap();
        assert_eq!(
            checkpoint.status,
            SessionRecoveryStatus::Unclosed,
            "checkpoint exists before process launch"
        );
        self.starts.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(AgentDriverError::new("fixture failed"));
        }
        let SessionManagement::Managed {
            remote_session_id, ..
        } = &launch.session.management
        else {
            unreachable!()
        };
        self.restored
            .lock()
            .unwrap()
            .push(remote_session_id.clone());
        let connection = Arc::new(Connection {
            store: self.store.clone(),
            id: launch.session.session_id.clone(),
            instance: checkpoint.run_id.unwrap(),
            closed: AtomicBool::new(false),
            ignore_cancel: AtomicBool::new(false),
            started: Notify::new(),
            finish: Notify::new(),
        });
        self.connections.lock().unwrap().push(connection.clone());
        Ok(StartedAgentSession {
            connection,
            remote_session_id: remote_session_id
                .clone()
                .unwrap_or_else(|| format!("remote-{}", launch.session.session_id)),
            capabilities: AgentSessionCapabilities {
                load_session: true,
                resume_session: true,
                http_mcp: true,
                feedback_transport: Some(FeedbackTransport::Http),
                prompt: AgentPromptCapabilities::default(),
            },
        })
    }
    async fn check(&self, _: &AgentConfig) -> Result<AgentSessionCapabilities, AgentDriverError> {
        Ok(AgentSessionCapabilities::default())
    }
}

#[derive(Default)]
struct Provider {
    bindings: Mutex<HashSet<String>>,
}
#[async_trait]
impl ManagedFeedbackProvider for Provider {
    async fn bind(
        &self,
        session: &SessionRecord,
    ) -> Result<ManagedFeedbackEndpoint, AgentDriverError> {
        self.bindings
            .lock()
            .unwrap()
            .insert(session.session_id.clone());
        Ok(ManagedFeedbackEndpoint {
            url: "http://127.0.0.1/fixture".into(),
            bearer_token: "fixture-token".into(),
        })
    }
    async fn revoke(&self, id: &str) -> Result<(), AgentDriverError> {
        self.bindings.lock().unwrap().remove(id);
        Ok(())
    }
}

fn runtime(
    store: Arc<SqliteFeedbackStore>,
    provider: Arc<Provider>,
) -> (SessionApplication, Arc<Driver>) {
    let driver = Arc::new(Driver {
        store: store.clone(),
        connections: Mutex::new(vec![]),
        restored: Mutex::new(vec![]),
        fail: AtomicBool::new(false),
        starts: AtomicUsize::new(0),
    });
    let app = SessionApplication::new(store.clone(), store.clone(), driver.clone())
        .with_recovery(store.clone())
        .with_deliveries(store)
        .with_feedback_provider(provider);
    (app, driver)
}

async fn setup() -> (
    tempfile::TempDir,
    Arc<SqliteFeedbackStore>,
    SessionApplication,
    Arc<Driver>,
    Arc<Provider>,
    String,
) {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteFeedbackStore::connect(&directory.path().join("database.sqlite"))
            .await
            .unwrap(),
    );
    let provider = Arc::new(Provider::default());
    let (app, driver) = runtime(store.clone(), provider.clone());
    let config = app
        .save_agent_config(SaveAgentConfigInput {
            catalog_id: None,
            id: None,
            name: "Fixture".into(),
            host_id: "dsh".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "fixture".into(),
            args: vec![],
            env: BTreeMap::new(),
        })
        .await
        .unwrap();
    (directory, store, app, driver, provider, config.id)
}
fn create(directory: &tempfile::TempDir, config: &str) -> CreateManagedSessionInput {
    CreateManagedSessionInput {
        agent_config_id: config.into(),
        cwd: directory.path().to_string_lossy().into_owned(),
        title: "Recovery".into(),
    }
}
fn target(snapshot: &ManagedSessionSnapshot) -> ManagedSessionInput {
    ManagedSessionInput {
        session_id: snapshot.session.session_id.clone(),
    }
}

#[tokio::test]
async fn checkpoints_precede_launch_and_prompt_then_finish_only_after_terminal_activity() {
    let (directory, store, app, driver, _provider, config) = setup().await;
    let session = app
        .create_session(create(&directory, &config))
        .await
        .unwrap();
    assert_eq!(
        session.recovery.as_ref().unwrap().run_id,
        session.runtime.instance_id
    );
    app.send_prompt(SendManagedPromptInput {
        session_id: session.session.session_id.clone(),
        text: "finish".into(),
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = app.get_session(target(&session)).await.unwrap();
            if snapshot.runtime.activity == SessionActivityState::Idle {
                assert!(snapshot.recovery.unwrap().active_turn_id.is_none());
                assert!(
                    snapshot
                        .activities
                        .iter()
                        .any(|row| row.text == "Turn finished: EndTurn")
                );
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let stopped = app.stop_session(target(&session)).await.unwrap();
    assert_eq!(
        stopped.recovery.unwrap().status,
        SessionRecoveryStatus::Stopped
    );
    driver.fail.store(true, Ordering::SeqCst);
    let failed = app.start_session(target(&session)).await;
    assert!(failed.is_err());
    assert_eq!(
        store
            .get_session_recovery(&session.session.session_id)
            .await
            .unwrap()
            .status,
        SessionRecoveryStatus::Interrupted
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn background_eof_reconciliation_revokes_only_that_scope_and_late_turn_cannot_close_replacement()
 {
    let (directory, store, app, driver, provider, config) = setup().await;
    app.start_delivery_worker().await.unwrap();
    let first = app
        .create_session(create(&directory, &config))
        .await
        .unwrap();
    let second = app
        .create_session(create(&directory, &config))
        .await
        .unwrap();
    let old = driver.connections.lock().unwrap()[0].clone();
    app.send_prompt(SendManagedPromptInput {
        session_id: first.session.session_id.clone(),
        text: "hold".into(),
    })
    .await
    .unwrap();
    old.started.notified().await;
    old.closed.store(true, Ordering::SeqCst);
    // Poll the durable repository, not get_session: the background owner itself
    // must notice EOF when there is no UI reading the affected session.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if store
                .get_session_recovery(&first.session.session_id)
                .await
                .unwrap()
                .status
                == SessionRecoveryStatus::Interrupted
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        !provider
            .bindings
            .lock()
            .unwrap()
            .contains(&first.session.session_id)
    );
    assert!(
        provider
            .bindings
            .lock()
            .unwrap()
            .contains(&second.session.session_id)
    );
    let interrupted = app.get_session(target(&first)).await.unwrap();
    assert_eq!(
        interrupted.runtime.connection,
        SessionConnectionState::Disconnected
    );
    assert!(interrupted.recovery.unwrap().interrupted_turn_id.is_some());
    assert_eq!(
        app.get_session(target(&second))
            .await
            .unwrap()
            .runtime
            .connection,
        SessionConnectionState::Connected
    );
    let replacement = app.start_session(target(&first)).await.unwrap();
    old.finish.notify_one();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let after = store
        .get_session_recovery(&first.session.session_id)
        .await
        .unwrap();
    assert_eq!(after.status, SessionRecoveryStatus::Unclosed);
    assert_eq!(after.run_id, replacement.runtime.instance_id);
    assert_ne!(replacement.runtime.instance_id, first.runtime.instance_id);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn runtime_restart_closes_unfinished_checkpoint_once_and_never_launches_implicitly() {
    let (directory, store, app, driver, _provider, config) = setup().await;
    let previous = app
        .create_session(create(&directory, &config))
        .await
        .unwrap();
    store
        .begin_turn(
            &previous.session.session_id,
            previous.runtime.instance_id.as_deref().unwrap(),
            "interrupted-turn",
            "2026-09-04T12:00:00Z",
        )
        .await
        .unwrap();
    driver.connections.lock().unwrap()[0]
        .closed
        .store(true, Ordering::SeqCst);
    // Simulate an owner crash: its connection disappears without shutdown or a
    // close checkpoint, then a separate application opens the same persisted DB.
    drop(app);
    drop(driver);
    store.close().await;
    drop(store);
    let reopened = Arc::new(
        SqliteFeedbackStore::connect(&directory.path().join("database.sqlite"))
            .await
            .unwrap(),
    );
    let (fresh, driver) = runtime(reopened.clone(), Arc::new(Provider::default()));
    fresh.recover_runtime().await.unwrap();
    let recovered = fresh.get_session(target(&previous)).await.unwrap();
    assert_eq!(driver.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        recovered.runtime.connection,
        SessionConnectionState::Stopped
    );
    assert_eq!(
        recovered.recovery.as_ref().unwrap().status,
        SessionRecoveryStatus::Interrupted
    );
    assert_eq!(
        recovered
            .recovery
            .as_ref()
            .unwrap()
            .interrupted_turn_id
            .as_deref(),
        Some("interrupted-turn")
    );
    assert_eq!(
        recovered
            .activities
            .iter()
            .filter(|row| row.text == "Turn interrupted before completion.")
            .count(),
        1
    );
    let resumed = fresh.start_session(target(&previous)).await.unwrap();
    assert_eq!(resumed.session.management, previous.session.management);
    assert!(driver.restored.lock().unwrap()[0].is_some());
    fresh.recover_runtime().await.unwrap();
    assert_eq!(
        reopened
            .get_session_recovery(&previous.session.session_id)
            .await
            .unwrap()
            .status,
        SessionRecoveryStatus::Unclosed
    );
    fresh.shutdown().await.unwrap();
    reopened.close().await;
}

#[tokio::test]
async fn delayed_cancel_watchdog_cannot_stop_a_replacement_or_a_later_turn() {
    let (directory, store, app, driver, _provider, config) = setup().await;
    let first = app
        .create_session(create(&directory, &config))
        .await
        .unwrap();
    let second = app
        .create_session(create(&directory, &config))
        .await
        .unwrap();
    let old = driver.connections.lock().unwrap().clone();
    for (session, connection) in [&first, &second].into_iter().zip(&old) {
        connection.ignore_cancel.store(true, Ordering::SeqCst);
        app.send_prompt(SendManagedPromptInput {
            session_id: session.session.session_id.clone(),
            text: "hold".into(),
        })
        .await
        .unwrap();
        connection.started.notified().await;
        app.cancel_prompt(target(session)).await.unwrap();
    }
    app.stop_session(target(&first)).await.unwrap();
    let replacement = app.start_session(target(&first)).await.unwrap();
    old[0].finish.notify_one();
    old[1].finish.notify_one();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if app
                .get_session(target(&second))
                .await
                .unwrap()
                .runtime
                .activity
                == SessionActivityState::Idle
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    for session in [&first, &second] {
        app.send_prompt(SendManagedPromptInput {
            session_id: session.session.session_id.clone(),
            text: "hold".into(),
        })
        .await
        .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(5200)).await;
    for session in [&first, &second] {
        let current = app.get_session(target(session)).await.unwrap();
        assert_eq!(
            current.runtime.connection,
            SessionConnectionState::Connected
        );
        assert_eq!(current.runtime.activity, SessionActivityState::Running);
        assert!(current.runtime.last_error.is_none());
    }
    assert_eq!(
        app.get_session(target(&first))
            .await
            .unwrap()
            .runtime
            .instance_id,
        replacement.runtime.instance_id
    );
    for connection in driver.connections.lock().unwrap().iter() {
        connection.finish.notify_one();
    }
    app.shutdown().await.unwrap();
    store.close().await;
}
