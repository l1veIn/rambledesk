use super::*;
use base64::Engine;
use rambledesk_core::{RequestAttachmentInput, WorkbenchResult};

fn table_spec() -> WorkbenchSpec {
    serde_json::from_value(json!({"type":"table_review","version":1,"data":{
        "title":"Budget","source_version":"v1","columns":[{"id":"amount","label":"Amount"}],
        "rows":[{"id":"hosting","cells":["120"]}]
    }}))
    .unwrap()
}
fn media_spec() -> WorkbenchSpec {
    serde_json::from_value(json!({"type":"media_review","version":1,"data":{
        "title":"Audio","source_version":"v1","media_kind":"audio","media_file_name":"clip.wav","duration_ms":1000
    }})).unwrap()
}
fn wave() -> Vec<u8> {
    // Real one-second 8kHz mono PCM16 WAV, not a signature-only fixture.
    let samples: u32 = 8000;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + samples * 2).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(samples.to_le_bytes());
    bytes.extend((samples * 2).to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((samples * 2).to_le_bytes());
    bytes.resize(44 + samples as usize * 2, 0);
    bytes
}
fn attachment(bytes: &[u8]) -> RequestAttachmentInput {
    RequestAttachmentInput {
        file_name: "clip.wav".into(),
        markdown: None,
        contents_base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
        path: None,
    }
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
fn table_state(body: &str, value: &str) -> Value {
    json!({"type":"table_review","changes":[{"row_id":"hosting","column_id":"amount","value":value}],"comments":[{"id":"c-1","row_id":"hosting","column_id":"amount","body":body}]})
}
fn media_state(body: &str) -> Value {
    json!({"type":"media_review","comments":[{"id":"c-1","start_ms":100,"end_ms":800,"body":body}]})
}

#[tokio::test]
async fn table_edits_and_attached_cell_comments_save_reopen_and_publish_without_changing_original_values()
 {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(table_spec());
    let created = app.request_feedback(input).await.unwrap();
    let uploaded = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: created.request_id.clone(),
            file_name: "evidence.txt".into(),
            contents: b"Estimate details".to_vec(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    let reference = format!(
        "[evidence.txt](attachment://{})",
        uploaded.attachments[0].attachment_id
    );
    let draft = envelope(table_state(&reference, "100"));
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: draft.clone(),
            body_markdown: String::new(),
            expected_revision: uploaded.draft.saved_revision,
        })
        .await
        .unwrap();
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
        Some(draft.as_str())
    );
    let published = app
        .submit_feedback(submission(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&published)
        .await
        .unwrap()
        .unwrap();
    let workbench = package.manifest.workbench.unwrap();
    assert_eq!(
        serde_json::to_value(&workbench.input).unwrap()["data"]["rows"][0]["cells"][0],
        "120"
    );
    let Some(WorkbenchResult::TableReview(result)) = workbench.result else {
        panic!("table result");
    };
    assert_eq!(result.source_version, "v1");
    assert_eq!(result.changes[0].row_id, "hosting");
    assert_eq!(result.changes[0].column_id, "amount");
    assert_eq!(result.changes[0].value, "100");
    assert_eq!(result.comments[0].body, reference);
    assert_eq!(
        package.manifest.attachments[0].id,
        uploaded.attachments[0].attachment_id
    );
    assert_eq!(
        tokio::fs::read(&package.attachment_paths[0]).await.unwrap(),
        b"Estimate details"
    );
    store.close().await;
}

#[tokio::test]
async fn table_invalid_locations_and_duplicates_are_rejected_but_empty_editing_drafts_survive() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(table_spec());
    let created = app.request_feedback(input).await.unwrap();
    for invalid in [
        json!({"type":"table_review","changes":[{"row_id":"unknown","column_id":"amount","value":"100"}],"comments":[]}),
        json!({"type":"table_review","changes":[{"row_id":"hosting","column_id":"amount","value":"100"},{"row_id":"hosting","column_id":"amount","value":"90"}],"comments":[]}),
        json!({"type":"table_review","changes":[],"comments":[{"id":"c","row_id":"hosting","column_id":"missing","body":"text"}]}),
    ] {
        assert!(
            app.save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: envelope(invalid),
                body_markdown: String::new(),
                expected_revision: 0
            })
            .await
            .is_err()
        );
    }
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(table_state("", "120")),
            body_markdown: "Overall notes cannot conceal an invalid comment".into(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert!(
        app.submit_feedback(submission(&created.request_id, saved.saved_revision))
            .await
            .is_err()
    );
    let restored = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert!(restored.draft.document_json.unwrap().contains("hosting"));
    store.close().await;
}

#[tokio::test]
async fn media_path_bytes_are_frozen_scope_checked_reopened_and_published_with_timed_attachment_comments()
 {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let original = wave();
    let source = workspace.database.parent().unwrap().join("clip.wav");
    tokio::fs::write(&source, &original).await.unwrap();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(media_spec());
    input.attachments = vec![RequestAttachmentInput {
        file_name: "clip.wav".into(),
        markdown: None,
        contents_base64: None,
        path: Some(source.to_string_lossy().into()),
    }];
    let created = app.request_feedback(input).await.unwrap();
    let loaded = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    let material = loaded.request_attachments[0].clone();
    assert_eq!(material.media_type, "audio/wav");
    assert_eq!(material.byte_size, original.len() as u64);
    assert_eq!(material.sha256, hex::encode(Sha256::digest(&original)));
    tokio::fs::write(&source, b"replaced original filesystem source")
        .await
        .unwrap();
    assert_eq!(
        app.read_request_attachment(created.request_id.clone(), material.attachment_id.clone())
            .await
            .unwrap(),
        original
    );
    let other = app
        .request_feedback(workspace.request(Uuid::now_v7().to_string()))
        .await
        .unwrap();
    assert!(
        app.read_request_attachment(other.request_id, material.attachment_id.clone())
            .await
            .is_err()
    );
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(media_state("")),
            body_markdown: String::new(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert!(
        app.submit_feedback(submission(&created.request_id, saved.saved_revision))
            .await
            .is_err()
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
    assert!(reopened.draft.document_json.unwrap().contains("c-1"));
    let upload = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: created.request_id.clone(),
            file_name: "reference.md".into(),
            contents: b"# Relevant detail".to_vec(),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap();
    let reference = format!(
        "[reference.md](attachment://{})",
        upload.attachments[0].attachment_id
    );
    let draft = envelope(media_state(&reference));
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: draft.clone(),
            body_markdown: String::new(),
            expected_revision: upload.draft.saved_revision,
        })
        .await
        .unwrap();
    let published = app
        .submit_feedback(submission(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&published)
        .await
        .unwrap()
        .unwrap();
    let Some(WorkbenchResult::MediaReview(result)) = package.manifest.workbench.unwrap().result
    else {
        panic!("media result");
    };
    assert_eq!(result.source_version, "v1");
    assert_eq!(result.duration_ms, 1000);
    assert_eq!(result.comments[0].start_ms, 100);
    assert_eq!(result.comments[0].end_ms, Some(800));
    assert_eq!(result.comments[0].body, reference);
    assert_eq!(
        tokio::fs::read(&package.request_attachment_paths[0])
            .await
            .unwrap(),
        original
    );
    assert_eq!(
        package.manifest.request_attachments[0].sha256,
        material.sha256
    );
    assert_eq!(
        app.read_request_attachment(created.request_id.clone(), material.attachment_id)
            .await
            .unwrap(),
        original
    );
    assert_eq!(
        app.get_feedback_workspace(created.request_id)
            .await
            .unwrap()
            .draft
            .document_json
            .as_deref(),
        Some(draft.as_str())
    );
    store.close().await;
}

#[tokio::test]
async fn media_attachment_mismatch_duplicate_and_invalid_timestamps_never_become_publishable() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    for attachments in [
        vec![],
        vec![attachment(&wave()), attachment(&wave())],
        vec![attachment(b"not audio")],
    ] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(media_spec());
        input.attachments = attachments;
        assert!(app.request_feedback(input).await.is_err());
    }
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(media_spec());
    input.attachments = vec![attachment(&wave())];
    let created = app.request_feedback(input).await.unwrap();
    for state in [
        json!({"type":"media_review","comments":[{"id":"c","start_ms":1001,"end_ms":null,"body":"Out of bounds"}]}),
        json!({"type":"media_review","comments":[{"id":"c","start_ms":800,"end_ms":100,"body":"Reversed"}]}),
        json!({"type":"media_review","comments":[{"id":"c","start_ms":0,"end_ms":null,"body":"a"},{"id":"c","start_ms":100,"end_ms":null,"body":"b"}]}),
    ] {
        assert!(
            app.save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: envelope(state),
                body_markdown: String::new(),
                expected_revision: 0
            })
            .await
            .is_err()
        );
    }
    store.close().await;
}

#[tokio::test]
async fn tampered_frozen_media_is_rejected_by_playback_read_and_publication_then_can_retry_original_bytes()
 {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let original = wave();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(media_spec());
    input.attachments = vec![attachment(&original)];
    let created = app.request_feedback(input).await.unwrap();
    let loaded = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    let id = loaded.request_attachments[0].attachment_id.clone();
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: envelope(media_state("Review this sound")),
            body_markdown: String::new(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    let path: String = sqlx::query_scalar(
        "SELECT draft_path FROM request_attachments WHERE request_id=?1 AND id=?2",
    )
    .bind(&created.request_id)
    .bind(&id)
    .fetch_one(&store.pool)
    .await
    .unwrap();
    let mut changed = original.clone();
    changed[44] = 1;
    tokio::fs::write(&path, changed).await.unwrap();
    assert!(
        app.read_request_attachment(created.request_id.clone(), id.clone())
            .await
            .is_err()
    );
    assert!(
        app.submit_feedback(submission(&created.request_id, saved.saved_revision))
            .await
            .is_err()
    );
    // A growing/tampered file must not bypass the bounded read via frozen metadata.
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .await
        .unwrap();
    file.set_len(rambledesk_core::MAX_ATTACHMENT_BYTES as u64 + 1)
        .await
        .unwrap();
    drop(file);
    assert!(
        app.read_request_attachment(created.request_id.clone(), id.clone())
            .await
            .is_err()
    );
    tokio::fs::write(&path, &original).await.unwrap();
    let published = app
        .submit_feedback(submission(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    assert_eq!(
        app.read_request_attachment(created.request_id, id)
            .await
            .unwrap(),
        original
    );
    assert!(
        app.read_feedback_package(&published)
            .await
            .unwrap()
            .is_some()
    );
    store.close().await;
}

#[tokio::test]
async fn table_and_media_notes_only_submit_without_initialized_state_and_cancellation_has_no_result()
 {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    for spec in [table_spec(), media_spec()] {
        for cancel in [false, true] {
            let mut input = workspace.request(Uuid::now_v7().to_string());
            input.actions.clear();
            input.workbench = Some(spec.clone());
            if spec.kind == "media_review" {
                input.attachments = vec![attachment(&wave())];
            }
            let created = app.request_feedback(input).await.unwrap();
            let fresh = app
                .get_feedback_workspace(created.request_id.clone())
                .await
                .unwrap();
            assert_eq!(fresh.draft.document_json, None);
            assert_eq!(fresh.draft.saved_revision, 0);
            // A bare editor document is not the host's v2 draft envelope.
            assert!(
                app.save_feedback_draft(SaveDraftInput {
                    request_id: created.request_id.clone(),
                    document_json: r#"{"type":"doc","content":[]}"#.into(),
                    body_markdown: "Overall review only".into(),
                    expected_revision: 0,
                })
                .await
                .is_err()
            );
            let saved = app
                .save_feedback_draft(SaveDraftInput {
                    request_id: created.request_id.clone(),
                    document_json: r#"{"schemaVersion":2,"doc":{"type":"doc","content":[]}}"#
                        .into(),
                    body_markdown: "Overall review only".into(),
                    expected_revision: 0,
                })
                .await
                .unwrap();
            let completed = if cancel {
                app.cancel_feedback(CancelFeedbackInput {
                    request_id: created.request_id,
                    reason: "No further review".into(),
                })
                .await
                .unwrap()
            } else {
                app.submit_feedback(submission(&created.request_id, saved.saved_revision))
                    .await
                    .unwrap()
            };
            let package = app
                .read_feedback_package(&completed)
                .await
                .unwrap()
                .unwrap();
            let result = package.manifest.workbench.unwrap().result;
            assert_eq!(result.is_none(), cancel);
            if let Some(result) = result {
                let result = serde_json::to_value(result).unwrap();
                assert_eq!(result["source_version"], "v1");
                assert_eq!(result["comments"], json!([]));
            }
        }
    }
    store.close().await;
}
