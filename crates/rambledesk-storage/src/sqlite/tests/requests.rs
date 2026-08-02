use super::*;

#[tokio::test]
async fn request_is_idempotent_conflict_safe_and_survives_restart() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let input = workspace.request(request_id.clone());

    let created = application
        .request_feedback(input.clone())
        .await
        .expect("create request");
    let retried = application
        .request_feedback(input.clone())
        .await
        .expect("retry request");
    assert_eq!(created, retried);
    assert_eq!(created.status, FeedbackStatus::Waiting);

    let mut conflicting = input;
    conflicting.context_refs[0].uri = "file:///tmp/other.diff".to_owned();
    let conflict = application
        .request_feedback(conflicting)
        .await
        .expect_err("changed immutable input must conflict");
    assert_eq!(conflict.code(), "REQUEST_CONFLICT");

    store.close().await;
    let reopened = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("reopen store");
    let recovered = reopened
        .clone()
        .into_application()
        .get_feedback(GetFeedbackInput { request_id })
        .await
        .expect("recover request");
    assert_eq!(created, recovered);
    reopened.close().await;
}

#[tokio::test]
async fn repeated_cancel_preserves_the_first_reason_and_terminal_state() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");

    let first = application
        .cancel_feedback(CancelFeedbackInput {
            request_id: request_id.clone(),
            reason: "The host no longer needs feedback.".to_owned(),
        })
        .await
        .expect("cancel request");
    let repeated = application
        .cancel_feedback(CancelFeedbackInput {
            request_id: request_id.clone(),
            reason: "This must not overwrite the original reason.".to_owned(),
        })
        .await
        .expect("repeat cancel");
    assert_eq!(first, repeated);
    assert_eq!(first.status, FeedbackStatus::Cancelled);

    let reason: String =
        sqlx::query_scalar("SELECT cancel_reason FROM feedback_requests WHERE id = ?1")
            .bind(&request_id)
            .fetch_one(&store.pool)
            .await
            .expect("stored cancel reason");
    assert_eq!(reason, "The host no longer needs feedback.");

    let terminal_update =
        sqlx::query("UPDATE feedback_requests SET status = 'waiting' WHERE id = ?1")
            .bind(&request_id)
            .execute(&store.pool)
            .await;
    assert!(terminal_update.is_err(), "cancelled state must be terminal");
    store.close().await;
}

#[tokio::test]
async fn cancellation_releases_all_feedback_waiters() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");

    let left_application = application.clone();
    let left_request_id = request_id.clone();
    let left = tokio::spawn(async move {
        left_application
            .wait_feedback(GetFeedbackInput {
                request_id: left_request_id,
            })
            .await
    });
    let right_application = application.clone();
    let right_request_id = request_id.clone();
    let right = tokio::spawn(async move {
        right_application
            .wait_feedback(GetFeedbackInput {
                request_id: right_request_id,
            })
            .await
    });
    tokio::task::yield_now().await;
    application
        .cancel_feedback(CancelFeedbackInput {
            request_id,
            reason: "The request is no longer needed.".to_owned(),
        })
        .await
        .expect("cancel request");

    for waiter in [left, right] {
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("waiter timeout")
            .expect("waiter task")
            .expect("wait result");
        assert_eq!(result.status, FeedbackStatus::Cancelled);
        assert_eq!(result.execution_mode, ExecutionMode::Wait);
    }
    store.close().await;
}

#[tokio::test]
async fn wait_returns_terminal_state_immediately_after_restart() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");
    application
        .cancel_feedback(CancelFeedbackInput {
            request_id: request_id.clone(),
            reason: "Restart recovery fixture.".to_owned(),
        })
        .await
        .expect("cancel request");
    store.close().await;

    let restarted = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("reopen store");
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        restarted
            .clone()
            .into_application()
            .wait_feedback(GetFeedbackInput { request_id }),
    )
    .await
    .expect("terminal wait must not block")
    .expect("terminal result");
    assert_eq!(result.status, FeedbackStatus::Cancelled);
    assert_eq!(result.execution_mode, ExecutionMode::Wait);
    restarted.close().await;
}

#[tokio::test]
async fn concurrent_retries_converge_on_one_request() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let left = application.request_feedback(workspace.request(request_id.clone()));
    let right = application.request_feedback(workspace.request(request_id.clone()));
    let (left, right) = tokio::join!(left, right);
    assert_eq!(left.expect("left retry"), right.expect("right retry"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feedback_requests WHERE id = ?1")
        .bind(request_id)
        .fetch_one(&store.pool)
        .await
        .expect("request count");
    assert_eq!(count, 1);
    store.close().await;
}

#[tokio::test]
async fn draft_uses_aggregate_revision_and_idempotent_replay() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");

    let first = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "The primary flow is clear.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");
    assert_eq!(first.saved_revision, 1);
    let replay = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: first.body_markdown.clone(),
            expected_revision: 0,
        })
        .await
        .expect("replay lost response");
    assert_eq!(first, replay);

    let conflict = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "A conflicting edit.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect_err("stale different body must conflict");
    assert_eq!(conflict.code(), "DRAFT_CONFLICT");

    let opened = application
        .get_feedback_workspace(request_id.clone())
        .await
        .expect("open workspace");
    assert_eq!(opened.request.revision, 1);
    assert_eq!(opened.request.status, FeedbackStatus::InProgress);
    assert_eq!(opened.draft, first);

    store.close().await;
    let reopened = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("reopen store");
    let recovered = reopened
        .clone()
        .into_application()
        .get_feedback_workspace(request_id)
        .await
        .expect("recover draft");
    assert_eq!(recovered.draft.saved_revision, 1);
    assert_eq!(recovered.draft.body_markdown, "The primary flow is clear.");
    reopened.close().await;
}

#[tokio::test]
async fn concurrent_different_drafts_have_one_cas_winner() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");

    let left = application.save_feedback_draft(SaveDraftInput {
        request_id: request_id.clone(),
        body_markdown: "left".to_owned(),
        expected_revision: 0,
    });
    let right = application.save_feedback_draft(SaveDraftInput {
        request_id: request_id.clone(),
        body_markdown: "right".to_owned(),
        expected_revision: 0,
    });
    let (left, right) = tokio::join!(left, right);
    assert_ne!(left.is_ok(), right.is_ok());
    let loser = left.err().or_else(|| right.err()).expect("one loser");
    assert_eq!(loser.code(), "DRAFT_CONFLICT");
    let saved = application
        .get_feedback_workspace(request_id)
        .await
        .expect("winner persisted");
    assert_eq!(saved.request.revision, 1);
    assert!(matches!(
        saved.draft.body_markdown.as_str(),
        "left" | "right"
    ));
    store.close().await;
}
