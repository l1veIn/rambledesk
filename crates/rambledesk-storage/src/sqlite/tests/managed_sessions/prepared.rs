use super::*;
use rambledesk_core::{NewSessionActivity, SessionActivityKind, SessionActivityRepository};

fn activity(session_id: &str, id: &str, kind: SessionActivityKind) -> NewSessionActivity {
    NewSessionActivity {
        id: id.into(),
        session_id: session_id.into(),
        turn_id: Some("first-turn".into()),
        kind,
        text: "First real prompt".into(),
        content: None,
        tool_call_id: None,
        created_at: CREATED.into(),
    }
}

#[tokio::test]
async fn promotion_and_first_message_roll_back_together_and_retry_without_duplicates() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    store.save_agent_config(config()).await.unwrap();
    let mut draft = session(&workspace, "prepared");
    draft.title.clear();
    store.create_prepared_session(draft).await.unwrap();
    store
        .bind_remote_session("prepared", "remote-prepared", CREATED)
        .await
        .unwrap();
    let user = activity("prepared", "user-id", SessionActivityKind::UserMessage);
    let invalid_turn = activity("prepared", "user-id", SessionActivityKind::Status);
    assert_eq!(
        store
            .promote_prepared_session(user.clone(), invalid_turn, "First real prompt")
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert!(store.get_session("prepared").await.unwrap().is_prepared());
    assert!(
        store
            .list_session_activity("prepared", None, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(store.list_managed_sessions().await.unwrap().is_empty());
    let turn = activity("prepared", "turn-id", SessionActivityKind::Status);
    store
        .promote_prepared_session(user.clone(), turn.clone(), "First real prompt")
        .await
        .unwrap();
    let published = store.get_session("prepared").await.unwrap();
    assert!(!published.is_prepared());
    assert_eq!(published.title, "First real prompt");
    assert_eq!(
        store
            .list_session_activity("prepared", None, 100)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        store
            .promote_prepared_session(user, turn, "Duplicate")
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store.get_session("prepared").await.unwrap().title,
        "First real prompt"
    );
}

#[tokio::test]
async fn promotion_preserves_a_user_title_and_discard_rejects_an_active_record() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    store.save_agent_config(config()).await.unwrap();
    store
        .create_prepared_session(session(&workspace, "prepared"))
        .await
        .unwrap();
    store
        .bind_remote_session("prepared", "remote-prepared", CREATED)
        .await
        .unwrap();
    store
        .promote_prepared_session(
            activity("prepared", "user-id", SessionActivityKind::UserMessage),
            activity("prepared", "turn-id", SessionActivityKind::Status),
            "Automatic fallback",
        )
        .await
        .unwrap();
    assert_eq!(
        store.get_session("prepared").await.unwrap().title,
        "Independent session"
    );
    assert_eq!(
        store.discard_prepared_session("prepared").await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(store.discard_stale_prepared_sessions().await.unwrap(), 0);
}
