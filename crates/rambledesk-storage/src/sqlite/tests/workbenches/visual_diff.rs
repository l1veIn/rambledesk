use super::*;
use rambledesk_core::{RequestAttachmentInput, WorkbenchPackage, WorkbenchResult};

fn visual_spec(name: Option<&str>) -> WorkbenchSpec {
    serde_json::from_value(json!({"type":"visual_feedback","version":1,"data":{
        "title":"Review the canvas","source_version":"v1","width":32,"height":32,"image_file_name":name
    }})).unwrap()
}
fn diff_spec() -> WorkbenchSpec {
    serde_json::from_value(json!({"type":"diff_review","version":1,"data":{
        "title":"Review the change","source_version":"commit-a","files":[{
            "id":"file","old_path":"src/file.ts","new_path":"src/file.ts",
            "diff":"--- a/src/file.ts\n+++ b/src/file.ts\n@@ -1,2 +1,2 @@\n context\n-before\n+after\n"
        }]
    }})).unwrap()
}
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(vec![]);
    image::DynamicImage::new_rgb8(width, height)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}
fn visual_state(id: Option<&str>) -> Value {
    json!({"type":"visual_feedback","composite_attachment_id":id,"annotations":[{
        "id":"a-1","kind":"arrow","points":[{"x":2,"y":3},{"x":20,"y":25}],"color":"#ff0000","stroke_width":3,"text":"","body":"Move the button"
    }]})
}
fn diff_state(body: &str) -> Value {
    json!({"type":"diff_review","comments":[{
        "id":"c-1","anchor":{"file_id":"file","hunk_index":0,"side":"new","start_line":2,"end_line":2},"body":body
    }]})
}
fn envelope(state: Value) -> String {
    json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state}).to_string()
}
fn submission(id: &str, revision: u64) -> SubmitFeedbackInput {
    SubmitFeedbackInput {
        request_id: id.into(),
        expected_revision: revision,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    }
}

#[tokio::test]
async fn visual_material_is_frozen_and_drawing_reopens_then_publishes_image_and_structured_history()
{
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let source = workspace.database.parent().unwrap().join("source.png");
    let original = png(32, 32);
    tokio::fs::write(&source, &original).await.unwrap();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(visual_spec(Some("source.png")));
    input.attachments = vec![RequestAttachmentInput {
        file_name: "source.png".into(),
        markdown: None,
        contents_base64: None,
        path: Some(source.to_string_lossy().into()),
    }];
    let created = app.request_feedback(input).await.unwrap();
    let loaded = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    tokio::fs::write(&source, b"changed source on disk")
        .await
        .unwrap();
    assert_eq!(
        app.read_request_attachment(
            created.request_id.clone(),
            loaded.request_attachments[0].attachment_id.clone()
        )
        .await
        .unwrap(),
        original
    );
    let drawing = envelope(visual_state(None));
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: drawing.clone(),
            body_markdown: String::new(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert_eq!(
        app.submit_feedback(submission(&created.request_id, saved.saved_revision))
            .await
            .unwrap_err()
            .code(),
        "INVALID_ARGUMENT"
    );
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let reopened = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(
        reopened.draft.document_json.as_deref(),
        Some(drawing.as_str())
    );
    let uploaded = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: created.request_id.clone(),
            file_name: "canvas.png".into(),
            contents: original.clone(),
            expected_revision: reopened.draft.saved_revision,
        })
        .await
        .unwrap();
    let id = uploaded.attachments[0].attachment_id.clone();
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(visual_state(Some(&id))),
            body_markdown: String::new(),
            expected_revision: uploaded.draft.saved_revision,
        })
        .await
        .unwrap();
    let completed = app
        .submit_feedback(submission(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&completed)
        .await
        .unwrap()
        .unwrap();
    let Some(WorkbenchPackage {
        result: Some(WorkbenchResult::VisualFeedback(result)),
        ..
    }) = package.manifest.workbench
    else {
        panic!("visual result")
    };
    assert_eq!(result.source_version, "v1");
    assert_eq!(result.composite_attachment_id, id);
    assert_eq!(result.annotations[0].body, "Move the button");
    assert!(package.markdown.trim().is_empty());
    assert_eq!(
        tokio::fs::read(&package.attachment_paths[0]).await.unwrap(),
        original
    );
    assert_eq!(package.manifest.attachments[0].id, id);
    assert_eq!(package.manifest.attachments[0].media_type, "image/png");
    assert_eq!(
        tokio::fs::read(&package.request_attachment_paths[0])
            .await
            .unwrap(),
        original
    );
    store.close().await;
}

#[tokio::test]
async fn visual_saved_and_published_composites_reject_foreign_wrong_dimensions_corruption_and_removal()
 {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(visual_spec(None));
    let created = app.request_feedback(input).await.unwrap();
    let mut revision = 0;
    for contents in [
        png(64, 32),
        b"\x89PNG\r\n\x1a\nopaque".to_vec(),
        b"text".to_vec(),
    ] {
        let uploaded = app
            .add_feedback_attachment(AddAttachmentInput {
                request_id: created.request_id.clone(),
                file_name: "invalid.png".into(),
                contents,
                expected_revision: revision,
            })
            .await
            .unwrap();
        revision = uploaded.draft.saved_revision;
        let id = uploaded.attachments.last().unwrap().attachment_id.clone();
        assert_eq!(
            app.save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: envelope(visual_state(Some(&id))),
                body_markdown: String::new(),
                expected_revision: revision
            })
            .await
            .unwrap_err()
            .code(),
            "INVALID_ARGUMENT"
        );
    }
    let mut foreign = workspace.request(Uuid::now_v7().to_string());
    foreign.actions.clear();
    foreign.workbench = Some(visual_spec(None));
    let foreign = app.request_feedback(foreign).await.unwrap();
    let other = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: foreign.request_id,
            file_name: "other.png".into(),
            contents: png(32, 32),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert_eq!(
        app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(visual_state(Some(&other.attachments[0].attachment_id))),
            body_markdown: String::new(),
            expected_revision: revision
        })
        .await
        .unwrap_err()
        .code(),
        "INVALID_ARGUMENT"
    );
    let uploaded = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: created.request_id.clone(),
            file_name: "canvas.png".into(),
            contents: png(32, 32),
            expected_revision: revision,
        })
        .await
        .unwrap();
    let id = uploaded.attachments.last().unwrap().attachment_id.clone();
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(visual_state(Some(&id))),
            body_markdown: String::new(),
            expected_revision: uploaded.draft.saved_revision,
        })
        .await
        .unwrap();
    let path: String = sqlx::query_scalar("SELECT draft_path FROM attachments WHERE id=?1")
        .bind(&id)
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let bytes = tokio::fs::read(&path).await.unwrap();
    tokio::fs::write(&path, vec![0u8; bytes.len()])
        .await
        .unwrap();
    assert_eq!(
        app.submit_feedback(submission(&created.request_id, saved.saved_revision))
            .await
            .unwrap_err()
            .code(),
        "INVALID_ARGUMENT"
    );
    tokio::fs::write(&path, bytes).await.unwrap();
    let removed = app
        .remove_feedback_attachment(RemoveAttachmentInput {
            request_id: created.request_id.clone(),
            attachment_id: id.clone(),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap();
    assert_eq!(
        app.submit_feedback(submission(
            &created.request_id,
            removed.draft.saved_revision
        ))
        .await
        .unwrap_err()
        .code(),
        "INVALID_ARGUMENT"
    );
    // An identical stale save must not bypass validation after the composite was removed.
    assert_eq!(
        app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(visual_state(Some(&id))),
            body_markdown: String::new(),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap_err()
        .code(),
        "INVALID_ARGUMENT"
    );
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(visual_state(None)),
            body_markdown: String::new(),
            expected_revision: removed.draft.saved_revision,
        })
        .await
        .unwrap();
    let uploaded = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: created.request_id.clone(),
            file_name: "replacement.png".into(),
            contents: png(32, 32),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap();
    let id = uploaded.attachments.last().unwrap().attachment_id.clone();
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(visual_state(Some(&id))),
            body_markdown: String::new(),
            expected_revision: uploaded.draft.saved_revision,
        })
        .await
        .unwrap();
    app.submit_feedback(submission(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    store.close().await;
}

#[tokio::test]
async fn visual_notes_only_still_require_the_current_canvas_png() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(visual_spec(None));
    let created = app.request_feedback(input).await.unwrap();
    let state = json!({"type":"visual_feedback","annotations":[],"composite_attachment_id":null});
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(state.clone()),
            body_markdown: "Overall visual feedback".into(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert_eq!(
        app.submit_feedback(submission(&created.request_id, saved.saved_revision))
            .await
            .unwrap_err()
            .code(),
        "INVALID_ARGUMENT"
    );
    let uploaded = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: created.request_id.clone(),
            file_name: "notes-canvas.png".into(),
            contents: png(32, 32),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap();
    let id = uploaded.attachments[0].attachment_id.clone();
    let mut state = state;
    state["composite_attachment_id"] = json!(id);
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(state.clone()),
            body_markdown: "Overall visual feedback".into(),
            expected_revision: uploaded.draft.saved_revision,
        })
        .await
        .unwrap();
    // The attachment menu first unlinks the composite and saves, then deletes its bytes.
    state["composite_attachment_id"] = Value::Null;
    let unlinked = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(state.clone()),
            body_markdown: "Overall visual feedback".into(),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap();
    let removed = app
        .remove_feedback_attachment(RemoveAttachmentInput {
            request_id: created.request_id.clone(),
            attachment_id: id.clone(),
            expected_revision: unlinked.saved_revision,
        })
        .await
        .unwrap();
    let removed_document: Value =
        serde_json::from_str(removed.draft.document_json.as_deref().unwrap()).unwrap();
    assert!(removed_document["workbenchState"]["composite_attachment_id"].is_null());
    // Generated persistence performs one more real flush before uploading a replacement.
    let flushed = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(state.clone()),
            body_markdown: "Overall visual feedback".into(),
            expected_revision: removed.draft.saved_revision,
        })
        .await
        .unwrap();
    let uploaded = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: created.request_id.clone(),
            file_name: "regenerated-notes-canvas.png".into(),
            contents: png(32, 32),
            expected_revision: flushed.saved_revision,
        })
        .await
        .unwrap();
    let replacement_id = uploaded.attachments[0].attachment_id.clone();
    assert_ne!(replacement_id, id);
    state["composite_attachment_id"] = json!(replacement_id);
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(state),
            body_markdown: "Overall visual feedback".into(),
            expected_revision: uploaded.draft.saved_revision,
        })
        .await
        .unwrap();
    let completed = app
        .submit_feedback(submission(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&completed)
        .await
        .unwrap()
        .unwrap();
    let Some(WorkbenchPackage {
        result: Some(WorkbenchResult::VisualFeedback(result)),
        ..
    }) = package.manifest.workbench
    else {
        panic!("visual result")
    };
    assert!(result.annotations.is_empty());
    assert_eq!(result.composite_attachment_id, replacement_id);
    assert_eq!(package.markdown.trim(), "Overall visual feedback");
    store.close().await;
}

#[tokio::test]
async fn diff_comments_reopen_and_publish_exact_anchors_notes_only_and_invalid_drafts_are_checked()
{
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    for notes_only in [false, true] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(diff_spec());
        let created = app.request_feedback(input).await.unwrap();
        let mut invalid = diff_state("Comment");
        invalid["comments"][0]["anchor"]["start_line"] = json!(3);
        invalid["comments"][0]["anchor"]["end_line"] = json!(3);
        assert_eq!(
            app.save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: envelope(invalid),
                body_markdown: String::new(),
                expected_revision: 0
            })
            .await
            .unwrap_err()
            .code(),
            "INVALID_ARGUMENT"
        );
        let unfinished = envelope(diff_state(""));
        let saved = app
            .save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: unfinished,
                body_markdown: "Notes cannot hide an empty comment".into(),
                expected_revision: 0,
            })
            .await
            .unwrap();
        assert_eq!(
            app.submit_feedback(submission(&created.request_id, saved.saved_revision))
                .await
                .unwrap_err()
                .code(),
            "INVALID_ARGUMENT"
        );
        let state = if notes_only {
            json!({"type":"diff_review","comments":[]})
        } else {
            diff_state("Explain this change")
        };
        let document = envelope(state);
        let saved = app
            .save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: document.clone(),
                body_markdown: if notes_only {
                    "Overall feedback".into()
                } else {
                    String::new()
                },
                expected_revision: saved.saved_revision,
            })
            .await
            .unwrap();
        let reopened = app
            .get_feedback_workspace(created.request_id.clone())
            .await
            .unwrap();
        assert_eq!(
            reopened.draft.document_json.as_deref(),
            Some(document.as_str())
        );
        let completed = app
            .submit_feedback(submission(&created.request_id, saved.saved_revision))
            .await
            .unwrap();
        let package = app
            .read_feedback_package(&completed)
            .await
            .unwrap()
            .unwrap();
        let Some(WorkbenchPackage {
            result: Some(WorkbenchResult::DiffReview(result)),
            ..
        }) = package.manifest.workbench
        else {
            panic!("diff result")
        };
        assert_eq!(result.source_version, "commit-a");
        assert_eq!(result.comments.len(), usize::from(!notes_only));
        if !notes_only {
            assert_eq!(result.comments[0].anchor.start_line, Some(2));
            assert!(package.markdown.trim().is_empty());
        }
    }
    store.close().await;
}
