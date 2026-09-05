use std::collections::BTreeMap;

use rambledesk_core::{
    AgentConfig, NewManagedSession, SessionActivityRepository, SessionManagement, SessionProtocol,
    SessionRecoveryRepository, SessionRecoveryStatus as RecoveryStatus, SessionRepository,
    SessionRepositoryError, SessionRunEnd,
};
use sqlx::migrate::Migrate;

use super::*;

const NOW: &str = "2026-09-04T04:00:00Z";
const LATER: &str = "2026-09-04T05:00:00Z";

async fn setup() -> (TestWorkspace, SqliteFeedbackStore) {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    seed(&workspace, &store).await;
    (workspace, store)
}

async fn seed(workspace: &TestWorkspace, store: &SqliteFeedbackStore) {
    store
        .save_agent_config(AgentConfig {
            catalog_id: None,
            id: "config".into(),
            name: "Test".into(),
            host_id: "dsh".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "agent".into(),
            args: vec![],
            env: BTreeMap::new(),
            created_at: NOW.into(),
            updated_at: NOW.into(),
        })
        .await
        .unwrap();
    for id in ["active", "idle", "stopped", "new"] {
        store
            .create_managed_session(NewManagedSession {
                session_id: id.into(),
                agent_config_id: "config".into(),
                cwd: workspace._temp.path().to_string_lossy().into_owned(),
                title: id.into(),
                created_at: NOW.into(),
            })
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn checkpoints_distinguish_never_started_from_clean_stop_and_launch_failure() {
    let (_workspace, store) = setup().await;
    let fresh = store.get_session_recovery("new").await.unwrap();
    assert_eq!(fresh.status, RecoveryStatus::NeverStarted);
    assert_eq!(fresh.run_id, None);
    let started = store.begin_run("active", "first-run", NOW).await.unwrap();
    assert_eq!(started.status, RecoveryStatus::Unclosed);
    assert_eq!(
        store.begin_run("active", "first-run", LATER).await.unwrap(),
        started
    );
    let failed = store
        .close_run(
            "active",
            "first-run",
            SessionRunEnd::Interrupted,
            Some("Launch failed"),
            LATER,
        )
        .await
        .unwrap();
    assert_eq!(failed.status, RecoveryStatus::Interrupted);
    assert_eq!(failed.last_error.as_deref(), Some("Launch failed"));
    // No remote binding has been manufactured. A fresh launch attempt remains possible.
    assert!(matches!(
        store.get_session("active").await.unwrap().management,
        SessionManagement::Managed {
            remote_session_id: None,
            ..
        }
    ));
    store.begin_run("active", "retry-run", LATER).await.unwrap();
    let stopped = store
        .close_run("active", "retry-run", SessionRunEnd::Stopped, None, LATER)
        .await
        .unwrap();
    assert_eq!(stopped.status, RecoveryStatus::Stopped);
    assert_eq!(stopped.interrupted_turn_id, None);
}

#[tokio::test]
async fn restart_marks_unclosed_runs_and_unfinished_turns_without_changing_agent_identity() {
    let (workspace, store) = setup().await;
    store
        .bind_remote_session("active", "original-agent-context", NOW)
        .await
        .unwrap();
    store.begin_run("active", "active-run", NOW).await.unwrap();
    store
        .begin_turn("active", "active-run", "unfinished-turn", NOW)
        .await
        .unwrap();
    store.begin_run("idle", "idle-run", NOW).await.unwrap();
    store
        .begin_run("stopped", "stopped-run", NOW)
        .await
        .unwrap();
    store
        .close_run("stopped", "stopped-run", SessionRunEnd::Stopped, None, NOW)
        .await
        .unwrap();
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let recovered = store.recover_open_runs(LATER).await.unwrap();
    assert_eq!(recovered.len(), 2);
    assert!(
        recovered
            .iter()
            .all(|checkpoint| checkpoint.status == RecoveryStatus::Interrupted)
    );
    let active = store.get_session_recovery("active").await.unwrap();
    assert_eq!(active.active_turn_id, None);
    assert_eq!(
        active.interrupted_turn_id.as_deref(),
        Some("unfinished-turn")
    );
    assert_eq!(
        store.get_session_recovery("stopped").await.unwrap().status,
        RecoveryStatus::Stopped
    );
    assert_eq!(
        store.get_session_recovery("new").await.unwrap().status,
        RecoveryStatus::NeverStarted
    );
    assert!(
        matches!(store.get_session("active").await.unwrap().management, SessionManagement::Managed { remote_session_id: Some(id), .. } if id == "original-agent-context")
    );
    let activities = store
        .list_recent_session_activity("active", 10)
        .await
        .unwrap();
    assert_eq!(activities.len(), 1);
    assert_eq!(activities[0].turn_id.as_deref(), Some("unfinished-turn"));
    assert_eq!(activities[0].text, "Turn interrupted before completion.");
    assert!(
        store
            .list_recent_session_activity("idle", 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(store.recover_open_runs(LATER).await.unwrap().is_empty());
    assert_eq!(
        store
            .list_recent_session_activity("active", 10)
            .await
            .unwrap(),
        activities
    );
}

#[tokio::test]
async fn old_runs_and_old_turn_callbacks_cannot_modify_a_replacement() {
    let (_workspace, store) = setup().await;
    store.begin_run("active", "old-run", NOW).await.unwrap();
    store
        .begin_turn("active", "old-run", "old-turn", NOW)
        .await
        .unwrap();
    assert_eq!(
        store.begin_run("active", "replacement", NOW).await,
        Err(SessionRepositoryError::Conflict)
    );
    store
        .close_run("active", "old-run", SessionRunEnd::Stopped, None, NOW)
        .await
        .unwrap();
    assert_eq!(
        store.begin_run("active", "old-run", NOW).await,
        Err(SessionRepositoryError::Conflict)
    );
    store.begin_run("active", "new-run", LATER).await.unwrap();
    store
        .begin_turn("active", "new-run", "new-turn", LATER)
        .await
        .unwrap();
    assert_eq!(
        store
            .close_run("active", "old-run", SessionRunEnd::Interrupted, None, LATER)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store
            .finish_turn("active", "old-run", "old-turn", LATER)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store
            .finish_turn("active", "new-run", "old-turn", LATER)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store
            .begin_turn("active", "new-run", "parallel-turn", LATER)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store
            .get_session_recovery("active")
            .await
            .unwrap()
            .active_turn_id
            .as_deref(),
        Some("new-turn")
    );
    store
        .finish_turn("active", "new-run", "new-turn", LATER)
        .await
        .unwrap();
    store
        .close_run("active", "new-run", SessionRunEnd::Stopped, None, LATER)
        .await
        .unwrap();
    let activities = store
        .list_recent_session_activity("active", 10)
        .await
        .unwrap();
    assert_eq!(activities.len(), 1);
    assert_eq!(activities[0].turn_id.as_deref(), Some("old-turn"));
}

#[tokio::test]
async fn explicit_stop_of_an_unfinished_turn_records_one_interruption() {
    let (_workspace, store) = setup().await;
    store.begin_run("active", "run", NOW).await.unwrap();
    let begun = store
        .begin_turn("active", "run", "turn", NOW)
        .await
        .unwrap();
    assert_eq!(
        store
            .begin_turn("active", "run", "turn", LATER)
            .await
            .unwrap(),
        begun
    );
    let stopped = store
        .close_run("active", "run", SessionRunEnd::Stopped, None, LATER)
        .await
        .unwrap();
    assert_eq!(stopped.status, RecoveryStatus::Stopped);
    assert_eq!(stopped.interrupted_turn_id.as_deref(), Some("turn"));
    assert_eq!(
        store
            .close_run("active", "run", SessionRunEnd::Stopped, None, LATER)
            .await
            .unwrap(),
        stopped
    );
    assert_eq!(
        store
            .list_recent_session_activity("active", 10)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn interruption_activity_and_checkpoint_are_one_transaction() {
    let (_workspace, store) = setup().await;
    store.begin_run("active", "run", NOW).await.unwrap();
    store
        .begin_turn("active", "run", "turn", NOW)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_interruption BEFORE INSERT ON session_activity BEGIN SELECT RAISE(ABORT,'test failure'); END").execute(&store.pool).await.unwrap();
    assert_eq!(
        store
            .close_run("active", "run", SessionRunEnd::Interrupted, None, LATER)
            .await,
        Err(SessionRepositoryError::Storage)
    );
    let checkpoint = store.get_session_recovery("active").await.unwrap();
    assert_eq!(checkpoint.status, RecoveryStatus::Unclosed);
    assert_eq!(checkpoint.active_turn_id.as_deref(), Some("turn"));
    sqlx::query("DROP TRIGGER reject_interruption")
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(store.recover_open_runs(LATER).await.unwrap().len(), 1);
    assert_eq!(
        store
            .list_recent_session_activity("active", 10)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn checkpoint_deletion_and_invalid_input_are_session_scoped() {
    let (_workspace, store) = setup().await;
    assert_eq!(
        store.get_session_recovery("missing").await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    assert_eq!(
        store.begin_run("active", "", NOW).await,
        Err(SessionRepositoryError::InvalidInput)
    );
    store.begin_run("active", "active-run", NOW).await.unwrap();
    store.begin_run("idle", "idle-run", NOW).await.unwrap();
    sqlx::query("DELETE FROM host_sessions WHERE id='active'")
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store.get_session_recovery("active").await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    assert_eq!(
        store
            .get_session_recovery("idle")
            .await
            .unwrap()
            .run_id
            .as_deref(),
        Some("idle-run")
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session_recovery")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn migration_treats_old_remote_bindings_as_interrupted_without_guessing_turns() {
    let workspace = TestWorkspace::new().await;
    tokio::fs::create_dir_all(workspace.database.parent().unwrap())
        .await
        .unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&workspace.database)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in MIGRATOR.iter().filter(|migration| migration.version <= 15) {
        connection.apply(migration).await.unwrap();
    }
    drop(connection);
    let store = SqliteFeedbackStore {
        pool,
        library_root: Arc::new(RwLock::new(workspace._temp.path().into())),
        publish_lock: Arc::new(tokio::sync::Mutex::new(())),
    };
    seed_legacy_managed_sessions(
        &store.pool,
        &["active", "idle", "stopped", "new"],
        workspace._temp.path(),
        NOW,
    )
    .await;
    sqlx::query(
        "UPDATE managed_sessions SET remote_session_id='existing-agent' WHERE session_id='active'",
    )
    .execute(&store.pool)
    .await
    .unwrap();
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let old = store.get_session_recovery("active").await.unwrap();
    assert_eq!(old.status, RecoveryStatus::Interrupted);
    assert_eq!(old.run_id, None);
    assert_eq!(old.interrupted_turn_id, None);
    assert_eq!(
        store.get_session_recovery("new").await.unwrap().status,
        RecoveryStatus::NeverStarted
    );
    assert!(store.recover_open_runs(LATER).await.unwrap().is_empty());
}
