use super::*;

#[tokio::test]
async fn structured_drafts_migration_adds_document_json_column() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('drafts')")
        .fetch_all(&store.pool)
        .await
        .expect("draft columns");
    assert!(
        columns.iter().any(|column| column == "document_json"),
        "structured draft migration must add document_json",
    );
    store.close().await;
}

#[tokio::test]
async fn markdown_only_drafts_load_with_null_document_json() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let request_id = Uuid::now_v7().to_string();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");

    sqlx::query(
        "INSERT INTO drafts (request_id, body_markdown, revision, updated_at) \
         VALUES (?1, ?2, 1, ?3)",
    )
    .bind(&request_id)
    .bind("Legacy markdown only.")
    .bind("2026-08-28T00:00:00Z")
    .execute(&store.pool)
    .await
    .expect("insert markdown-only draft");
    sqlx::query("UPDATE feedback_requests SET revision = 1, status = 'in_progress' WHERE id = ?1")
        .bind(&request_id)
        .execute(&store.pool)
        .await
        .expect("align request revision");

    let loaded = application
        .get_feedback_workspace(request_id)
        .await
        .expect("load markdown-only draft");
    assert_eq!(loaded.draft.body_markdown, "Legacy markdown only.");
    assert_eq!(loaded.draft.document_json, None);
    store.close().await;
}

#[tokio::test]
async fn save_draft_persists_document_json_and_markdown_together() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let request_id = Uuid::now_v7().to_string();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");

    let document_json = r#"{"schemaVersion":2,"doc":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Structured"}]}]}}"#;
    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            document_json: document_json.to_owned(),
            body_markdown: "Structured".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save structured draft");
    assert_eq!(saved.document_json.as_deref(), Some(document_json));
    assert_eq!(saved.body_markdown, "Structured");

    let loaded = application
        .get_feedback_workspace(request_id)
        .await
        .expect("reload structured draft");
    assert_eq!(loaded.draft.document_json.as_deref(), Some(document_json));
    assert_eq!(loaded.draft.body_markdown, "Structured");
    store.close().await;
}
