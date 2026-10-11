use super::*;

#[tokio::test]
async fn brainstorm_copies_only_card_references_once_and_child_survives_parent_deletion() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let parent = app
        .request_feedback(request(&workspace, None))
        .await
        .unwrap();
    let first = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: parent.request_id.clone(),
            file_name: "card.txt".into(),
            contents: b"Card evidence".to_vec(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    let second = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: parent.request_id.clone(),
            file_name: "overall.txt".into(),
            contents: b"Overall notes only".to_vec(),
            expected_revision: first.draft.saved_revision,
        })
        .await
        .unwrap();
    let card_id = second.attachments[0].attachment_id.clone();
    let overall_id = second.attachments[1].attachment_id.clone();
    let card_ref = format!("[Card](attachment://{card_id})");
    let mut state = serde_json::from_str::<Value>(second.draft.document_json.as_ref().unwrap())
        .unwrap()["workbenchState"]
        .clone();
    state["cards"][0]["body"] = json!(card_ref);
    let mut human = state["cards"][0].clone();
    human["id"] = json!("human");
    human["origin"] = json!("human");
    state["cards"].as_array_mut().unwrap().push(human);
    state["selected_card_ids"] = json!(["small-task"]);
    state["intent"] = json!("expand");
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: parent.request_id.clone(),
            document_json: document(state),
            body_markdown: format!("[Overall](attachment://{overall_id})"),
            expected_revision: second.draft.saved_revision,
        })
        .await
        .unwrap();
    let published = app
        .submit_feedback(submission(&parent.request_id, saved.saved_revision))
        .await
        .unwrap();
    let parent_result = published.feedback.as_ref().unwrap();
    let parent_package = app
        .read_feedback_package(&published)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parent_package.manifest.attachments.len(), 2);
    let original_manifest = tokio::fs::read(&parent_result.manifest_path).await.unwrap();
    let original_markdown = tokio::fs::read(&parent_result.markdown_path).await.unwrap();
    let original_bytes = tokio::fs::read(&parent_package.attachment_paths[0])
        .await
        .unwrap();

    let child_input = request(&workspace, Some(&parent.request_id));
    let child = app.request_feedback(child_input.clone()).await.unwrap();
    let inherited = app
        .get_feedback_workspace(child.request_id.clone())
        .await
        .unwrap();
    assert_eq!(inherited.attachments.len(), 1);
    let new_id = &inherited.attachments[0].attachment_id;
    assert_ne!(new_id, &card_id);
    let mut inherited_state = serde_json::from_str::<Value>(
        inherited.draft.document_json.as_ref().unwrap(),
    )
    .unwrap()["workbenchState"]
        .clone();
    for index in [0, 1] {
        assert_eq!(
            inherited_state["cards"][index]["body"],
            json!(format!("[Card](attachment://{new_id})"))
        );
    }
    assert_eq!(
        tokio::fs::read(&parent_result.manifest_path).await.unwrap(),
        original_manifest
    );
    assert_eq!(
        tokio::fs::read(&parent_result.markdown_path).await.unwrap(),
        original_markdown
    );
    assert_eq!(
        tokio::fs::read(&parent_package.attachment_paths[0])
            .await
            .unwrap(),
        original_bytes
    );
    assert_eq!(
        FeedbackRepository::read_attachment(&store, &child.request_id, new_id)
            .await
            .unwrap(),
        b"Card evidence"
    );
    inherited_state["intent"] = json!("proceed");
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: child.request_id.clone(),
            document_json: document(inherited_state),
            body_markdown: String::new(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    let child_result = app
        .submit_feedback(submission(&child.request_id, saved.saved_revision))
        .await
        .unwrap();
    app.archive_host_session(HostSessionInput {
        host_id: "test-host".into(),
        host_session_id: "test-session".into(),
    })
    .await
    .unwrap();
    app.delete_feedback_request(DeleteFeedbackRequestInput {
        request_id: parent.request_id.clone(),
    })
    .await
    .unwrap();
    assert!(!std::path::Path::new(&parent_result.directory_path).exists());
    let package = app
        .read_feedback_package(&child_result)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(package.attachment_paths.len(), 1);
    assert_eq!(
        tokio::fs::read(&package.attachment_paths[0]).await.unwrap(),
        b"Card evidence"
    );
    // Replay succeeds even when its former source is deleted; it is not rehydrated.
    assert_eq!(
        app.request_feedback(child_input).await.unwrap().status,
        FeedbackStatus::Completed
    );
    store.close().await;
}
