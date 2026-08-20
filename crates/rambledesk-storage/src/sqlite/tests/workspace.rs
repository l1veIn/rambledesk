use super::*;

#[tokio::test]
async fn attachments_share_revision_publish_in_order_and_survive_restart() {
    let workspace = TestWorkspace::new().await;
    let request_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    let mut request = workspace.request(request_id.clone());
    request.what_happened = "中文反馈请求：检查图片和正文是否完整".to_owned();
    request.actions[0].instruction = "边截图边记录中文说明".to_owned();
    application
        .request_feedback(request)
        .await
        .expect("create request");

    let first_bytes = b"\x89PNG\r\n\x1a\nfirst-image".to_vec();
    let first = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id: request_id.clone(),
            file_name: "first.png".to_owned(),
            contents: first_bytes.clone(),
            expected_revision: 0,
        })
        .await
        .expect("add first attachment");
    assert_eq!(first.request.revision, 1);
    assert_eq!(first.draft.saved_revision, 1);
    assert_eq!(first.attachments.len(), 1);
    let first_id = first.attachments[0].attachment_id.clone();
    assert_eq!(
        application
            .read_feedback_attachment(request_id.clone(), first_id.clone())
            .await
            .expect("read attachment"),
        first_bytes
    );

    let stale = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id: request_id.clone(),
            file_name: "stale.gif".to_owned(),
            contents: b"GIF89astale".to_vec(),
            expected_revision: 0,
        })
        .await
        .expect_err("stale aggregate revision must conflict");
    assert_eq!(stale.code(), "DRAFT_CONFLICT");

    let second_bytes = b"\xff\xd8\xffsecond-image".to_vec();
    let second = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id: request_id.clone(),
            file_name: "second.jpg".to_owned(),
            contents: second_bytes.clone(),
            expected_revision: 1,
        })
        .await
        .expect("add second attachment");
    let second_id = second.attachments[1].attachment_id.clone();
    let reordered = application
        .reorder_feedback_attachments(ReorderAttachmentsInput {
            request_id: request_id.clone(),
            attachment_ids: vec![second_id.clone(), first_id.clone()],
            expected_revision: 2,
        })
        .await
        .expect("reorder attachments");
    assert_eq!(reordered.request.revision, 3);
    assert_eq!(reordered.attachments[0].attachment_id, second_id);

    let removed = application
        .remove_feedback_attachment(RemoveAttachmentInput {
            request_id: request_id.clone(),
            attachment_id: first_id,
            expected_revision: 3,
        })
        .await
        .expect("remove attachment");
    assert_eq!(removed.request.revision, 4);
    assert_eq!(removed.attachments.len(), 1);
    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: format!(
                "图片前的中文说明。\n\n![中文截图](attachment://{second_id})\n\n图片后的中文结论。"
            ),
            expected_revision: 4,
        })
        .await
        .expect("save feedback");
    let submitted = application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: draft.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
        })
        .await
        .expect("publish feedback");
    let result = submitted.feedback.expect("feedback package");
    let published_markdown = tokio::fs::read_to_string(&result.markdown_path)
        .await
        .expect("published Markdown");
    assert!(!published_markdown.contains("## What Happened"));
    assert!(!published_markdown.contains("## What to Try"));
    assert!(published_markdown.contains("图片前的中文说明。"));
    assert!(published_markdown.contains("![中文截图](attachments/001-second.jpg)"));
    assert!(published_markdown.contains("图片后的中文结论。"));
    assert!(!published_markdown.contains("attachment://"));
    assert!(!published_markdown.contains("## Attachments"));
    let uncooked_markdown =
        tokio::fs::read_to_string(Path::new(&result.directory_path).join("uncooked.md"))
            .await
            .expect("uncooked Markdown");
    assert!(uncooked_markdown.contains(&format!("attachment://{second_id}")));
    assert!(!uncooked_markdown.contains("## Operator Feedback"));
    let manifest: serde_json::Value = serde_json::from_str(
        &tokio::fs::read_to_string(&result.manifest_path)
            .await
            .expect("manifest"),
    )
    .expect("valid manifest");
    assert_eq!(manifest["attachments"][0]["file_name"], "second.jpg");
    assert_eq!(
        manifest["attachments"][0]["path"],
        "attachments/001-second.jpg"
    );
    let published_attachment = Path::new(&result.directory_path).join("attachments/001-second.jpg");
    assert_eq!(
        tokio::fs::read(published_attachment)
            .await
            .expect("published attachment"),
        second_bytes
    );
    store.close().await;

    let reopened = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("reopen store");
    let recovered = reopened
        .clone()
        .into_application()
        .get_feedback_workspace(request_id)
        .await
        .expect("recover workspace");
    assert_eq!(recovered.attachments.len(), 1);
    assert_eq!(recovered.attachments[0].file_name, "second.jpg");
    assert_eq!(recovered.feedback.as_ref(), Some(&result));
    reopened.close().await;
}

#[tokio::test]
async fn non_image_attachments_detect_publish_and_round_trip() {
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

    let pdf_bytes = b"%PDF-1.4\n1 0 obj\n%%EOF".to_vec();
    let pdf = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id: request_id.clone(),
            file_name: "plan.pdf".to_owned(),
            contents: pdf_bytes.clone(),
            expected_revision: 0,
        })
        .await
        .expect("add pdf attachment");
    assert_eq!(pdf.attachments[0].media_type, "application/pdf");
    assert_eq!(pdf.attachments[0].file_name, "plan.pdf");
    let pdf_id = pdf.attachments[0].attachment_id.clone();
    assert_eq!(
        application
            .read_feedback_attachment(request_id.clone(), pdf_id.clone())
            .await
            .expect("read pdf"),
        pdf_bytes
    );

    let md = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id: request_id.clone(),
            file_name: "notes.md".to_owned(),
            contents: b"# Notes".to_vec(),
            expected_revision: 1,
        })
        .await
        .expect("add markdown attachment");
    assert_eq!(md.attachments[1].media_type, "text/markdown");

    let unknown = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id: request_id.clone(),
            file_name: "archive.zzz".to_owned(),
            contents: b"\x00\x01binary".to_vec(),
            expected_revision: 2,
        })
        .await
        .expect("add unknown attachment");
    assert_eq!(
        unknown.attachments[2].media_type,
        "application/octet-stream"
    );

    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: format!(
                "方案见 [plan](attachment://{pdf_id})，笔记见 [notes](attachment://{pdf_id})"
            ),
            expected_revision: 3,
        })
        .await
        .expect("save draft");
    let submitted = application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: draft.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
        })
        .await
        .expect("publish feedback");
    let result = submitted.feedback.expect("feedback package");
    let published = tokio::fs::read_to_string(&result.markdown_path)
        .await
        .expect("published Markdown");
    assert!(published.contains("[plan](attachments/001-plan.pdf)"));
    assert!(published.contains("[notes](attachments/001-plan.pdf)"));
    let published_pdf = Path::new(&result.directory_path).join("attachments/001-plan.pdf");
    assert_eq!(
        tokio::fs::read(published_pdf).await.expect("published pdf"),
        pdf_bytes
    );
    store.close().await;
}

#[tokio::test]
async fn request_list_filters_and_paginates_without_duplicates() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    for _ in 0..3 {
        application
            .request_feedback(workspace.request(Uuid::now_v7().to_string()))
            .await
            .expect("create request");
    }

    let first = application
        .list_feedback_requests(ListFeedbackRequestsInput {
            host_id: Some("test-host".to_owned()),
            limit: Some(2),
            ..Default::default()
        })
        .await
        .expect("first page");
    assert_eq!(first.requests.len(), 2);
    let cursor = first.next_cursor.expect("next cursor");
    let second = application
        .list_feedback_requests(ListFeedbackRequestsInput {
            host_id: Some("test-host".to_owned()),
            limit: Some(2),
            cursor: Some(cursor),
            ..Default::default()
        })
        .await
        .expect("second page");
    assert_eq!(second.requests.len(), 1);
    assert!(second.next_cursor.is_none());
    assert!(first.requests.iter().all(|left| {
        second
            .requests
            .iter()
            .all(|right| left.request_id != right.request_id)
    }));

    let invalid = application
        .list_feedback_requests(ListFeedbackRequestsInput {
            cursor: Some("not-a-cursor".to_owned()),
            ..Default::default()
        })
        .await
        .expect_err("invalid cursor");
    assert_eq!(invalid.code(), "INVALID_ARGUMENT");
    store.close().await;
}

#[tokio::test]
async fn host_session_navigation_reports_request_and_pending_counts() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();

    let mut first_request = workspace.request(Uuid::now_v7().to_string());
    first_request.title = Some("Session kickoff".to_owned());
    first_request.source_hint = Some("C:/workspace/rambledesk".to_owned());
    application
        .request_feedback(first_request)
        .await
        .expect("first request");

    let mut follow_up = workspace.request(Uuid::now_v7().to_string());
    follow_up.title = Some("Later review".to_owned());
    follow_up.source_hint = Some("a later hint".to_owned());
    application
        .request_feedback(follow_up)
        .await
        .expect("follow-up request");

    let second_id = Uuid::now_v7().to_string();
    let mut second = workspace.request(second_id.clone());
    second.host_session_id = "second-session".to_owned();
    application
        .request_feedback(second)
        .await
        .expect("second request");
    application
        .cancel_feedback(CancelFeedbackInput {
            request_id: second_id,
            reason: "Navigation fixture completed.".to_owned(),
        })
        .await
        .expect("cancel second request");

    let mut third = workspace.request(Uuid::now_v7().to_string());
    third.host_id = Some("other-host".to_owned());
    third.host_session_id = "other-session".to_owned();
    application
        .request_feedback(third)
        .await
        .expect("third request");

    let sessions = application
        .list_host_sessions()
        .await
        .expect("list host sessions");
    assert_eq!(sessions.len(), 3);
    let first = sessions
        .iter()
        .find(|session| session.host_session_id == "test-session")
        .expect("first session");
    assert_eq!(first.title, "Session kickoff");
    assert_eq!(
        first.source_hint.as_deref(),
        Some("C:/workspace/rambledesk")
    );
    assert_eq!(first.request_count, 2);
    assert_eq!(first.pending_count, 2);
    assert_eq!(first.pinned_at, None);
    assert_eq!(first.archived_at, None);
    assert_eq!(first.host_pinned_at, None);
    let second = sessions
        .iter()
        .find(|session| session.host_session_id == "second-session")
        .expect("second session");
    assert_eq!(second.request_count, 1);
    assert_eq!(second.pending_count, 0);
    store.close().await;
}

#[tokio::test]
async fn host_session_management_renames_session_and_orders_pins() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();

    let mut first = workspace.request(Uuid::now_v7().to_string());
    first.title = Some("Original title".to_owned());
    application
        .request_feedback(first)
        .await
        .expect("first request");

    let mut second = workspace.request(Uuid::now_v7().to_string());
    second.host_session_id = "second-session".to_owned();
    second.title = Some("Second session".to_owned());
    application
        .request_feedback(second)
        .await
        .expect("second request");

    let mut other_host = workspace.request(Uuid::now_v7().to_string());
    other_host.host_id = Some("other-host".to_owned());
    other_host.host_session_id = "other-session".to_owned();
    application
        .request_feedback(other_host)
        .await
        .expect("other host request");

    let renamed = application
        .rename_host_session(RenameHostSessionInput {
            host_id: "test-host".to_owned(),
            host_session_id: "test-session".to_owned(),
            title: "Renamed session".to_owned(),
        })
        .await
        .expect("rename session");
    assert_eq!(renamed.title, "Renamed session");

    let pinned = application
        .set_host_session_pinned(SetHostSessionPinnedInput {
            host_id: "test-host".to_owned(),
            host_session_id: "second-session".to_owned(),
            pinned: true,
        })
        .await
        .expect("pin session");
    assert!(pinned.pinned_at.is_some());

    application
        .set_host_pinned(SetHostPinnedInput {
            host_id: "other-host".to_owned(),
            pinned: true,
        })
        .await
        .expect("pin host");

    let sessions = application
        .list_host_sessions()
        .await
        .expect("list host sessions");
    assert_eq!(
        sessions
            .iter()
            .map(|session| (session.host_id.as_str(), session.host_session_id.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("other-host", "other-session"),
            ("test-host", "second-session"),
            ("test-host", "test-session"),
        ]
    );
    let first = sessions
        .iter()
        .find(|session| session.host_session_id == "test-session")
        .expect("renamed session listed");
    assert_eq!(first.title, "Renamed session");
    let other = sessions
        .iter()
        .find(|session| session.host_id == "other-host")
        .expect("pinned host listed");
    assert!(other.host_pinned_at.is_some());
    store.close().await;
}

#[tokio::test]
async fn archiving_a_session_requires_terminal_requests_and_filters_active_lists() {
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

    let archive_error = application
        .archive_host_session(HostSessionInput {
            host_id: "test-host".to_owned(),
            host_session_id: "test-session".to_owned(),
        })
        .await
        .expect_err("open session cannot be archived");
    assert_eq!(archive_error.code(), "HOST_SESSION_HAS_OPEN_REQUESTS");

    application
        .cancel_feedback(CancelFeedbackInput {
            request_id: request_id.clone(),
            reason: "Archive this completed fixture.".to_owned(),
        })
        .await
        .expect("cancel request");
    let archived = application
        .archive_host_session(HostSessionInput {
            host_id: "test-host".to_owned(),
            host_session_id: "test-session".to_owned(),
        })
        .await
        .expect("archive terminal session");
    assert!(archived.archived_at.is_some());
    assert_eq!(
        application
            .list_host_sessions()
            .await
            .expect("active sessions"),
        Vec::new()
    );

    let active_requests = application
        .list_feedback_requests(ListFeedbackRequestsInput {
            status: Some(vec![
                FeedbackStatus::Waiting,
                FeedbackStatus::InProgress,
                FeedbackStatus::Completed,
                FeedbackStatus::Cancelled,
            ]),
            ..Default::default()
        })
        .await
        .expect("active requests");
    assert!(active_requests.requests.is_empty());

    let archived_requests = application
        .list_feedback_requests(ListFeedbackRequestsInput {
            status: Some(vec![
                FeedbackStatus::Waiting,
                FeedbackStatus::InProgress,
                FeedbackStatus::Completed,
                FeedbackStatus::Cancelled,
            ]),
            archived: Some(true),
            ..Default::default()
        })
        .await
        .expect("archived requests");
    assert_eq!(archived_requests.requests.len(), 1);
    assert_eq!(archived_requests.requests[0].request_id, request_id);
    let archived_sessions = application
        .list_archived_host_sessions(ListHostSessionsInput {
            search: Some("Persistence".to_owned()),
        })
        .await
        .expect("search archived sessions");
    assert_eq!(archived_sessions.len(), 1);
    assert_eq!(archived_sessions[0].host_session_id, "test-session");

    application
        .request_feedback(workspace.request(Uuid::now_v7().to_string()))
        .await
        .expect("new request unarchives session");
    let sessions = application
        .list_host_sessions()
        .await
        .expect("active sessions after new request");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].host_session_id, "test-session");
    assert_eq!(sessions[0].request_count, 2);
    assert_eq!(sessions[0].pending_count, 1);
    assert_eq!(sessions[0].archived_at, None);
    store.close().await;
}

#[tokio::test]
async fn delete_feedback_request_requires_archived_session_and_removes_files() {
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

    let open_delete = application
        .delete_feedback_request(DeleteFeedbackRequestInput {
            request_id: request_id.clone(),
        })
        .await
        .expect_err("active request cannot be deleted");
    assert_eq!(open_delete.code(), "DELETE_REQUIRES_ARCHIVED_HOST_SESSION");

    let cancelled = application
        .cancel_feedback(CancelFeedbackInput {
            request_id: request_id.clone(),
            reason: "Deleting archived fixture.".to_owned(),
        })
        .await
        .expect("cancel request");
    let package_dir = cancelled
        .feedback
        .as_ref()
        .expect("cancellation package")
        .directory_path
        .clone();
    assert!(Path::new(&package_dir).is_dir());
    application
        .archive_host_session(HostSessionInput {
            host_id: "test-host".to_owned(),
            host_session_id: "test-session".to_owned(),
        })
        .await
        .expect("archive terminal session");

    application
        .delete_feedback_request(DeleteFeedbackRequestInput {
            request_id: request_id.clone(),
        })
        .await
        .expect("delete archived request");
    let missing = application
        .get_feedback(GetFeedbackInput { request_id })
        .await
        .expect_err("request row removed");
    assert_eq!(missing.code(), "REQUEST_NOT_FOUND");
    assert!(!Path::new(&package_dir).exists());
    assert!(
        application
            .list_archived_host_sessions(ListHostSessionsInput::default())
            .await
            .expect("archived sessions")
            .is_empty()
    );
    store.close().await;
}

#[tokio::test]
async fn delete_host_session_removes_all_archived_requests() {
    let workspace = TestWorkspace::new().await;
    let first_id = Uuid::now_v7().to_string();
    let second_id = Uuid::now_v7().to_string();
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let application = store.clone().into_application();
    application
        .request_feedback(workspace.request(first_id.clone()))
        .await
        .expect("first request");
    application
        .request_feedback(workspace.request(second_id.clone()))
        .await
        .expect("second request");
    let first = application
        .cancel_feedback(CancelFeedbackInput {
            request_id: first_id.clone(),
            reason: "Delete session fixture one.".to_owned(),
        })
        .await
        .expect("cancel first");
    let second = application
        .cancel_feedback(CancelFeedbackInput {
            request_id: second_id.clone(),
            reason: "Delete session fixture two.".to_owned(),
        })
        .await
        .expect("cancel second");
    let package_dirs = [first, second]
        .into_iter()
        .map(|view| view.feedback.expect("package").directory_path)
        .collect::<Vec<_>>();
    application
        .archive_host_session(HostSessionInput {
            host_id: "test-host".to_owned(),
            host_session_id: "test-session".to_owned(),
        })
        .await
        .expect("archive session");

    application
        .delete_host_session(HostSessionInput {
            host_id: "test-host".to_owned(),
            host_session_id: "test-session".to_owned(),
        })
        .await
        .expect("delete archived session");
    for request_id in [first_id, second_id] {
        let missing = application
            .get_feedback(GetFeedbackInput { request_id })
            .await
            .expect_err("request row removed");
        assert_eq!(missing.code(), "REQUEST_NOT_FOUND");
    }
    for package_dir in package_dirs {
        assert!(!Path::new(&package_dir).exists());
    }
    store.close().await;
}

#[tokio::test]
async fn completed_without_result_is_reported_as_corrupt() {
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
    sqlx::query(
        "UPDATE feedback_requests SET status = 'completed', completed_at = ?2, \
         updated_at = ?2 WHERE id = ?1",
    )
    .bind(&request_id)
    .bind("2026-07-29T14:30:00Z")
    .execute(&store.pool)
    .await
    .expect("corrupt completed fixture");
    let error = application
        .get_feedback(GetFeedbackInput { request_id })
        .await
        .expect_err("missing result must not look completed");
    assert_eq!(error.code(), "STORAGE_FAILURE");
    store.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn existing_database_permissions_are_repaired() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let database = directory.path().join("rambledesk.sqlite3");
    tokio::fs::write(&database, [])
        .await
        .expect("empty database file");
    tokio::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o644))
        .await
        .expect("permissive fixture");

    let store = SqliteFeedbackStore::connect(&database)
        .await
        .expect("open store");
    let mode = tokio::fs::metadata(&database)
        .await
        .expect("database metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    store.close().await;
}

#[tokio::test]
async fn migration_installs_the_full_foundation_contract() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master \
         WHERE type IN ('table', 'trigger', 'index') AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&store.pool)
    .await
    .expect("schema objects");
    for expected in [
        "host_sessions",
        "feedback_requests",
        "request_actions",
        "request_context_refs",
        "request_attachments",
        "drafts",
        "attachments",
        "feedback_results",
        "submission_plans",
        "host_preferences",
        "feedback_requests_completed_is_terminal",
        "feedback_requests_cancelled_is_terminal",
        "feedback_requests_status_updated",
        "feedback_requests_host_session_updated",
        "drafts_locked_after_submission_plan_update",
        "drafts_locked_after_submission_plan_delete",
        "host_sessions_host",
        "host_sessions_archive_pin_updated",
        "host_preferences_pin",
        "request_attachments_request",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "missing {expected}"
        );
    }

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
