//! Publication of files referenced from structured workbench fields.

use super::*;

#[tokio::test]
async fn structured_field_attachment_references_publish_with_resolvable_manifest_files() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    for kind in ["questions", "document_review"] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(serde_json::from_value(if kind == "questions" {
            json!({"type":"questions","version":1,"data":{"questions":[{
                "id":"q","prompt":"What should change?","allowOther":true,
                "options":[{"value":"a","label":"A"},{"value":"b","label":"B"}]
            }]}})
        } else {
            json!({"type":"document_review","version":1,"data":{
                "title":"Script","source_version":"v1","paragraphs":[{"id":"p","text":"Original manuscript"}]
            }})
        }).unwrap());
        let created = app.request_feedback(input).await.unwrap();
        let bytes = b"Reference contents".to_vec();
        let uploaded = app
            .add_feedback_attachment(AddAttachmentInput {
                request_id: created.request_id.clone(),
                file_name: "reference.txt".into(),
                contents: bytes.clone(),
                expected_revision: 0,
            })
            .await
            .unwrap();
        let id = &uploaded.attachments[0].attachment_id;
        let reference = format!("[reference.txt](attachment://{id})");
        let state = if kind == "questions" {
            json!({"type":"questions","answers":[{"id":"q","value":reference,"label":reference,"wasCustom":true}]})
        } else {
            json!({"type":"document_review","verdict":"changes_requested","paragraph_marks":[],"annotations":[{
                "id":"a","paragraph_id":"p","kind":"comment","body":reference,"replacement":null,
                "start":null,"end":null,"quote":null
            }]})
        };
        let saved = app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state}).to_string(),
            body_markdown: String::new(), expected_revision: uploaded.draft.saved_revision,
        }).await.unwrap();
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
        let manifest = serde_json::to_value(&package.manifest).unwrap();
        let field = if kind == "questions" {
            &manifest["workbench"]["result"]["answers"][0]["value"]
        } else {
            &manifest["workbench"]["result"]["annotations"][0]["body"]
        };
        assert_eq!(field, &json!(reference));
        assert_eq!(manifest["attachments"][0]["id"], json!(id));
        assert!(
            manifest["attachments"][0]["path"]
                .as_str()
                .unwrap()
                .starts_with("attachments/")
        );
        assert_eq!(package.attachment_paths.len(), 1);
        assert_eq!(
            tokio::fs::read(&package.attachment_paths[0]).await.unwrap(),
            bytes
        );
        assert!(package.markdown.trim().is_empty());
    }
    store.close().await;
}
