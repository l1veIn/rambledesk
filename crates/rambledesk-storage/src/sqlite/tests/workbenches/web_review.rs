use super::*;

fn annotation() -> Value {
    json!({
        "id":"note-1","page_url":"http://localhost:5173/settings?tab=profile",
        "viewport":{"width":390,"height":844},
        "element":{"selector":"#save","tag_name":"button","text":"保存😀",
            "rect":{"x":20,"y":1300,"width":200,"height":48}},
        "body":"Keep this action visible"
    })
}

fn document(annotations: Vec<Value>) -> String {
    json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},
        "workbenchState":{"type":"web_review","annotations":annotations}})
    .to_string()
}

#[tokio::test]
async fn web_review_roundtrips_restarts_and_atomically_publishes_notes_with_anchors() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(original_workbench_example("web_review"));
    let mut approval = input.clone();
    approval.allow_finish = true;
    approval.final_summary = Some("Execute changes".into());
    assert_eq!(
        app.request_feedback(approval).await.unwrap_err().code(),
        "INVALID_ARGUMENT"
    );
    let created = app.request_feedback(input.clone()).await.unwrap();
    assert_eq!(
        app.request_feedback(input.clone())
            .await
            .unwrap()
            .request_id,
        created.request_id
    );
    let mut changed = input.clone();
    let WorkbenchData::WebReview(data) = &mut changed.workbench.as_mut().unwrap().data else {
        unreachable!()
    };
    data.source_version = "draft-2".into();
    assert_eq!(
        app.request_feedback(changed).await.unwrap_err().code(),
        "REQUEST_CONFLICT"
    );
    let doc = document(vec![annotation()]);
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: doc.clone(),
            body_markdown: "General feedback from the same revision".into(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert_eq!(
        app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: document(vec![]),
            body_markdown: "Stale notes".into(),
            expected_revision: 0,
        })
        .await
        .unwrap_err()
        .code(),
        "DRAFT_CONFLICT"
    );
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let loaded = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(loaded.workbench, input.workbench);
    assert!(loaded.actions.is_empty());
    assert_eq!(loaded.draft.document_json.as_deref(), Some(doc.as_str()));
    assert_eq!(
        loaded.draft.body_markdown,
        "General feedback from the same revision"
    );
    assert_eq!(
        app.submit_feedback(SubmitFeedbackInput {
            request_id: created.request_id.clone(),
            expected_revision: 0,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .unwrap_err()
        .code(),
        "DRAFT_CONFLICT"
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
    assert_eq!(
        package.markdown.trim(),
        "General feedback from the same revision"
    );
    let workbench = serde_json::to_value(package.manifest.workbench.unwrap()).unwrap();
    assert_eq!(workbench["result"]["source_version"], "draft-1");
    assert_eq!(workbench["result"]["annotations"][0], annotation());
    assert!(workbench["result"].get("verdict").is_none());
    let terminal = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(terminal.draft.document_json.as_deref(), Some(doc.as_str()));
    store.close().await;
}

#[tokio::test]
async fn web_review_supports_notes_only_and_cancellation_preserves_recoverable_draft() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    for cancelled in [false, true] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(original_workbench_example("web_review"));
        let host_id = input.host_id.clone();
        let host_session_id = input.host_session_id.clone();
        let created = app.request_feedback(input).await.unwrap();
        let doc = document(if cancelled {
            vec![annotation()]
        } else {
            vec![]
        });
        let saved = app
            .save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: doc.clone(),
                body_markdown: "General notes".into(),
                expected_revision: 0,
            })
            .await
            .unwrap();
        let result = if cancelled {
            app.cancel_feedback(CancelFeedbackInput {
                request_id: created.request_id.clone(),
                reason: "Review later".into(),
            })
            .await
            .unwrap()
        } else {
            app.submit_feedback(SubmitFeedbackInput {
                request_id: created.request_id.clone(),
                expected_revision: saved.saved_revision,
                cooked_markdown: None,
                cooking_model: None,
                uncooked_markdown: None,
            })
            .await
            .unwrap()
        };
        let package = app.read_feedback_package(&result).await.unwrap().unwrap();
        let workbench = package.manifest.workbench.unwrap();
        if cancelled {
            assert!(workbench.result.is_none());
            let recovered = app
                .recover_feedback(RecoverFeedbackInput {
                    request_id: Some(created.request_id.clone()),
                    host_id,
                    host_session_id,
                })
                .await
                .unwrap();
            assert_eq!(recovered.status, FeedbackStatus::Cancelled);
            let loaded = app
                .get_feedback_workspace(created.request_id.clone())
                .await
                .unwrap();
            assert_eq!(loaded.draft.document_json.as_deref(), Some(doc.as_str()));
        } else {
            assert_eq!(package.markdown.trim(), "General notes");
            assert_eq!(
                serde_json::to_value(workbench.result.unwrap()).unwrap()["annotations"],
                json!([])
            );
        }
    }
    store.close().await;
}
