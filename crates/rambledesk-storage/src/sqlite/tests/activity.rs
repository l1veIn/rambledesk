use std::collections::{BTreeMap, BTreeSet};

use rambledesk_core::{
    AgentConfig, NewManagedSession, NewSessionActivity, SessionActivityKind,
    SessionActivityRepository, SessionProtocol, SessionRepository, SessionRepositoryError,
};

use super::*;

#[path = "structured_activity.rs"]
mod structured;
#[path = "activity_history.rs"]
mod history;

async fn setup() -> (TestWorkspace, SqliteFeedbackStore) {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    store
        .save_agent_config(AgentConfig {
            id: "config".into(),
            name: "Test".into(),
            host_id: "dsh".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "agent".into(),
            args: vec![],
            env: BTreeMap::new(),
            created_at: "2026-09-04T00:00:00Z".into(),
            updated_at: "2026-09-04T00:00:00Z".into(),
        })
        .await
        .unwrap();
    for id in ["one", "two"] {
        store
            .create_managed_session(NewManagedSession {
                session_id: id.into(),
                agent_config_id: "config".into(),
                cwd: workspace._temp.path().to_string_lossy().into_owned(),
                title: id.into(),
                created_at: "2026-09-04T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
    (workspace, store)
}

fn activity(id: &str, session_id: &str) -> NewSessionActivity {
    NewSessionActivity {
        id: id.into(),
        session_id: session_id.into(),
        turn_id: Some("turn-one".into()),
        kind: SessionActivityKind::AgentMessage,
        text: "Partial reply".into(),
        content: None,
        tool_call_id: None,
        created_at: "2026-09-04T01:00:00Z".into(),
    }
}

#[tokio::test]
async fn activity_sequence_is_per_session_and_survives_restart() {
    let (workspace, store) = setup().await;
    assert_eq!(
        store
            .append_activity(activity("first", "one"))
            .await
            .unwrap()
            .sequence,
        1
    );
    assert_eq!(
        store
            .append_activity(activity("other", "two"))
            .await
            .unwrap()
            .sequence,
        1
    );
    assert_eq!(
        store
            .append_activity(activity("second", "one"))
            .await
            .unwrap()
            .sequence,
        2
    );
    let record = store.get_session("one").await.unwrap();
    assert_eq!(record.updated_at, "2026-09-04T01:00:00Z");
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        store
            .append_activity(activity("third", "one"))
            .await
            .unwrap()
            .sequence,
        3
    );
    let page = store
        .list_session_activity("one", Some(1), 1)
        .await
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, "second");
    let next = store
        .list_session_activity("one", Some(page[0].sequence), 10)
        .await
        .unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].id, "third");
}

#[tokio::test]
async fn concurrent_appends_allocate_unique_order_and_identical_retries_converge() {
    let (_workspace, store) = setup().await;
    let mut tasks = Vec::new();
    for index in 0..12 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store
                .append_activity(activity(&format!("activity-{index}"), "one"))
                .await
                .unwrap()
        }));
    }
    let mut sequences = BTreeSet::new();
    for task in tasks {
        sequences.insert(task.await.unwrap().sequence);
    }
    assert_eq!(sequences, (1..=12).collect());
    let (left, right) = tokio::join!(
        store.append_activity(activity("same", "one")),
        store.append_activity(activity("same", "one")),
    );
    assert_eq!(left.unwrap(), right.unwrap());
    assert_eq!(
        store
            .list_session_activity("one", None, 100)
            .await
            .unwrap()
            .len(),
        13
    );
}

#[tokio::test]
async fn duplicate_id_cannot_change_content_or_move_activity_between_sessions() {
    let (_workspace, store) = setup().await;
    let original = store
        .append_activity(activity("same", "one"))
        .await
        .unwrap();
    assert_eq!(
        store.append_activity(activity("same", "two")).await,
        Err(SessionRepositoryError::Conflict)
    );
    let mut changed = activity("same", "one");
    changed.text = "Different message".into();
    assert_eq!(
        store.append_activity(changed).await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store.list_session_activity("one", None, 10).await.unwrap(),
        vec![original]
    );
    assert!(
        store
            .list_session_activity("two", None, 10)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn streaming_text_replacement_checks_session_ownership_and_preserves_order() {
    let (workspace, store) = setup().await;
    let original = store
        .append_activity(activity("message", "one"))
        .await
        .unwrap();
    assert_eq!(
        store
            .update_activity_text("message", "two", "Wrong session")
            .await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    let updated = store
        .update_activity_text("message", "one", "Complete reply")
        .await
        .unwrap();
    assert_eq!(updated.sequence, original.sequence);
    assert_eq!(updated.created_at, original.created_at);
    assert_eq!(updated.turn_id, original.turn_id);
    assert_eq!(updated.text, "Complete reply");
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        store.list_session_activity("one", None, 10).await.unwrap(),
        vec![updated]
    );
}

#[tokio::test]
async fn activity_rejects_unknown_sessions_and_invalid_cursor_ranges() {
    let (_workspace, store) = setup().await;
    assert_eq!(
        store.append_activity(activity("missing", "missing")).await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    assert_eq!(
        store.list_session_activity("missing", None, 10).await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    assert_eq!(
        store.list_session_activity("one", None, 0).await,
        Err(SessionRepositoryError::InvalidInput)
    );
    assert_eq!(
        store.list_session_activity("one", None, 1001).await,
        Err(SessionRepositoryError::InvalidInput)
    );
    assert_eq!(
        store.list_session_activity("one", Some(u64::MAX), 10).await,
        Err(SessionRepositoryError::InvalidInput)
    );
}

#[tokio::test]
async fn deleting_one_session_cascades_only_its_activity() {
    let (_workspace, store) = setup().await;
    store
        .append_activity(activity("one-message", "one"))
        .await
        .unwrap();
    store
        .append_activity(activity("two-message", "two"))
        .await
        .unwrap();
    sqlx::query("DELETE FROM host_sessions WHERE id = 'one'")
        .execute(&store.pool)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session_activity")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(remaining, 1);
    assert_eq!(
        store.list_session_activity("two", None, 10).await.unwrap()[0].id,
        "two-message"
    );
}

#[tokio::test]
async fn recent_snapshot_shows_latest_window_in_conversation_order() {
    let (_workspace, store) = setup().await;
    for index in 1..=5 {
        store
            .append_activity(activity(&format!("message-{index}"), "one"))
            .await
            .unwrap();
    }
    store
        .append_activity(activity("other-session", "two"))
        .await
        .unwrap();
    store
        .update_activity_text("message-5", "one", "Latest streamed reply")
        .await
        .unwrap();
    let recent = store.list_recent_session_activity("one", 2).await.unwrap();
    assert_eq!(
        recent
            .iter()
            .map(|activity| activity.sequence)
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
    assert_eq!(recent[1].text, "Latest streamed reply");
    let oldest = store.list_session_activity("one", None, 2).await.unwrap();
    assert_eq!(
        oldest
            .iter()
            .map(|activity| activity.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        store.list_recent_session_activity("missing", 2).await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    assert_eq!(
        store.list_recent_session_activity("one", 0).await,
        Err(SessionRepositoryError::InvalidInput)
    );
    assert_eq!(
        store.list_recent_session_activity("one", 1001).await,
        Err(SessionRepositoryError::InvalidInput)
    );
}
