use super::*;
use rambledesk_core::{
    ApplicationChangeHub, ApplicationResourceKey, ApproveFeedbackInput, CancelFeedbackInput,
    GetFeedbackInput, HostSessionInput, SaveDraftInput, SubmitFeedbackInput,
};
use std::sync::Arc;

#[tokio::test]
async fn successful_mutations_notify_once_while_queries_and_errors_do_not() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let hub = Arc::new(ApplicationChangeHub::with_runtime_generation("runtime-a"));
    let application = store.into_application().with_change_observer(hub.clone());
    let mut changes = hub.subscribe();
    let request_id = Uuid::now_v7().to_string();

    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");
    let created = changes.recv().await.expect("creation invalidation");
    assert_eq!(created.revision, "1");
    assert_eq!(
        created.resources,
        vec![
            ApplicationResourceKey::Navigation,
            ApplicationResourceKey::FeedbackWorkspace {
                request_id: request_id.clone(),
            },
        ]
    );

    application
        .get_feedback(GetFeedbackInput {
            request_id: request_id.clone(),
        })
        .await
        .expect("query request");
    assert!(changes.try_recv().is_err(), "queries must not invalidate");

    application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            document_json: r#"{"type":"doc","content":[]}"#.to_owned(),
            body_markdown: String::new(),
            expected_revision: 99,
        })
        .await
        .expect_err("stale draft save");
    assert!(changes.try_recv().is_err(), "errors must not invalidate");

    application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            document_json: r#"{"type":"doc","content":[]}"#.to_owned(),
            body_markdown: String::new(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");
    let saved = changes.recv().await.expect("draft invalidation");
    assert_eq!(saved.revision, "2");
    assert_eq!(
        saved.resources,
        vec![
            ApplicationResourceKey::Navigation,
            ApplicationResourceKey::FeedbackWorkspace { request_id },
        ]
    );
    assert!(
        changes.try_recv().is_err(),
        "mutation must notify exactly once"
    );
}

#[tokio::test]
async fn attachment_mutations_invalidate_navigation_and_workspace_once() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let hub = Arc::new(ApplicationChangeHub::with_runtime_generation("runtime-a"));
    let application = store.into_application().with_change_observer(hub.clone());
    let mut changes = hub.subscribe();
    let request_id = Uuid::now_v7().to_string();
    application
        .request_feedback(workspace.request(request_id.clone()))
        .await
        .expect("create request");
    changes.recv().await.expect("creation invalidation");

    let first = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id: request_id.clone(),
            file_name: "first.txt".into(),
            contents: b"first".to_vec(),
            expected_revision: 0,
        })
        .await
        .expect("add first attachment");
    let first_id = first.attachments[0].attachment_id.clone();
    assert_navigation_and_workspace_change(&mut changes, &request_id).await;

    let second = application
        .add_feedback_attachment(AddAttachmentInput {
            request_id: request_id.clone(),
            file_name: "second.txt".into(),
            contents: b"second".to_vec(),
            expected_revision: 1,
        })
        .await
        .expect("add second attachment");
    let second_id = second.attachments[1].attachment_id.clone();
    assert_navigation_and_workspace_change(&mut changes, &request_id).await;

    application
        .reorder_feedback_attachments(ReorderAttachmentsInput {
            request_id: request_id.clone(),
            attachment_ids: vec![second_id, first_id.clone()],
            expected_revision: 2,
        })
        .await
        .expect("reorder attachments");
    assert_navigation_and_workspace_change(&mut changes, &request_id).await;

    application
        .remove_feedback_attachment(RemoveAttachmentInput {
            request_id: request_id.clone(),
            attachment_id: first_id,
            expected_revision: 3,
        })
        .await
        .expect("remove attachment");
    assert_navigation_and_workspace_change(&mut changes, &request_id).await;
    assert!(
        changes.try_recv().is_err(),
        "mutations must notify exactly once"
    );
}

async fn assert_navigation_and_workspace_change(
    changes: &mut tokio::sync::broadcast::Receiver<rambledesk_core::ApplicationInvalidation>,
    request_id: &str,
) {
    let change = changes.recv().await.expect("application invalidation");
    assert_eq!(
        change.resources,
        vec![
            ApplicationResourceKey::Navigation,
            ApplicationResourceKey::FeedbackWorkspace {
                request_id: request_id.to_owned(),
            },
        ]
    );
}

#[tokio::test]
async fn idempotent_request_and_terminal_replays_do_not_advance_the_revision() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let hub = Arc::new(ApplicationChangeHub::with_runtime_generation("runtime-a"));
    let application = store.into_application().with_change_observer(hub.clone());

    let request_input = workspace.request(Uuid::now_v7().to_string());
    application
        .request_feedback(request_input.clone())
        .await
        .expect("create request");
    let after_create = hub.metadata().revision;
    application
        .request_feedback(request_input)
        .await
        .expect("replay request");
    assert_eq!(hub.metadata().revision, after_create);

    let approve_id = Uuid::now_v7().to_string();
    let mut approve_input = workspace.request(approve_id.clone());
    approve_input.allow_finish = true;
    approve_input.final_summary = Some("The requested work is complete.".into());
    application
        .request_feedback(approve_input)
        .await
        .expect("create approval request");
    application
        .approve_feedback(ApproveFeedbackInput {
            request_id: approve_id.clone(),
        })
        .await
        .expect("approve");
    let after_approve = hub.metadata().revision;
    application
        .approve_feedback(ApproveFeedbackInput {
            request_id: approve_id,
        })
        .await
        .expect("replay approval");
    assert_eq!(hub.metadata().revision, after_approve);

    let cancel_id = Uuid::now_v7().to_string();
    application
        .request_feedback(workspace.request(cancel_id.clone()))
        .await
        .expect("create cancellation request");
    application
        .cancel_feedback(CancelFeedbackInput {
            request_id: cancel_id.clone(),
            reason: "No longer needed.".into(),
        })
        .await
        .expect("cancel");
    let after_cancel = hub.metadata().revision;
    application
        .cancel_feedback(CancelFeedbackInput {
            request_id: cancel_id,
            reason: "Replay must not change facts.".into(),
        })
        .await
        .expect("replay cancellation");
    assert_eq!(hub.metadata().revision, after_cancel);

    let submit_id = Uuid::now_v7().to_string();
    application
        .request_feedback(workspace.request(submit_id.clone()))
        .await
        .expect("create submission request");
    let draft = application
        .save_feedback_draft(SaveDraftInput {
            request_id: submit_id.clone(),
            document_json: r#"{"type":"doc","content":[]}"#.into(),
            body_markdown: "Ship it.".into(),
            expected_revision: 0,
        })
        .await
        .expect("save draft");
    application
        .submit_feedback(SubmitFeedbackInput {
            request_id: submit_id.clone(),
            expected_revision: draft.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .expect("submit");
    let after_submit = hub.metadata().revision;
    application
        .submit_feedback(SubmitFeedbackInput {
            request_id: submit_id,
            expected_revision: 0,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .expect("replay submission");
    assert_eq!(hub.metadata().revision, after_submit);
}

#[tokio::test]
async fn concurrent_request_retries_emit_one_change() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let hub = Arc::new(ApplicationChangeHub::with_runtime_generation("runtime-a"));
    let application = store.into_application().with_change_observer(hub.clone());
    let input = workspace.request(Uuid::now_v7().to_string());
    let attempts = (0..8)
        .map(|_| {
            let application = application.clone();
            let input = input.clone();
            tokio::spawn(async move { application.request_feedback(input).await })
        })
        .collect::<Vec<_>>();
    for attempt in attempts {
        attempt.await.expect("request task").expect("request retry");
    }
    assert_eq!(hub.metadata().revision, "1");
}

#[tokio::test]
async fn deleting_a_session_invalidates_all_request_scoped_resources() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .expect("open store");
    let hub = Arc::new(ApplicationChangeHub::with_runtime_generation("runtime-a"));
    let application = store.into_application().with_change_observer(hub.clone());
    let mut changes = hub.subscribe();
    let mut request_ids = Vec::new();
    for _ in 0..2 {
        let request_id = Uuid::now_v7().to_string();
        application
            .request_feedback(workspace.request(request_id.clone()))
            .await
            .expect("create session request");
        changes.recv().await.expect("creation invalidation");
        request_ids.push(request_id);
    }
    for request_id in &request_ids {
        application
            .cancel_feedback(CancelFeedbackInput {
                request_id: request_id.clone(),
                reason: "Prepare session deletion.".into(),
            })
            .await
            .expect("cancel session request");
        changes.recv().await.expect("cancellation invalidation");
    }
    let identity = HostSessionInput {
        host_id: "test-host".into(),
        host_session_id: "test-session".into(),
    };
    application
        .archive_host_session(identity.clone())
        .await
        .expect("archive session");
    changes.recv().await.expect("archive invalidation");
    application
        .delete_host_session(identity)
        .await
        .expect("delete session");
    let deleted = changes.recv().await.expect("delete invalidation");
    assert_eq!(
        &deleted.resources[..2],
        &[
            ApplicationResourceKey::Navigation,
            ApplicationResourceKey::HostSessionResources {
                host_id: "test-host".into(),
                host_session_id: "test-session".into(),
            },
        ]
    );
    let scoped_resources = deleted.resources[2..]
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let expected_scoped_resources = request_ids
        .iter()
        .flat_map(|request_id| {
            [
                ApplicationResourceKey::FeedbackWorkspace {
                    request_id: request_id.clone(),
                },
                ApplicationResourceKey::PublishedFeedback {
                    request_id: request_id.clone(),
                },
            ]
        })
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(scoped_resources, expected_scoped_resources);
}
