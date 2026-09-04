use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

async fn setup() -> (
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
            id: None,
            name: "Fixture".into(),
            host_id: "fixture".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "node".into(),
            args: vec![
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/agent.mjs")
                    .to_string_lossy()
                    .into_owned(),
                "load".into(),
            ],
            env: BTreeMap::new(),
        })
        .await
        .unwrap();
    (dir, store, app, config.id)
}

async fn create(
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
fn id(snapshot: &ManagedSessionSnapshot) -> ManagedSessionInput {
    ManagedSessionInput {
        session_id: snapshot.session.session_id.clone(),
    }
}
async fn idle(app: &SessionApplication, session: ManagedSessionInput) -> ManagedSessionSnapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = app.get_session(session.clone()).await.unwrap();
            if snapshot.runtime.activity == SessionActivityState::Idle {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("prompt did not settle")
}

#[tokio::test]
async fn two_instances_stream_to_their_own_durable_activity_and_resume_without_replay() {
    let (dir, store, app, config) = setup().await;
    let first = create(&app, &dir, &config, "One").await;
    let second = create(&app, &dir, &config, "Two").await;
    let (a, b) = tokio::join!(
        app.send_prompt(SendManagedPromptInput {
            session_id: first.session.session_id.clone(),
            text: "ALPHA".into()
        }),
        app.send_prompt(SendManagedPromptInput {
            session_id: second.session.session_id.clone(),
            text: "BETA".into()
        })
    );
    a.unwrap();
    b.unwrap();
    let first = idle(&app, id(&first)).await;
    let second = idle(&app, id(&second)).await;
    for (snapshot, word, other) in [(&first, "ALPHA", "BETA"), (&second, "BETA", "ALPHA")] {
        let messages = snapshot
            .activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::AgentMessage)
            .collect::<Vec<_>>();
        assert_eq!(
            messages.len(),
            1,
            "stream chunks should form one durable message"
        );
        assert_eq!(messages[0].text, format!("fixture reply: {word}"));
        assert!(
            snapshot
                .activities
                .iter()
                .all(|row| row.session_id == snapshot.session.session_id
                    && !row.text.contains(other))
        );
    }
    app.shutdown().await.unwrap();
    store.close().await;
    let store = Arc::new(
        SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
            .await
            .unwrap(),
    );
    let app = SessionApplication::new(store.clone(), store.clone(), Arc::new(AcpSessionDriver));
    let before = app.get_session(id(&first)).await.unwrap();
    assert_eq!(before.runtime.connection, SessionConnectionState::Stopped);
    assert_eq!(before.activities, first.activities);
    let resumed = app.start_session(id(&first)).await.unwrap();
    assert_eq!(resumed.session.management, first.session.management);
    assert_eq!(
        resumed.activities, first.activities,
        "startup replay must not duplicate the timeline"
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn busy_session_rejects_duplicate_input_and_stopping_it_keeps_other_instance_connected() {
    let (dir, store, app, config) = setup().await;
    let first = create(&app, &dir, &config, "One").await;
    let second = create(&app, &dir, &config, "Two").await;
    app.send_prompt(SendManagedPromptInput {
        session_id: first.session.session_id.clone(),
        text: "wait".into(),
    })
    .await
    .unwrap();
    assert!(matches!(
        app.send_prompt(SendManagedPromptInput {
            session_id: first.session.session_id.clone(),
            text: "duplicate".into()
        })
        .await,
        Err(SessionError::Busy)
    ));
    app.stop_session(id(&first)).await.unwrap();
    assert_eq!(
        app.get_session(id(&second))
            .await
            .unwrap()
            .runtime
            .connection,
        SessionConnectionState::Connected
    );
    assert!(
        app.get_session(id(&first))
            .await
            .unwrap()
            .activities
            .iter()
            .all(|row| row.text != "duplicate")
    );
    app.shutdown().await.unwrap();
    store.close().await;
}
