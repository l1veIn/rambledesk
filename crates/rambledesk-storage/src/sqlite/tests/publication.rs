use super::*;

#[tokio::test]
async fn submit_is_idempotent_and_publishes_one_immutable_package() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let mut request = workspace.request(request_id.clone());
    request.attachments = vec![RequestAttachmentInput {
        file_name: "agent-review.md".to_owned(),
        markdown: Some("# Agent review\n\nKeep this with the package.".to_owned()),
        contents_base64: None,
        path: None,
    }];
    application
        .request_feedback(request)
        .await
        .expect("create request");
    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "Ship it after tightening the empty state.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");

    let submitted = application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: draft.saved_revision,
            cooked_markdown: Some(
                "The empty state should be tightened before shipping.".to_owned(),
            ),
            cooking_model: Some("deepseek/deepseek-chat".to_owned()),
        })
        .await
        .expect("submit");
    let replay = application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: 0,
            cooked_markdown: None,
            cooking_model: None,
        })
        .await
        .expect("completed submit replay");
    assert_eq!(submitted, replay);
    assert_eq!(submitted.status, FeedbackStatus::Completed);
    let result = submitted.feedback.clone().expect("published feedback");
    assert!(Path::new(&result.markdown_path).is_file());
    assert!(
        tokio::fs::read_to_string(&result.markdown_path)
            .await
            .expect("cooked feedback")
            .trim()
            == "The empty state should be tightened before shipping."
    );
    assert!(
        tokio::fs::read_to_string(Path::new(&result.directory_path).join("uncooked.md"))
            .await
            .expect("uncooked feedback")
            .trim()
            == "Ship it after tightening the empty state."
    );
    let manifest: serde_json::Value = serde_json::from_str(
        &tokio::fs::read_to_string(&result.manifest_path)
            .await
            .expect("manifest"),
    )
    .expect("valid manifest");
    assert_eq!(manifest["request_id"], request_id);
    assert_eq!(manifest["source_revision"], 1);
    assert_eq!(manifest["draft_revision"], 1);
    assert_eq!(manifest["feedback_markdown"], "feedback.md");
    assert_eq!(manifest["uncooked_markdown"], "uncooked.md");
    assert_eq!(manifest["cooking_model"], "deepseek/deepseek-chat");
    assert!(manifest["feedback_sha256"].as_str().is_some());
    assert!(manifest["uncooked_sha256"].as_str().is_some());
    assert_eq!(
        manifest["request_attachments"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        manifest["request_attachments"][0]["path"],
        "request-attachments/001-agent-review.md"
    );
    let package = application
        .read_feedback_package(&submitted)
        .await
        .expect("read package")
        .expect("published package");
    assert_eq!(package.request_attachment_paths.len(), 1);
    assert_eq!(
        tokio::fs::read_to_string(&package.request_attachment_paths[0])
            .await
            .expect("published request attachment"),
        "# Agent review\n\nKeep this with the package."
    );
    let (blob_bytes, draft_path, published_path): (i64, String, String) = sqlx::query_as(
        "SELECT length(contents), draft_path, published_path \
         FROM request_attachments WHERE request_id = ?1",
    )
    .bind(&request_id)
    .fetch_one(&store.pool)
    .await
    .expect("published request attachment paths");
    assert_eq!(blob_bytes, 0);
    assert!(!Path::new(&draft_path).exists());
    assert!(Path::new(&published_path).is_file());

    let directory_count = std::fs::read_dir(workspace.database.parent().unwrap().join("feedback"))
        .expect("feedback root")
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .count();
    assert_eq!(directory_count, 1);
    store.close().await;
}

#[tokio::test]
async fn restart_reconciles_package_published_before_database_completion() {
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
    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "Recovery must converge on this package.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");
    let plan = store
        .plan_submission(
            &request_id,
            draft.saved_revision,
            None,
            None,
            &Uuid::now_v7().to_string(),
            "2026-07-29T14:00:00Z",
        )
        .await
        .expect("persist intent");
    rambledesk_core::FeedbackPackagePublisher::publish(&store, &plan)
        .await
        .expect("publish before simulated crash");
    store.close().await;

    let reopened = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("startup reconciliation");
    let completed = reopened
        .clone()
        .into_application()
        .get_feedback(GetFeedbackInput { request_id })
        .await
        .expect("completed after recovery");
    assert_eq!(completed.status, FeedbackStatus::Completed);
    assert_eq!(
        completed.feedback.expect("feedback result").directory_path,
        plan.directory_path
    );
    reopened.close().await;
}

#[tokio::test]
async fn publishes_feedback_package_under_explicit_library_root() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let library_root = workspace.database.with_file_name("library");
    let store = SqliteFeedbackStore::connect_with_library(&workspace.database, &library_root)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");
    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "Feedback packages are stored under app data.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");
    let completed = application
        .submit_feedback(SubmitFeedbackInput {
            request_id,
            expected_revision: draft.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
        })
        .await
        .expect("submit");
    let directory = completed.feedback.expect("feedback result").directory_path;
    let feedback_root = tokio::fs::canonicalize(library_root.join("feedback"))
        .await
        .expect("canonical library feedback root");
    assert!(Path::new(&directory).starts_with(feedback_root));
    store.close().await;
}

#[tokio::test]
async fn mismatched_existing_final_package_is_never_overwritten() {
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
    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "Do not overwrite an unexpected package.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");
    let plan = store
        .plan_submission(
            &request_id,
            draft.saved_revision,
            None,
            None,
            &Uuid::now_v7().to_string(),
            "2026-07-29T15:00:00Z",
        )
        .await
        .expect("plan");
    tokio::fs::create_dir_all(&plan.directory_path)
        .await
        .expect("unexpected final directory");
    tokio::fs::write(&plan.manifest_path, "owned by someone else\n")
        .await
        .expect("unexpected manifest");
    tokio::fs::write(&plan.markdown_path, "do not replace\n")
        .await
        .expect("unexpected markdown");

    let error = rambledesk_core::FeedbackPackagePublisher::publish(&store, &plan)
        .await
        .expect_err("mismatch must fail");
    assert_eq!(error, RepositoryError::PackagePublish);
    assert_eq!(
        tokio::fs::read_to_string(&plan.manifest_path)
            .await
            .expect("manifest preserved"),
        "owned by someone else\n"
    );
    assert_eq!(
        tokio::fs::read_to_string(&plan.markdown_path)
            .await
            .expect("markdown preserved"),
        "do not replace\n"
    );
    store.close().await;
}

#[tokio::test]
async fn mismatched_pending_package_does_not_block_startup() {
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
    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "Keep the workbench available for repair.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");
    let plan = store
        .plan_submission(
            &request_id,
            draft.saved_revision,
            None,
            None,
            &Uuid::now_v7().to_string(),
            "2026-07-29T15:30:00Z",
        )
        .await
        .expect("plan");
    tokio::fs::create_dir_all(&plan.directory_path)
        .await
        .expect("unexpected final directory");
    tokio::fs::write(&plan.manifest_path, "mismatch\n")
        .await
        .expect("unexpected manifest");
    tokio::fs::write(&plan.markdown_path, "preserve\n")
        .await
        .expect("unexpected markdown");
    store.close().await;

    let reopened = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("one failed recovery must not block startup");
    let error_code: String =
        sqlx::query_scalar("SELECT last_error_code FROM submission_plans WHERE request_id = ?1")
            .bind(&request_id)
            .fetch_one(&reopened.pool)
            .await
            .expect("diagnostic recovery error");
    assert_eq!(error_code, "PACKAGE_PUBLISH_FAILURE");
    let request = reopened
        .clone()
        .into_application()
        .get_feedback(GetFeedbackInput { request_id })
        .await
        .expect("request remains visible");
    assert_eq!(request.status, FeedbackStatus::InProgress);
    reopened.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn publisher_rejects_feedback_parent_replaced_by_symlink_after_plan() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new().await;
    let outside = workspace._temp.path().join("outside-after-plan");
    tokio::fs::create_dir(&outside)
        .await
        .expect("outside directory");
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");
    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "Revalidate the frozen target before writing.".to_owned(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");
    let plan = store
        .plan_submission(
            &request_id,
            draft.saved_revision,
            None,
            None,
            &Uuid::now_v7().to_string(),
            "2026-07-29T16:00:00Z",
        )
        .await
        .expect("plan");
    let feedback_root = Path::new(&plan.directory_path)
        .parent()
        .expect("feedback root");
    tokio::fs::remove_dir(feedback_root)
        .await
        .expect("replace empty feedback root");
    symlink(&outside, feedback_root).expect("replacement symlink");

    let error = rambledesk_core::FeedbackPackagePublisher::publish(&store, &plan)
        .await
        .expect_err("publisher must reject swapped parent");
    assert_eq!(error, RepositoryError::PackagePublish);
    assert_eq!(
        std::fs::read_dir(&outside)
            .expect("outside remains readable")
            .count(),
        0
    );
    store.close().await;
}
