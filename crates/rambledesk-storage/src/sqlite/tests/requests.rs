use super::*;

#[tokio::test]
async fn request_attachments_are_immutable_readable_and_idempotent() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let mut input = workspace.request(request_id.clone());
    input.attachments = vec![
        RequestAttachmentInput {
            file_name: "review.md".to_owned(),
            markdown: Some("# Review\n\nPlease inspect this proposal.".to_owned()),
            contents_base64: None,
        },
        RequestAttachmentInput {
            file_name: "mockup".to_owned(),
            markdown: None,
            contents_base64: Some(
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Zl1sAAAAASUVORK5CYII="
                    .to_owned(),
            ),
        },
    ];

    application
        .request_feedback(input.clone())
        .await
        .expect("create request with attachments");
    application
        .request_feedback(input.clone())
        .await
        .expect("retry identical request");

    let opened = application
        .get_feedback_workspace(request_id.clone())
        .await
        .expect("open workspace");
    assert_eq!(opened.request_attachments.len(), 2);
    assert_eq!(opened.request_attachments[0].file_name, "review.md");
    assert_eq!(opened.request_attachments[0].media_type, "text/markdown");
    assert_eq!(opened.request_attachments[1].file_name, "mockup.png");
    assert_eq!(opened.request_attachments[1].media_type, "image/png");

    let markdown = application
        .read_request_attachment(
            request_id.clone(),
            opened.request_attachments[0].attachment_id.clone(),
        )
        .await
        .expect("read markdown attachment");
    assert_eq!(
        String::from_utf8(markdown).expect("markdown UTF-8"),
        "# Review\n\nPlease inspect this proposal."
    );
    let image = application
        .read_request_attachment(
            request_id.clone(),
            opened.request_attachments[1].attachment_id.clone(),
        )
        .await
        .expect("read image attachment");
    assert!(image.starts_with(b"\x89PNG\r\n\x1a\n"));

    let stored_paths: Vec<(i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT length(contents), draft_path, published_path \
         FROM request_attachments WHERE request_id = ?1 ORDER BY position",
    )
    .bind(&request_id)
    .fetch_all(&store.pool)
    .await
    .expect("request attachment storage paths");
    assert_eq!(stored_paths.len(), 2);
    for (blob_bytes, draft_path, published_path) in stored_paths {
        assert_eq!(blob_bytes, 0, "SQLite must not retain attachment bytes");
        assert!(Path::new(&draft_path.expect("draft path")).is_file());
        assert!(published_path.is_none());
    }

    input.attachments[0].markdown = Some("changed".to_owned());
    let conflict = application
        .request_feedback(input)
        .await
        .expect_err("changed request attachment must conflict");
    assert_eq!(conflict.code(), "REQUEST_CONFLICT");
    store.close().await;
}

#[tokio::test]
async fn startup_externalizes_legacy_request_attachment_blobs() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let mut request = workspace.request(request_id.clone());
    request.attachments = vec![RequestAttachmentInput {
        file_name: "legacy.md".to_owned(),
        markdown: Some("# Legacy attachment".to_owned()),
        contents_base64: None,
    }];
    application
        .request_feedback(request)
        .await
        .expect("create request");
    let (attachment_id, draft_path): (String, String) =
        sqlx::query_as("SELECT id, draft_path FROM request_attachments WHERE request_id = ?1")
            .bind(&request_id)
            .fetch_one(&store.pool)
            .await
            .expect("stored attachment");
    let bytes = tokio::fs::read(&draft_path).await.expect("draft bytes");
    tokio::fs::remove_file(&draft_path)
        .await
        .expect("remove external file");
    sqlx::query("UPDATE request_attachments SET contents = ?2, draft_path = NULL WHERE id = ?1")
        .bind(&attachment_id)
        .bind(&bytes)
        .execute(&store.pool)
        .await
        .expect("simulate legacy blob");
    store.close().await;

    let reopened = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("reopen and externalize");
    let (blob_bytes, external_path): (i64, String) = sqlx::query_as(
        "SELECT length(contents), draft_path FROM request_attachments WHERE id = ?1",
    )
    .bind(&attachment_id)
    .fetch_one(&reopened.pool)
    .await
    .expect("externalized attachment");
    assert_eq!(blob_bytes, 0);
    assert_eq!(
        tokio::fs::read(external_path)
            .await
            .expect("external bytes"),
        bytes
    );
    reopened.close().await;
}

#[tokio::test]
async fn startup_archives_legacy_cancelled_requests() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let mut request = workspace.request(request_id.clone());
    request.attachments = vec![RequestAttachmentInput {
        file_name: "cancelled.md".to_owned(),
        markdown: Some("# Preserved request context".to_owned()),
        contents_base64: None,
    }];
    application
        .request_feedback(request)
        .await
        .expect("create request");
    sqlx::query(
        "UPDATE feedback_requests SET status = 'cancelled', resolution = 'cancelled', \
             cancelled_at = ?2, cancel_reason = 'Legacy cancellation', updated_at = ?2, revision = 1 \
         WHERE id = ?1",
    )
    .bind(&request_id)
    .bind("2026-08-03T08:00:00Z")
    .execute(&store.pool)
    .await
    .expect("simulate legacy cancellation");
    store.close().await;

    let reopened = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("reopen and archive cancellation");
    let recovered = reopened
        .clone()
        .into_application()
        .get_feedback(GetFeedbackInput {
            request_id: request_id.clone(),
        })
        .await
        .expect("read archived cancellation");
    assert_eq!(recovered.status, FeedbackStatus::Cancelled);
    assert!(recovered.feedback.is_some());
    let published_path: String =
        sqlx::query_scalar("SELECT published_path FROM request_attachments WHERE request_id = ?1")
            .bind(&request_id)
            .fetch_one(&reopened.pool)
            .await
            .expect("published legacy request attachment");
    assert!(Path::new(&published_path).is_file());
    reopened.close().await;
}

#[tokio::test]
async fn recovery_is_host_scoped_and_rejects_ambiguous_session_matches() {
    let workspace = TestWorkspace::new().await;
    let first_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let first = workspace.request(first_id.clone());
    let host_id = first.host_id.clone();
    let host_session_id = first.host_session_id.clone();
    application
        .request_feedback(first)
        .await
        .expect("create first request");

    let recovered = application
        .recover_feedback(RecoverFeedbackInput {
            request_id: Some(first_id.clone()),
            host_id: Some(host_id.clone()),
            host_session_id: host_session_id.clone(),
        })
        .await
        .expect("recover exact request");
    assert_eq!(recovered.request_id, first_id);

    let wrong_session = application
        .recover_feedback(RecoverFeedbackInput {
            request_id: Some(first_id),
            host_id: Some(host_id.clone()),
            host_session_id: "another-session".to_owned(),
        })
        .await
        .expect_err("cross-session recovery must be hidden");
    assert_eq!(wrong_session.code(), "REQUEST_NOT_FOUND");

    application
        .request_feedback(workspace.request(Uuid::now_v7().to_string()))
        .await
        .expect("create second request in session");
    let ambiguous = application
        .recover_feedback(RecoverFeedbackInput {
            request_id: None,
            host_id: Some(host_id),
            host_session_id,
        })
        .await
        .expect_err("session-only recovery must not guess between requests");
    assert_eq!(ambiguous.code(), "RECOVERY_AMBIGUOUS");
    store.close().await;
}

#[tokio::test]
async fn final_summary_can_be_approved_without_publishing_feedback() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let mut input = workspace.request(request_id.clone());
    input.allow_finish = true;
    input.final_summary = Some("Implemented the feature and all tests pass.".to_owned());

    let created = application
        .request_feedback(input)
        .await
        .expect("create final proposal");
    assert!(created.allow_finish);
    assert_eq!(
        created.final_summary.as_deref(),
        Some("Implemented the feature and all tests pass.")
    );

    let approved = application
        .approve_feedback(ApproveFeedbackInput {
            request_id: request_id.clone(),
        })
        .await
        .expect("approve final summary");
    assert_eq!(approved.status, FeedbackStatus::Completed);
    assert_eq!(approved.resolution, Some(FeedbackResolution::Approved));
    assert!(approved.feedback.is_none());

    let replay = application
        .approve_feedback(ApproveFeedbackInput { request_id })
        .await
        .expect("approval is idempotent");
    assert_eq!(replay.resolution, Some(FeedbackResolution::Approved));
    store.close().await;
}

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
    let mut request = workspace.request(request_id.clone());
    request.attachments = vec![RequestAttachmentInput {
        file_name: "agent-note.md".to_owned(),
        markdown: Some("# Agent note\n\nReview before cancelling.".to_owned()),
        contents_base64: None,
    }];
    application
        .request_feedback(request)
        .await
        .expect("create request");
    application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "Partial human feedback.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save partial draft");

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
    let result = first.feedback.as_ref().expect("cancel feedback package");
    let package = application
        .read_feedback_package(&first)
        .await
        .expect("read cancellation package")
        .expect("published cancellation package");
    assert_eq!(package.manifest.resolution, FeedbackResolution::Cancelled);
    assert_eq!(
        package.manifest.cancel_reason.as_deref(),
        Some("The host no longer needs feedback.")
    );
    assert_eq!(
        package.uncooked_markdown.as_deref(),
        Some("Partial human feedback.\n")
    );
    assert_eq!(package.request_attachment_paths.len(), 1);
    assert!(Path::new(&result.directory_path).is_dir());
    assert!(
        !workspace
            .database
            .parent()
            .expect("database parent")
            .join("drafts")
            .join(&request_id)
            .exists()
    );

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
