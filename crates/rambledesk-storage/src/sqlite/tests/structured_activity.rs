use super::*;
use rambledesk_core::{
    MAX_TOOL_RAW_BYTES, SessionActivity, SessionActivityContent, SessionContentBlock,
    SessionToolCall,
};
use sqlx::migrate::Migrate;

#[tokio::test]
async fn version_sixteen_text_history_upgrades_without_rewriting_messages() {
    let workspace = TestWorkspace::new().await;
    tokio::fs::create_dir_all(workspace.database.parent().unwrap())
        .await
        .unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&workspace.database)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in MIGRATOR.iter().filter(|migration| migration.version <= 16) {
        connection.apply(migration).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO agent_configs VALUES ('config', 'Test', 'dsh', 'acp', 1, 'agent', '[]', '{}', 'now', 'now');
        INSERT INTO host_sessions (id, host_id, host_session_id, display_title, created_at, updated_at)
            VALUES ('one', 'dsh', 'one', 'Legacy', 'now', 'now');
        INSERT INTO managed_sessions VALUES ('one', 'acp', 'config', '/project', 'remote-one');
        INSERT INTO session_activity VALUES ('legacy', 'one', 1, 'turn', 'agent_message', 'Legacy transcript', NULL, 'now');")
        .execute(&mut *connection).await.unwrap();
    drop(connection);
    pool.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let rows = store.list_session_activity("one", None, 10).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "Legacy transcript");
    assert_eq!(rows[0].content, None);
    let wire = serde_json::to_value(&rows[0]).unwrap();
    assert!(wire.get("content").is_none());
    let decoded: SessionActivity = serde_json::from_value(wire).unwrap();
    assert_eq!(decoded, rows[0]);
    store.close().await;
}

#[tokio::test]
async fn structured_content_is_atomic_scoped_and_survives_reopen() {
    let (workspace, store) = setup().await;
    let mut first = activity("tool", "one");
    first.kind = SessionActivityKind::ToolCall;
    first.tool_call_id = Some("remote-tool".into());
    first.content = Some(SessionActivityContent::ToolCall {
        tool: SessionToolCall::new("remote-tool".into()),
    });
    let initial = store.append_activity(first.clone()).await.unwrap();
    assert_eq!(store.append_activity(first.clone()).await.unwrap(), initial);
    first.content = Some(SessionActivityContent::Message {
        blocks: vec![],
        truncated: false,
    });
    assert_eq!(
        store.append_activity(first).await.unwrap_err(),
        SessionRepositoryError::Conflict
    );
    let replacement = SessionActivityContent::Message {
        blocks: vec![
            SessionContentBlock::Text {
                text: "Rich output".into(),
            },
            SessionContentBlock::Resource {
                uri: "file:///project/context".into(),
                name: None,
                mime_type: Some("text/plain".into()),
                text: Some("Body".into()),
            },
        ],
        truncated: false,
    };
    assert_eq!(
        store
            .update_activity_content("tool", "two", "Wrong", &replacement)
            .await
            .unwrap_err(),
        SessionRepositoryError::SessionNotFound
    );
    let updated = store
        .update_activity_content("tool", "one", "Rich output", &replacement)
        .await
        .unwrap();
    assert_eq!(updated.sequence, initial.sequence);
    assert_eq!(updated.text, "Rich output");
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        store.list_session_activity("one", None, 10).await.unwrap(),
        vec![updated]
    );
    assert!(
        store
            .list_session_activity("two", None, 10)
            .await
            .unwrap()
            .is_empty()
    );
    store.close().await;
}

#[tokio::test]
async fn out_of_contract_payloads_are_rejected_without_partial_write() {
    let (_workspace, store) = setup().await;
    let mut item = activity("tool", "one");
    let mut tool = SessionToolCall::new("remote-tool".into());
    tool.raw_output = Some("x".repeat(MAX_TOOL_RAW_BYTES + 1));
    item.content = Some(SessionActivityContent::ToolCall { tool });
    assert_eq!(
        store.append_activity(item).await.unwrap_err(),
        SessionRepositoryError::InvalidInput
    );
    assert!(
        store
            .list_session_activity("one", None, 10)
            .await
            .unwrap()
            .is_empty()
    );
    store.close().await;
}
