use std::{collections::BTreeMap, future::Future, task::Poll};

use rambledesk_core::{
    AgentConfig, FeedbackDeliveryRepository, FeedbackPackagePublisher, ManagedFeedbackScope,
    NewManagedSession, NewSessionActivity, SessionActivityKind, SessionActivityRepository,
    SessionDeletionRepository, SessionProtocol, SessionRepository, SessionRepositoryError,
};

use super::*;

const NOW: &str = "2026-09-04T02:00:00Z";

async fn setup() -> (TestWorkspace, SqliteFeedbackStore) {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    store
        .save_agent_config(AgentConfig {
            catalog_id: None,
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
    (workspace, store)
}

async fn request(
    workspace: &TestWorkspace,
    store: &SqliteFeedbackStore,
    session: Option<&str>,
) -> String {
    let id = Uuid::now_v7().to_string();
    let mut input = workspace.request(id.clone());
    input.attachments.push(RequestAttachmentInput {
        file_name: "context.md".into(),
        markdown: Some("Owned request attachment".into()),
        contents_base64: None,
        path: None,
    });
    let app = store.clone().into_application();
    if let Some(session_id) = session {
        app.request_managed_feedback(
            &ManagedFeedbackScope {
                session_id: session_id.into(),
                host_id: "dsh".into(),
                host_session_id: session_id.into(),
            },
            input,
        )
        .await
        .unwrap();
    } else {
        app.request_feedback(input).await.unwrap();
    }
    id
}

async fn draft(store: &SqliteFeedbackStore, id: &str) -> u64 {
    let app = store.clone().into_application();
    app.add_feedback_attachment(AddAttachmentInput {
        request_id: id.into(),
        expected_revision: 0,
        file_name: "review.png".into(),
        contents: b"\x89PNG\r\n\x1a\nimage".to_vec(),
    })
    .await
    .unwrap();
    app.save_feedback_draft(SaveDraftInput {
        request_id: id.into(),
        expected_revision: 1,
        document_json: r#"{"schemaVersion":2,"doc":{"type":"doc"}}"#.into(),
        body_markdown: "Continue with these changes.".into(),
    })
    .await
    .unwrap()
    .saved_revision
}

async fn plan(store: &SqliteFeedbackStore, id: &str) -> SubmissionPlan {
    let revision = draft(store, id).await;
    store
        .plan_submission(SubmissionPlanInput {
            request_id: id,
            expected_revision: revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
            publication_id: &Uuid::now_v7().to_string(),
            now: NOW,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn direct_deletion_cleans_owned_data_and_files_without_archiving() {
    let (workspace, store) = setup().await;
    let waiting = request(&workspace, &store, Some("one")).await;
    let active = request(&workspace, &store, Some("one")).await;
    draft(&store, &active).await;
    let completed = request(&workspace, &store, Some("one")).await;
    let plan = plan(&store, &completed).await;
    let package = store.publish(&plan).await.unwrap();
    store.complete_submission(&plan, &package).await.unwrap();
    let other = request(&workspace, &store, Some("two")).await;
    let external = request(&workspace, &store, None).await;
    for session in ["one", "two"] {
        store
            .append_activity(NewSessionActivity {
                id: format!("activity-{session}"),
                session_id: session.into(),
                turn_id: None,
                kind: SessionActivityKind::UserMessage,
                text: "Work".into(),
                content: None,
                tool_call_id: None,
                created_at: NOW.into(),
            })
            .await
            .unwrap();
    }
    assert_eq!(store.list_session_deliveries("one").await.unwrap().len(), 1);
    store
        .begin_managed_session_deletion("one", NOW)
        .await
        .unwrap();
    let deleted = store.delete_managed_session_data("one").await.unwrap();
    let mut expected = vec![waiting.clone(), active.clone(), completed.clone()];
    expected.sort();
    assert_eq!(deleted.request_ids, expected);
    assert_eq!(
        (
            deleted.session_id.as_str(),
            deleted.host_id.as_str(),
            deleted.host_session_id.as_str()
        ),
        ("one", "dsh", "one")
    );
    for id in [&waiting, &active, &completed] {
        assert!(matches!(
            store.get_request(id).await,
            Err(RepositoryError::RequestNotFound)
        ));
        assert!(!store.library_root().join("drafts").join(id).exists());
        for table in [
            "drafts",
            "attachments",
            "request_attachments",
            "request_actions",
            "request_context_refs",
            "submission_plans",
            "feedback_results",
            "feedback_deliveries",
        ] {
            let count: i64 =
                sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE request_id=?1"))
                    .bind(id)
                    .fetch_one(&store.pool)
                    .await
                    .unwrap();
            assert_eq!(count, 0, "{table}");
        }
    }
    assert!(!Path::new(&plan.directory_path).exists());
    assert!(!Path::new(&plan.temp_directory_path).exists());
    assert!(
        store
            .list_managed_session_deletions()
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store.get_session("one").await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    assert!(store.get_request(&other).await.is_ok());
    assert!(store.get_request(&external).await.is_ok());
    assert!(store.library_root().join("drafts").join(&other).exists());
    assert!(store.library_root().join("drafts").join(&external).exists());
    assert_eq!(
        store
            .list_session_activity("two", None, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session_activity WHERE session_id='one'")
            .fetch_one(&store.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        store.delete_managed_session_data("one").await,
        Err(SessionRepositoryError::SessionNotFound)
    );
}

#[tokio::test]
async fn deletion_intent_survives_restart_blocks_requests_and_is_idempotent() {
    let (workspace, store) = setup().await;
    let id = request(&workspace, &store, Some("one")).await;
    assert_eq!(
        store.delete_managed_session_data("one").await,
        Err(SessionRepositoryError::Conflict)
    );
    store
        .begin_managed_session_deletion("one", NOW)
        .await
        .unwrap();
    store
        .begin_managed_session_deletion("one", "later")
        .await
        .unwrap();
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        store.list_managed_session_deletions().await.unwrap(),
        vec!["one"]
    );
    assert!(store.is_managed_session_deleting("one").await.unwrap());
    let began: String =
        sqlx::query_scalar("SELECT started_at FROM session_deletions WHERE session_id='one'")
            .fetch_one(&store.pool)
            .await
            .unwrap();
    assert_eq!(began, NOW);
    let app = store.clone().into_application();
    for request_id in [id, Uuid::now_v7().to_string()] {
        let result = app
            .request_managed_feedback(
                &ManagedFeedbackScope {
                    session_id: "one".into(),
                    host_id: "dsh".into(),
                    host_session_id: "one".into(),
                },
                workspace.request(request_id),
            )
            .await;
        assert!(result.is_err());
    }
    store.delete_managed_session_data("one").await.unwrap();
    assert!(!store.is_managed_session_deleting("one").await.unwrap());
}

#[tokio::test]
async fn external_sessions_are_never_deleted_through_managed_port() {
    let (workspace, store) = setup().await;
    let id = request(&workspace, &store, None).await;
    let session_id: String =
        sqlx::query_scalar("SELECT host_session_record_id FROM feedback_requests WHERE id=?1")
            .bind(&id)
            .fetch_one(&store.pool)
            .await
            .unwrap();
    assert_eq!(
        store.begin_managed_session_deletion(&session_id, NOW).await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(
        store.delete_managed_session_data(&session_id).await,
        Err(SessionRepositoryError::Conflict)
    );
    assert!(store.get_request(&id).await.is_ok());
    assert!(
        store
            .list_managed_session_deletions()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn file_failure_retains_metadata_and_intent_for_retry() {
    let (_workspace, store) = setup().await;
    let bad_path = store.library_root().join("drafts");
    tokio::fs::write(&bad_path, b"unexpected file")
        .await
        .unwrap();
    // No requests is valid, so seed one without filesystem attachments.
    store
        .create_or_get_request(NewFeedbackRequest {
            request_id: "owned".into(),
            host_session_record_id: "one".into(),
            managed_session_id: Some("one".into()),
            host_id: "dsh".into(),
            host_session_id: "one".into(),
            title: "Review".into(),
            what_happened: "Work".into(),
            actions: vec![],
            context_refs: vec![],
            attachments: vec![],
            source_hint: None,
            allow_finish: false,
            final_summary: None,
            created_at: NOW.into(),
        })
        .await
        .unwrap();
    store
        .begin_managed_session_deletion("one", NOW)
        .await
        .unwrap();
    assert_eq!(
        store.delete_managed_session_data("one").await,
        Err(SessionRepositoryError::Storage)
    );
    assert!(store.get_request("owned").await.is_ok());
    assert!(store.is_managed_session_deleting("one").await.unwrap());
    tokio::fs::remove_file(&bad_path).await.unwrap();
    store.delete_managed_session_data("one").await.unwrap();
}

#[tokio::test]
async fn failed_database_commit_after_cleanup_can_resume_without_republishing() {
    let (workspace, store) = setup().await;
    let id = request(&workspace, &store, Some("one")).await;
    let plan = plan(&store, &id).await;
    store.publish(&plan).await.unwrap();
    store
        .begin_managed_session_deletion("one", NOW)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_delete BEFORE DELETE ON host_sessions BEGIN SELECT RAISE(ABORT,'test failure'); END").execute(&store.pool).await.unwrap();
    assert_eq!(
        store.delete_managed_session_data("one").await,
        Err(SessionRepositoryError::Storage)
    );
    assert!(!Path::new(&plan.directory_path).exists());
    assert!(store.get_request(&id).await.is_ok());
    sqlx::query("DROP TRIGGER fail_delete")
        .execute(&store.pool)
        .await
        .unwrap();
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert!(store.is_managed_session_deleting("one").await.unwrap());
    assert!(!Path::new(&plan.directory_path).exists());
    store.delete_managed_session_data("one").await.unwrap();
}

#[tokio::test]
async fn unsafe_persisted_paths_are_rejected_before_any_cleanup() {
    let (workspace, store) = setup().await;
    let id = request(&workspace, &store, Some("one")).await;
    let plan = plan(&store, &id).await;
    let outside = workspace._temp.path().join(format!("outside-{id}"));
    tokio::fs::create_dir_all(&outside).await.unwrap();
    tokio::fs::write(outside.join("sentinel"), b"keep")
        .await
        .unwrap();
    store
        .begin_managed_session_deletion("one", NOW)
        .await
        .unwrap();
    for path in [
        outside.clone(),
        store.library_root(),
        store.library_root().join("feedback").join("wrong-owner"),
        store
            .library_root()
            .join("feedback")
            .join("..")
            .join(format!("outside-{id}")),
    ] {
        sqlx::query("UPDATE submission_plans SET directory_path=?2 WHERE request_id=?1")
            .bind(&id)
            .bind(path.to_str().unwrap())
            .execute(&store.pool)
            .await
            .unwrap();
        assert_eq!(
            store.delete_managed_session_data("one").await,
            Err(SessionRepositoryError::CorruptData)
        );
        assert!(store.library_root().join("drafts").join(&id).exists());
        assert!(outside.join("sentinel").is_file());
    }
    sqlx::query("UPDATE submission_plans SET directory_path=?2 WHERE request_id=?1")
        .bind(&id)
        .bind(&plan.directory_path)
        .execute(&store.pool)
        .await
        .unwrap();
    store.delete_managed_session_data("one").await.unwrap();
    assert!(outside.join("sentinel").is_file());
}

#[tokio::test]
async fn publisher_queued_behind_deletion_cannot_recreate_old_package() {
    let (workspace, store) = setup().await;
    let id = request(&workspace, &store, Some("one")).await;
    let plan = plan(&store, &id).await;
    store
        .begin_managed_session_deletion("one", NOW)
        .await
        .unwrap();
    let guard = store.publish_lock.lock().await;
    let mut deletion = Box::pin(store.delete_managed_session_data("one"));
    // Poll both operations while locked, proving they are concurrently queued in
    // FIFO order rather than relying on sleeps or task scheduling assumptions.
    std::future::poll_fn(|cx| match deletion.as_mut().poll(cx) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(_) => panic!("deletion did not wait for publication lock"),
    })
    .await;
    let mut publication = Box::pin(store.publish(&plan));
    std::future::poll_fn(|cx| match publication.as_mut().poll(cx) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(_) => panic!("publication did not wait for lock"),
    })
    .await;
    drop(guard);
    let (deleted, published) = tokio::join!(deletion, publication);
    assert!(deleted.is_ok());
    assert!(matches!(published, Err(RepositoryError::RequestNotFound)));
    assert!(!Path::new(&plan.directory_path).exists());
    assert!(!Path::new(&plan.temp_directory_path).exists());
    assert!(!store.library_root().join("drafts").join(&id).exists());
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn directory_link_cannot_redirect_cleanup_outside_the_library() {
    let (workspace, store) = setup().await;
    let id = request(&workspace, &store, Some("one")).await;
    let plan = plan(&store, &id).await;
    let outside = workspace._temp.path().join("outside");
    tokio::fs::create_dir_all(&outside).await.unwrap();
    tokio::fs::write(outside.join("sentinel"), b"keep")
        .await
        .unwrap();
    let link = Path::new(&plan.directory_path);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, link).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Junctions require no developer-mode or administrator privilege.
        let output = std::process::Command::new("powershell.exe")
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:RAMBLEDESK_TEST_LINK -Target $env:RAMBLEDESK_TEST_TARGET | Out-Null"])
            .env("RAMBLEDESK_TEST_LINK", link)
            .env("RAMBLEDESK_TEST_TARGET", &outside)
            .output().unwrap();
        assert!(
            output.status.success(),
            "junction fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    store
        .begin_managed_session_deletion("one", NOW)
        .await
        .unwrap();
    assert_eq!(
        store.delete_managed_session_data("one").await,
        Err(SessionRepositoryError::CorruptData)
    );
    assert!(outside.join("sentinel").is_file());
    assert!(store.library_root().join("drafts").join(&id).exists());
    // Remove only the link itself before the temporary workspace is cleaned.
    #[cfg(unix)]
    tokio::fs::remove_file(link).await.unwrap();
    #[cfg(windows)]
    tokio::fs::remove_dir(link).await.unwrap();
    store.delete_managed_session_data("one").await.unwrap();
    assert!(outside.join("sentinel").is_file());
}
