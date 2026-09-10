use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

/// Each test keeps its own directory, SQLite store and supervised protocol child.
pub async fn setup(
    script: &str,
    mode: &str,
) -> (
    tempfile::TempDir,
    Arc<SqliteFeedbackStore>,
    SessionApplication,
    String,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
            .await
            .unwrap(),
    );
    let app = SessionApplication::new(store.clone(), store.clone(), Arc::new(AcpSessionDriver));
    let config = app
        .save_agent_config(SaveAgentConfigInput {
            catalog_id: None,
            id: None,
            name: format!("{script} fixture"),
            host_id: "fixture".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "node".into(),
            args: vec![
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/fixtures/{script}.mjs"))
                    .to_string_lossy()
                    .into_owned(),
                mode.into(),
            ],
            env: BTreeMap::new(),
        })
        .await
        .unwrap();
    (dir, store, app, config.id)
}

pub async fn create(
    app: &SessionApplication,
    dir: &tempfile::TempDir,
    config: &str,
    title: &str,
) -> ManagedSessionSnapshot {
    app.create_session(CreateManagedSessionInput {
        agent_config_id: config.into(),
        cwd: dir.path().to_string_lossy().into_owned(),
        title: title.into(),
    })
    .await
    .unwrap()
}

pub fn id(snapshot: &ManagedSessionSnapshot) -> ManagedSessionInput {
    ManagedSessionInput {
        session_id: snapshot.session.session_id.clone(),
    }
}

pub async fn wait_for(
    app: &SessionApplication,
    session: &ManagedSessionInput,
    ready: impl Fn(&ManagedSessionSnapshot) -> bool,
) -> ManagedSessionSnapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = app.get_session(session.clone()).await.unwrap();
            if ready(&snapshot) {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Agent fixture did not reach its expected state")
}
