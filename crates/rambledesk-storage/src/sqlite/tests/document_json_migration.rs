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
async fn foundation_contract_pragmas_are_enforced() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&store.pool)
        .await
        .expect("foreign_keys pragma");
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&store.pool)
        .await
        .expect("journal_mode pragma");
    assert_eq!(foreign_keys, 1);
    assert_eq!(journal_mode, "wal");
    store.close().await;
}
