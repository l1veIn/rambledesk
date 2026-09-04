use std::collections::BTreeMap;

use rambledesk_core::{
    AgentConfig, FeedbackDeliveryRepository, FeedbackDeliveryState as DeliveryState,
    NewManagedSession, ResolveDeliveryAction, SessionProtocol, SessionRepository,
    SessionRepositoryError,
};
use sqlx::migrate::Migrate;

use super::*;

const NOW: &str = "2026-09-04T02:00:00Z";
const LATER: &str = "2026-09-04T03:00:00Z";

async fn setup() -> (TestWorkspace, SqliteFeedbackStore) {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    seed_sessions(&workspace, &store).await;
    (workspace, store)
}

async fn seed_sessions(workspace: &TestWorkspace, store: &SqliteFeedbackStore) {
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
            created_at: NOW.into(),
            updated_at: NOW.into(),
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
                created_at: NOW.into(),
            })
            .await
            .unwrap();
    }
}

async fn new_request(store: &SqliteFeedbackStore, managed: Option<&str>) -> String {
    let request_id = Uuid::now_v7().to_string();
    let correlation = managed.unwrap_or("external");
    store
        .create_or_get_request(NewFeedbackRequest {
            request_id: request_id.clone(),
            host_session_record_id: correlation.into(),
            managed_session_id: managed.map(str::to_owned),
            host_id: "dsh".into(),
            host_session_id: correlation.into(),
            title: "Review".into(),
            what_happened: "Scoped feedback".into(),
            actions: vec![],
            context_refs: vec![],
            attachments: vec![],
            source_hint: None,
            allow_finish: true,
            final_summary: Some("Done".into()),
            created_at: NOW.into(),
        })
        .await
        .unwrap();
    request_id
}

async fn approved(store: &SqliteFeedbackStore, session: &str) -> String {
    let id = new_request(store, Some(session)).await;
    store.approve_request(&id, NOW).await.unwrap();
    id
}

fn submission(request_id: &str, revision: u64) -> SubmitFeedbackInput {
    SubmitFeedbackInput {
        request_id: request_id.into(),
        expected_revision: revision,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    }
}

async fn prepare_draft(store: &SqliteFeedbackStore, request_id: &str) -> u64 {
    store
        .clone()
        .into_application()
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.into(),
            expected_revision: 0,
            document_json: r#"{"schemaVersion":2,"doc":{"type":"doc"}}"#.into(),
            body_markdown: "Review completed. Continue with the requested changes.".into(),
        })
        .await
        .unwrap()
        .saved_revision
}

#[tokio::test]
async fn all_managed_terminal_resolutions_enqueue_once_and_external_requests_do_not() {
    let (_workspace, store) = setup().await;
    let approve_id = approved(&store, "one").await;
    let cancel_id = new_request(&store, Some("one")).await;
    let submit_id = new_request(&store, Some("one")).await;
    let external = new_request(&store, None).await;
    store.approve_request(&external, NOW).await.unwrap();
    let app = store.clone().into_application();
    let cancel = CancelFeedbackInput {
        request_id: cancel_id.clone(),
        reason: "Cancel this review".into(),
    };
    app.cancel_feedback(cancel.clone()).await.unwrap();
    let revision = prepare_draft(&store, &submit_id).await;
    app.submit_feedback(submission(&submit_id, revision))
        .await
        .unwrap();
    store.approve_request(&approve_id, LATER).await.unwrap();
    app.cancel_feedback(cancel).await.unwrap();
    app.submit_feedback(submission(&submit_id, 0))
        .await
        .unwrap();
    let pending = store.list_pending_deliveries().await.unwrap();
    assert_eq!(pending.len(), 3);
    assert!(
        pending
            .iter()
            .all(|item| item.session_id == "one" && item.state == DeliveryState::Pending)
    );
    assert_eq!(
        pending
            .iter()
            .find(|item| item.request_id == approve_id)
            .unwrap()
            .resolution,
        FeedbackResolution::Approved
    );
    assert_eq!(
        pending
            .iter()
            .find(|item| item.request_id == cancel_id)
            .unwrap()
            .resolution,
        FeedbackResolution::Cancelled
    );
    assert_eq!(
        pending
            .iter()
            .find(|item| item.request_id == submit_id)
            .unwrap()
            .resolution,
        FeedbackResolution::FeedbackSubmitted
    );
}

#[tokio::test]
async fn approval_and_delivery_commit_atomically() {
    let (_workspace, store) = setup().await;
    let id = new_request(&store, Some("one")).await;
    sqlx::query("CREATE TRIGGER reject_delivery BEFORE INSERT ON feedback_deliveries BEGIN SELECT RAISE(ABORT, 'test failure'); END").execute(&store.pool).await.unwrap();
    assert_eq!(
        store.approve_request(&id, NOW).await,
        Err(RepositoryError::Storage)
    );
    assert_eq!(
        store.get_request(&id).await.unwrap().status,
        FeedbackStatus::Waiting
    );
    assert!(store.list_pending_deliveries().await.unwrap().is_empty());
    sqlx::query("DROP TRIGGER reject_delivery")
        .execute(&store.pool)
        .await
        .unwrap();
    store.approve_request(&id, NOW).await.unwrap();
    assert_eq!(store.list_pending_deliveries().await.unwrap().len(), 1);
}

#[tokio::test]
async fn failed_delivery_insert_recovers_published_feedback_and_outbox_together() {
    let (workspace, store) = setup().await;
    let id = new_request(&store, Some("one")).await;
    let revision = prepare_draft(&store, &id).await;
    sqlx::query("CREATE TRIGGER reject_delivery BEFORE INSERT ON feedback_deliveries BEGIN SELECT RAISE(ABORT, 'test failure'); END").execute(&store.pool).await.unwrap();
    assert!(
        store
            .clone()
            .into_application()
            .submit_feedback(submission(&id, revision))
            .await
            .is_err()
    );
    assert_eq!(
        store.get_request(&id).await.unwrap().status,
        FeedbackStatus::InProgress
    );
    assert!(store.list_pending_deliveries().await.unwrap().is_empty());
    sqlx::query("DROP TRIGGER reject_delivery")
        .execute(&store.pool)
        .await
        .unwrap();
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let request = store.get_request(&id).await.unwrap();
    assert_eq!(request.status, FeedbackStatus::Completed);
    assert!(request.feedback.is_some());
    assert_eq!(
        store.list_pending_deliveries().await.unwrap()[0].request_id,
        id
    );
}

#[tokio::test]
async fn competing_claims_and_old_attempts_cannot_duplicate_or_overwrite_delivery() {
    let (_workspace, store) = setup().await;
    let id = approved(&store, "one").await;
    let (left, right) = tokio::join!(
        store.claim_delivery(&id, "attempt-a", NOW),
        store.claim_delivery(&id, "attempt-b", NOW)
    );
    let claimed = [left.unwrap(), right.unwrap()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(claimed.len(), 1);
    let first_attempt = claimed[0].attempt_id.as_deref().unwrap();
    store
        .finish_delivery(
            &id,
            first_attempt,
            DeliveryState::Pending,
            Some("No bytes sent"),
            NOW,
        )
        .await
        .unwrap();
    store
        .claim_delivery(&id, "new-attempt", LATER)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .finish_delivery(&id, first_attempt, DeliveryState::Delivered, None, LATER)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    let delivered = store
        .finish_delivery(&id, "new-attempt", DeliveryState::Delivered, None, LATER)
        .await
        .unwrap();
    assert_eq!(
        store
            .finish_delivery(&id, "new-attempt", DeliveryState::Delivered, None, LATER)
            .await
            .unwrap(),
        delivered
    );
    assert!(
        store
            .claim_delivery(&id, "duplicate", LATER)
            .await
            .unwrap()
            .is_none()
    );
    store.approve_request(&id, LATER).await.unwrap();
    assert_eq!(
        store.list_session_deliveries("one").await.unwrap()[0].state,
        DeliveryState::Delivered
    );
}

#[tokio::test]
async fn interrupted_send_requires_explicit_resolution_and_checks_session_ownership() {
    let (workspace, store) = setup().await;
    let id = approved(&store, "one").await;
    store
        .claim_delivery(&id, "attempt-before-restart", NOW)
        .await
        .unwrap()
        .unwrap();
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        store.recover_interrupted_deliveries(LATER).await.unwrap(),
        1
    );
    assert_eq!(
        store.recover_interrupted_deliveries(LATER).await.unwrap(),
        0
    );
    assert!(store.list_pending_deliveries().await.unwrap().is_empty());
    assert!(
        store
            .claim_delivery(&id, "automatic-retry", LATER)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .resolve_delivery(&id, "two", ResolveDeliveryAction::Retry, LATER)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store
            .resolve_delivery(&id, "one", ResolveDeliveryAction::Retry, LATER)
            .await
            .unwrap()
            .state,
        DeliveryState::Pending
    );
    store
        .claim_delivery(&id, "retry-attempt", LATER)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .finish_delivery(
                &id,
                "attempt-before-restart",
                DeliveryState::Delivered,
                None,
                LATER
            )
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    store
        .finish_delivery(
            &id,
            "retry-attempt",
            DeliveryState::Uncertain,
            Some("Connection lost"),
            LATER,
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .resolve_delivery(&id, "one", ResolveDeliveryAction::Acknowledge, LATER)
            .await
            .unwrap()
            .state,
        DeliveryState::Delivered
    );
    assert_eq!(
        store
            .resolve_delivery(&id, "one", ResolveDeliveryAction::Retry, LATER)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
}

#[tokio::test]
async fn discard_prevents_late_completion_and_does_not_touch_other_sessions() {
    let (_workspace, store) = setup().await;
    let pending = approved(&store, "one").await;
    let sending = approved(&store, "one").await;
    let other = approved(&store, "two").await;
    store
        .claim_delivery(&sending, "in-flight", NOW)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .discard_session_deliveries("one", LATER)
            .await
            .unwrap(),
        2
    );
    assert!(
        store
            .claim_delivery(&pending, "late", LATER)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .finish_delivery(&sending, "in-flight", DeliveryState::Delivered, None, LATER)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store.list_pending_deliveries().await.unwrap()[0].request_id,
        other
    );
    store.approve_request(&pending, LATER).await.unwrap();
    assert!(
        store
            .list_session_deliveries("one")
            .await
            .unwrap()
            .iter()
            .all(|item| item.state == DeliveryState::Discarded)
    );
}

#[tokio::test]
async fn database_rejects_delivery_for_wrong_owner_or_nonterminal_request() {
    let (_workspace, store) = setup().await;
    let waiting = new_request(&store, Some("one")).await;
    assert!(sqlx::query("INSERT INTO feedback_deliveries(request_id,session_id,resolution,created_at,updated_at) VALUES (?1,'one','approved',?2,?2)").bind(&waiting).bind(NOW).execute(&store.pool).await.is_err());
    let approved = approved(&store, "one").await;
    assert!(
        sqlx::query("UPDATE feedback_deliveries SET session_id = 'two' WHERE request_id = ?1")
            .bind(&approved)
            .execute(&store.pool)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .finish_delivery(&approved, "attempt", DeliveryState::Sending, None, NOW)
            .await,
        Err(SessionRepositoryError::InvalidInput)
    );
    assert_eq!(
        store.claim_delivery(&approved, "", NOW).await,
        Err(SessionRepositoryError::InvalidInput)
    );
}

#[tokio::test]
async fn migration_reconciles_preexisting_terminal_managed_requests_only() {
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
    for migration in MIGRATOR.iter().filter(|migration| migration.version <= 13) {
        connection.apply(migration).await.unwrap();
    }
    drop(connection);
    let store = SqliteFeedbackStore {
        pool,
        library_root: Arc::new(RwLock::new(workspace._temp.path().into())),
        publish_lock: Arc::new(tokio::sync::Mutex::new(())),
    };
    seed_sessions(&workspace, &store).await;
    // Build the old-schema fixture directly: current repository guards can
    // legitimately require tables introduced after migration 13.
    sqlx::query("INSERT INTO host_sessions(id,host_id,host_session_id,created_at,updated_at) VALUES ('external','dsh','external',?1,?1)")
        .bind(NOW).execute(&store.pool).await.unwrap();
    let managed = "managed-before-migration";
    let external = "external-before-migration";
    for (id, session_id, marker) in [
        (managed, "one", Some("one")),
        (external, "external", None),
        ("waiting-before-migration", "one", Some("one")),
    ] {
        sqlx::query("INSERT INTO feedback_requests(id,host_session_record_id,managed_session_id,title,what_happened,status,input_hash,created_at,updated_at) VALUES (?1,?2,?3,'Review','Before migration','waiting','fixture',?4,?4)")
            .bind(id).bind(session_id).bind(marker).bind(NOW).execute(&store.pool).await.unwrap();
    }
    for id in [&managed, &external] {
        sqlx::query("UPDATE feedback_requests SET status='completed', resolution='approved', completed_at=?2 WHERE id=?1").bind(id).bind(NOW).execute(&store.pool).await.unwrap();
    }
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let pending = store.list_pending_deliveries().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request_id, managed);
}
