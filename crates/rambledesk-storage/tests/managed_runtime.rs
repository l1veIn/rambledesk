use async_trait::async_trait;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

#[derive(Default)]
struct FakeDriver {
    starts: Mutex<Vec<(String, Option<String>)>>,
    connections: Mutex<Vec<Arc<FakeConnection>>>,
    fail: AtomicBool,
    hang: AtomicBool,
    starting: tokio::sync::Notify,
    starting_count: Arc<AtomicUsize>,
}

struct StartGuard(Arc<AtomicUsize>);
impl Drop for StartGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct FakeConnection {
    closed: AtomicBool,
    stops: AtomicUsize,
}

#[async_trait]
impl AgentSessionConnection for FakeConnection {
    async fn prompt(&self, _: &str) -> Result<String, AgentDriverError> {
        Ok("EndTurn".into())
    }
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
    async fn stop(&self) -> Result<(), AgentDriverError> {
        self.closed.store(true, Ordering::SeqCst);
        self.stops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[async_trait]
impl AgentSessionDriver for FakeDriver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        self.starting_count.fetch_add(1, Ordering::SeqCst);
        let _guard = StartGuard(self.starting_count.clone());
        self.starting.notify_one();
        if self.hang.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(AgentDriverError::new("fixture launch failed"));
        }
        let SessionManagement::Managed {
            remote_session_id, ..
        } = &launch.session.management
        else {
            panic!("external launch")
        };
        self.starts
            .lock()
            .unwrap()
            .push((launch.session.session_id.clone(), remote_session_id.clone()));
        let connection = Arc::new(FakeConnection::default());
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
            },
        })
    }
    async fn check(&self, _: &AgentConfig) -> Result<AgentSessionCapabilities, AgentDriverError> {
        Ok(AgentSessionCapabilities::default())
    }
}

async fn setup() -> (
    tempfile::TempDir,
    Arc<SqliteFeedbackStore>,
    Arc<FakeDriver>,
    SessionApplication,
    String,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteFeedbackStore::connect(&dir.path().join("database.sqlite"))
            .await
            .unwrap(),
    );
    let driver = Arc::new(FakeDriver::default());
    let application = SessionApplication::new(store.clone(), store.clone(), driver.clone());
    let config = application
        .save_agent_config(SaveAgentConfigInput {
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
    (dir, store, driver, application, config.id)
}

fn input(dir: &tempfile::TempDir, config: &str, title: &str) -> CreateManagedSessionInput {
    CreateManagedSessionInput {
        agent_config_id: config.into(),
        cwd: dir.path().to_string_lossy().into_owned(),
        title: title.into(),
    }
}
fn target(snapshot: &ManagedSessionSnapshot) -> ManagedSessionInput {
    ManagedSessionInput {
        session_id: snapshot.session.session_id.clone(),
    }
}

#[tokio::test]
async fn failed_start_retains_visible_identity_and_can_retry() {
    let (dir, store, driver, app, config) = setup().await;
    driver.fail.store(true, Ordering::SeqCst);
    let failed = app
        .create_session(input(&dir, &config, "Retry me"))
        .await
        .unwrap();
    assert_eq!(failed.runtime.connection, SessionConnectionState::Failed);
    assert!(
        failed
            .runtime
            .last_error
            .unwrap()
            .contains("fixture launch failed")
    );
    assert_eq!(store.list_managed_sessions().await.unwrap().len(), 1);
    driver.fail.store(false, Ordering::SeqCst);
    let ready = app
        .start_session(ManagedSessionInput {
            session_id: failed.session.session_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(ready.session.session_id, failed.session.session_id);
    assert_eq!(ready.runtime.connection, SessionConnectionState::Connected);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn duplicate_start_is_idempotent_and_two_sessions_own_independent_instances() {
    let (dir, store, driver, app, config) = setup().await;
    let first = app
        .create_session(input(&dir, &config, "One"))
        .await
        .unwrap();
    let second = app
        .create_session(input(&dir, &config, "Two"))
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        app.start_session(target(&first)),
        app.start_session(target(&first))
    );
    assert_eq!(
        a.unwrap().runtime.instance_id,
        b.unwrap().runtime.instance_id
    );
    assert_ne!(first.runtime.instance_id, second.runtime.instance_id);
    assert_eq!(driver.starts.lock().unwrap().len(), 2);
    app.stop_session(target(&first)).await.unwrap();
    assert_eq!(
        app.get_session(target(&second))
            .await
            .unwrap()
            .runtime
            .connection,
        SessionConnectionState::Connected
    );
    assert_eq!(store.list_managed_sessions().await.unwrap().len(), 2);
    let resumed = app.start_session(target(&first)).await.unwrap();
    assert_ne!(resumed.runtime.instance_id, first.runtime.instance_id);
    assert_eq!(resumed.session.management, first.session.management);
    assert!(driver.starts.lock().unwrap()[2].1.is_some());
    app.shutdown().await.unwrap();
    assert!(
        driver
            .connections
            .lock()
            .unwrap()
            .iter()
            .all(|connection| connection.is_closed())
    );
    store.close().await;
}

#[tokio::test]
async fn owner_shutdown_interrupts_in_progress_launch_and_rejects_new_work() {
    let (dir, store, driver, app, config) = setup().await;
    driver.hang.store(true, Ordering::SeqCst);
    let creation = {
        let app = app.clone();
        let input = input(&dir, &config, "Interrupted startup");
        tokio::spawn(async move { app.create_session(input).await })
    };
    driver.starting.notified().await;
    app.shutdown().await.unwrap();
    let snapshot = creation.await.unwrap().unwrap();
    assert_ne!(
        snapshot.runtime.connection,
        SessionConnectionState::Connected
    );
    assert_eq!(driver.starting_count.load(Ordering::SeqCst), 0);
    assert!(matches!(
        app.create_session(input(&dir, &config, "Too late")).await,
        Err(SessionError::ShuttingDown)
    ));
    assert_eq!(store.list_managed_sessions().await.unwrap().len(), 1);
    store.close().await;
}
