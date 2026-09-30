use super::*;

fn session() -> Value {
    json!({"id":"terminal-1","cwd":"/prepared/project","shell":"sh","cols":80,"rows":24,
        "status":"stopped","exit_code":null,
        "output":"\u{1b}[32mUsage: my-cli 😀\u{1b}[0m\r\n", "screen":"Usage: my-cli 😀", "truncated":true})
}

fn document(state: Value) -> String {
    json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state}).to_string()
}

#[tokio::test]
async fn terminal_trial_reopens_and_freezes_feedback_with_original_output() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(original_workbench_example("terminal"));
    let created = app.request_feedback(input.clone()).await.unwrap();
    let attachment_bytes = b"CLI trial reference".to_vec();
    let uploaded = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: created.request_id.clone(),
            file_name: "reference.txt".into(),
            contents: attachment_bytes.clone(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    let attachment_id = &uploaded.attachments[0].attachment_id;
    let evidence = document(json!({"type":"terminal","sessions":[session()]}));
    let saved = app.save_feedback_draft(SaveDraftInput {request_id:created.request_id.clone(),document_json:evidence.clone(),
        body_markdown:format!("The command help is clear; [reference](attachment://{attachment_id}) explains the menu."), expected_revision:uploaded.draft.saved_revision}).await.unwrap();
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let reopened = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(reopened.workbench, input.workbench);
    assert_eq!(
        reopened.draft.document_json.as_deref(),
        Some(evidence.as_str())
    );
    let result = app
        .submit_feedback(SubmitFeedbackInput {
            request_id: created.request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .unwrap();
    let package = app.read_feedback_package(&result).await.unwrap().unwrap();
    let published = serde_json::to_value(package.manifest.workbench.unwrap()).unwrap();
    assert_eq!(published["result"]["sessions"][0], session());
    assert!(package.markdown.contains("The command help is clear"));
    assert!(package.markdown.contains("attachments/"));
    assert!(!package.markdown.contains("attachment://"));
    assert_eq!(
        tokio::fs::read(&package.attachment_paths[0]).await.unwrap(),
        attachment_bytes
    );
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let frozen = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(frozen.request.status, FeedbackStatus::Completed);
    assert_eq!(
        frozen.draft.document_json.as_deref(),
        Some(evidence.as_str())
    );
    let package_again = app.read_feedback_package(&result).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(package_again.manifest.workbench.unwrap()).unwrap(),
        published
    );
    store.close().await;
}

#[tokio::test]
async fn terminal_rejects_malformed_or_oversized_state_without_publishing_notes_alone() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut bad_dimension = session();
    bad_dimension["cols"] = json!(1);
    let mut bad_capture = session();
    bad_capture["output"] = json!("x".repeat(262145));
    for state in [
        json!({"type":"terminal","sessions":[{"id":"broken"}]}),
        json!({"type":"terminal","sessions":[],"future":"unknown"}),
        json!({"type":"terminal","sessions":[bad_dimension]}),
        json!({"type":"terminal","sessions":[bad_capture]}),
    ] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(original_workbench_example("terminal"));
        let created = app.request_feedback(input).await.unwrap();
        let saved = app
            .save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: document(state),
                body_markdown: "Preserve these notes and trial evidence".into(),
                expected_revision: 0,
            })
            .await
            .unwrap();
        assert_eq!(
            app.submit_feedback(SubmitFeedbackInput {
                request_id: created.request_id.clone(),
                expected_revision: saved.saved_revision,
                cooked_markdown: None,
                cooking_model: None,
                uncooked_markdown: None
            })
            .await
            .unwrap_err()
            .code(),
            "INVALID_ARGUMENT"
        );
        assert!(
            app.get_feedback_workspace(created.request_id)
                .await
                .unwrap()
                .feedback
                .is_none()
        );
    }
    store.close().await;
}
