use std::collections::BTreeMap;

use rambledesk_core::{AgentConfig, NewManagedSession, SessionProtocol, SessionRepository};

use super::*;

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
    for id in ["managed-one", "managed-two"] {
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

fn request(request_id: &str, managed_session_id: Option<&str>) -> NewFeedbackRequest {
    let session_id = managed_session_id.unwrap_or("external-session");
    NewFeedbackRequest {
        request_id: request_id.into(),
        host_session_record_id: session_id.into(),
        managed_session_id: managed_session_id.map(str::to_owned),
        host_id: "dsh".into(),
        host_session_id: session_id.into(),
        title: "Review".into(),
        what_happened: "Scoped feedback".into(),
        actions: vec![],
        context_refs: vec![],
        attachments: vec![],
        source_hint: None,
        allow_finish: false,
        final_summary: None,
        created_at: "2026-09-04T01:00:00Z".into(),
    }
}

#[tokio::test]
async fn continuation_routing_uses_the_stored_marker_for_identical_host_labels() {
    let (_workspace, store) = setup().await;
    let managed = Uuid::now_v7().to_string();
    let external = Uuid::now_v7().to_string();
    store
        .create_or_get_request(request(&managed, Some("managed-one")))
        .await
        .unwrap();
    store
        .create_or_get_request(request(&external, None))
        .await
        .unwrap();
    let application = store.clone().into_application();
    for (id, expected) in [(&managed, Some("managed-one")), (&external, None)] {
        let view = application
            .get_feedback(GetFeedbackInput {
                request_id: id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(view.managed_session_id.as_deref(), expected);
        let workspace = application
            .get_feedback_workspace(id.clone())
            .await
            .unwrap();
        assert_eq!(workspace.request.managed_session_id.as_deref(), expected);
        let summaries = application.list_open_feedback_requests().await.unwrap();
        assert_eq!(
            summaries
                .iter()
                .find(|item| item.request_id == *id)
                .unwrap()
                .managed_session_id
                .as_deref(),
            expected,
        );
        let listed = application
            .list_feedback_requests(ListFeedbackRequestsInput::default())
            .await
            .unwrap();
        assert_eq!(
            listed
                .requests
                .iter()
                .find(|item| item.request_id == *id)
                .unwrap()
                .managed_session_id
                .as_deref(),
            expected,
        );
        let json = serde_json::to_value(view).unwrap();
        assert_eq!(json.get("managed_session_id").is_some(), expected.is_some());
    }
    assert_eq!(
        application
            .managed_feedback_session(&managed)
            .await
            .unwrap()
            .as_deref(),
        Some("managed-one")
    );
    assert_eq!(
        application
            .managed_feedback_session(&external)
            .await
            .unwrap(),
        None
    );
    assert!(
        application
            .managed_feedback_session(&Uuid::now_v7().to_string())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn managed_delivery_ownership_is_persistent_and_replay_is_idempotent() {
    let (workspace, store) = setup().await;
    let original = request("request-one", Some("managed-one"));
    let created = store.create_or_get_request(original.clone()).await.unwrap();
    assert!(created.changed);
    assert_eq!(
        created.value.managed_session_id.as_deref(),
        Some("managed-one")
    );
    let replay = store.create_or_get_request(original).await.unwrap();
    assert!(!replay.changed);
    assert_eq!(replay.value, created.value);
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_request("request-one")
            .await
            .unwrap()
            .managed_session_id
            .as_deref(),
        Some("managed-one")
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM host_sessions WHERE host_session_id = 'managed-one'",
    )
    .fetch_one(&store.pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn managed_scope_requires_matching_local_id_host_family_and_correlation() {
    let (_workspace, store) = setup().await;
    let valid = request("wrong-scope", Some("managed-one"));
    let mut wrong_local = valid.clone();
    wrong_local.host_session_record_id = "managed-two".into();
    let mut wrong_host = valid.clone();
    wrong_host.host_id = "pi".into();
    let mut wrong_pair = valid;
    wrong_pair.host_session_id = "managed-two".into();
    for invalid in [
        wrong_local,
        wrong_host,
        wrong_pair,
        request("missing", Some("missing")),
    ] {
        assert_eq!(
            store.create_or_get_request(invalid).await,
            Err(RepositoryError::RequestNotFound)
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feedback_requests")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn external_caller_cannot_append_to_or_replay_a_managed_session() {
    let (_workspace, store) = setup().await;
    let original = request("managed-request", Some("managed-one"));
    store.create_or_get_request(original.clone()).await.unwrap();
    let mut external = original;
    external.managed_session_id = None;
    external.host_session_record_id = "random-external-candidate".into();
    assert_eq!(
        store.create_or_get_request(external.clone()).await,
        Err(RepositoryError::RequestNotFound)
    );
    external.request_id = "new-forged-request".into();
    assert_eq!(
        store.create_or_get_request(external).await,
        Err(RepositoryError::RequestNotFound)
    );
    assert_eq!(
        store
            .create_or_get_request(request("managed-request", Some("managed-two")))
            .await,
        Err(RepositoryError::RequestNotFound)
    );
    assert_eq!(
        store
            .create_or_get_request(request("managed-request", None))
            .await,
        Err(RepositoryError::RequestNotFound)
    );
}

#[tokio::test]
async fn legacy_requests_stay_external_and_cannot_be_claimed_by_managed_scope() {
    let (_workspace, store) = setup().await;
    let external = request("legacy", None);
    let created = store.create_or_get_request(external.clone()).await.unwrap();
    assert_eq!(created.value.managed_session_id, None);
    assert!(!store.create_or_get_request(external).await.unwrap().changed);
    assert_eq!(
        store
            .create_or_get_request(request("legacy", Some("managed-one")))
            .await,
        Err(RepositoryError::RequestNotFound)
    );
    let row = store.get_request("legacy").await.unwrap();
    assert_eq!(row.managed_session_id, None);
    assert_eq!(row.host_session_id, "external-session");
}

#[tokio::test]
async fn database_cannot_reassign_request_delivery_ownership() {
    let (_workspace, store) = setup().await;
    store
        .create_or_get_request(request("managed-request", Some("managed-one")))
        .await
        .unwrap();
    assert!(
        sqlx::query(
            "UPDATE feedback_requests SET managed_session_id = NULL WHERE id = 'managed-request'"
        )
        .execute(&store.pool)
        .await
        .is_err()
    );
    assert!(sqlx::query("UPDATE feedback_requests SET managed_session_id = 'managed-two', host_session_record_id = 'managed-two' WHERE id = 'managed-request'").execute(&store.pool).await.is_err());
    assert_eq!(
        store
            .get_request("managed-request")
            .await
            .unwrap()
            .managed_session_id
            .as_deref(),
        Some("managed-one")
    );
}

#[tokio::test]
async fn managed_session_foreign_key_cascades_only_owned_unpublished_requests() {
    let (_workspace, store) = setup().await;
    store
        .create_or_get_request(request("one", Some("managed-one")))
        .await
        .unwrap();
    store
        .create_or_get_request(request("two", Some("managed-two")))
        .await
        .unwrap();
    store
        .create_or_get_request(request("external", None))
        .await
        .unwrap();
    sqlx::query("DELETE FROM host_sessions WHERE id = 'managed-one'")
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store.get_request("one").await,
        Err(RepositoryError::RequestNotFound)
    );
    assert!(store.get_request("two").await.is_ok());
    assert!(store.get_request("external").await.is_ok());
}
