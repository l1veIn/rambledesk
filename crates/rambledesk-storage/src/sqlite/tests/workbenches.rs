use super::*;
use rambledesk_core::{
    DescribeWorkbenchInput, ListWorkbenchesInput, WorkbenchData, WorkbenchSpec, describe_workbench,
    list_workbenches,
};
use serde_json::{Value, json};

mod attachments;
#[cfg(feature = "workbench-fixtures")]
mod rating_review;
mod terminal;
mod web_review;

// Compatibility fixtures must not rely on the catalog for new requests.
fn original_workbench_example(kind: &str) -> WorkbenchSpec {
    if kind == "single_choice" {
        return serde_json::from_value(json!({
            "type":"single_choice", "version":1,
            "data":{"prompt":"Which layout should we use?", "options":[
                {"id":"compact","label":"Compact layout"},
                {"id":"spacious","label":"Spacious layout"}
            ]}
        }))
        .unwrap();
    }
    let mut example = describe_workbench(&DescribeWorkbenchInput {
        kind: kind.into(),
        version: None,
    })
    .unwrap()
    .example;
    if let WorkbenchData::Questions(data) = &mut example.data {
        data.questions[0].allow_other = true;
        data.questions.push(
            serde_json::from_value(json!({
                "id":"scope", "prompt":"What should we build first?", "label":"Scope",
                "allowOther":true, "options":[
                    {"value":"feedback","label":"Feedback"},
                    {"value":"review","label":"Review"}
                ]
            }))
            .unwrap(),
        );
    }
    example
}

#[test]
fn discovery_is_paged_and_request_schema_does_not_embed_each_type() {
    let catalog = list_workbenches(&ListWorkbenchesInput::default()).unwrap();
    assert_eq!(
        catalog
            .workbenches
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>(),
        [
            "ramble",
            "questions",
            "document_review",
            "web_review",
            "terminal"
        ]
    );
    assert_eq!(catalog.next_offset, Some(5));
    let page = list_workbenches(&ListWorkbenchesInput {
        limit: Some(1),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(page.workbenches.len(), 1);
    assert_eq!(page.next_offset, Some(1));
    assert!(
        !serde_json::to_string(&page)
            .unwrap()
            .contains("input_schema")
    );
    let page = list_workbenches(&ListWorkbenchesInput {
        offset: 2,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(page.workbenches[0].kind, "document_review");
    assert_eq!(page.next_offset, None);
    assert!(
        list_workbenches(&ListWorkbenchesInput {
            limit: Some(21),
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        describe_workbench(&DescribeWorkbenchInput {
            kind: "questions".into(),
            version: Some(9)
        })
        .is_err()
    );
    let schema = serde_json::to_string(&schemars::schema_for!(RequestFeedbackInput)).unwrap();
    assert!(!schema.contains("QuestionsData"));
    assert!(!schema.contains("SingleChoiceData"));
    let legacy_description = describe_workbench(&DescribeWorkbenchInput {
        kind: "single_choice".into(),
        version: Some(1),
    })
    .unwrap_err();
    assert_eq!(legacy_description.code(), "INVALID_ARGUMENT");
    assert!(
        legacy_description
            .message()
            .contains("one question with allowOther:false")
    );
    for kind in [
        "ramble",
        "questions",
        "document_review",
        "web_review",
        "terminal",
        "sort",
    ] {
        let description = describe_workbench(&DescribeWorkbenchInput {
            kind: kind.into(),
            version: None,
        })
        .unwrap();
        let roundtrip: WorkbenchSpec =
            serde_json::from_value(serde_json::to_value(&description.example).unwrap()).unwrap();
        assert_eq!(roundtrip, description.example);
        if let WorkbenchData::Questions(data) = &roundtrip.data {
            assert_eq!(data.questions.len(), 1);
            assert!(!data.questions[0].allow_other);
        }
        assert!(rambledesk_core::validate_workbench(&roundtrip).is_ok());
        assert_eq!(
            rambledesk_core::workbench_actions(&roundtrip)
                .unwrap()
                .is_empty(),
            matches!(kind, "document_review" | "web_review" | "terminal" | "sort")
        );
    }
}

#[tokio::test]
async fn single_question_discovery_example_publishes_a_strict_choice_as_questions() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(
        describe_workbench(&DescribeWorkbenchInput {
            kind: "questions".into(),
            version: None,
        })
        .unwrap()
        .example,
    );
    let created = app.request_feedback(input).await.unwrap();
    let mut revision = 0;
    for custom in [true, false] {
        let document = json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},
            "workbenchState":{"type":"questions","answers":[{
                "id":"audience","value":"individuals","label":"Human-supplied label",
                "wasCustom":custom
            }]}
        });
        let saved = app
            .save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: document.to_string(),
                body_markdown: String::new(),
                expected_revision: revision,
            })
            .await
            .unwrap();
        revision = saved.saved_revision;
        let result = app
            .submit_feedback(SubmitFeedbackInput {
                request_id: created.request_id.clone(),
                expected_revision: revision,
                cooked_markdown: None,
                cooking_model: None,
                uncooked_markdown: None,
            })
            .await;
        if custom {
            assert_eq!(result.unwrap_err().code(), "INVALID_ARGUMENT");
        } else {
            let package = app
                .read_feedback_package(&result.unwrap())
                .await
                .unwrap()
                .unwrap();
            let workbench = serde_json::to_value(package.manifest.workbench.unwrap()).unwrap();
            assert_eq!(workbench["type"], "questions");
            assert_eq!(
                workbench["result"]["answers"],
                json!([{
                    "id":"audience","value":"individuals","label":"Individuals",
                    "wasCustom":false,"index":1
                }])
            );
            assert_eq!(workbench["result"]["cancelled"], false);
            assert!(workbench["result"].get("selected_option_id").is_none());
        }
    }
    store.close().await;
}

#[tokio::test]
async fn typed_requests_preserve_identity_and_publish_answers_without_notes() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    for kind in ["ramble", "questions", "single_choice"] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(original_workbench_example(kind));
        let created = app.request_feedback(input.clone()).await.unwrap();
        assert_eq!(
            app.request_feedback(input.clone())
                .await
                .unwrap()
                .request_id,
            created.request_id
        );
        let loaded = app
            .get_feedback_workspace(created.request_id.clone())
            .await
            .unwrap();
        assert_eq!(loaded.workbench, input.workbench);
        let mut changed = input.clone();
        match &mut changed.workbench.as_mut().unwrap().data {
            WorkbenchData::Ramble(data) => data.actions[0].instruction.push('!'),
            WorkbenchData::Questions(data) => data.questions[0].prompt.push('!'),
            WorkbenchData::SingleChoice(data) => data.prompt.push('!'),
            _ => unreachable!("this regression fixture covers the original contracts"),
        }
        assert_eq!(
            app.request_feedback(changed).await.unwrap_err().code(),
            "REQUEST_CONFLICT"
        );
        let state = match kind {
            "questions" => json!({"type":"questions","answers":[
                {"id":"audience","value":"individuals","label":"untrusted label","wasCustom":false,"index":99},
                {"id":"scope","value":"My own plan","label":"My own plan","wasCustom":true}
            ]}),
            "single_choice" => json!({"type":"single_choice","selected_option_id":"compact"}),
            _ => Value::Null,
        };
        let doc =
            json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state});
        let saved = app
            .save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: doc.to_string(),
                body_markdown: if kind == "ramble" {
                    "Human notes".into()
                } else {
                    String::new()
                },
                expected_revision: 0,
            })
            .await
            .unwrap();
        let submitted = app
            .submit_feedback(SubmitFeedbackInput {
                request_id: created.request_id.clone(),
                expected_revision: saved.saved_revision,
                cooked_markdown: Some("A rewritten summary cannot change the selection".into()),
                cooking_model: Some("test".into()),
                uncooked_markdown: None,
            })
            .await
            .unwrap();
        let package = app
            .read_feedback_package(&submitted)
            .await
            .unwrap()
            .unwrap();
        let value = serde_json::to_value(package.manifest.workbench.unwrap()).unwrap();
        assert_eq!(value["type"], kind);
        match kind {
            "questions" => {
                assert_eq!(value["result"]["answers"][0]["value"], "individuals");
                assert_eq!(value["result"]["answers"][0]["label"], "Individuals");
                assert_eq!(value["result"]["answers"][0]["index"], 1);
                assert_eq!(value["result"]["answers"][1]["wasCustom"], true);
            }
            "single_choice" => assert_eq!(value["result"]["selected_option_id"], "compact"),
            _ => assert_eq!(value["result"]["kind"], "free_feedback"),
        }
    }
    store.close().await;
}

#[tokio::test]
async fn invalid_types_versions_and_data_fail_before_persistence_and_cancel_has_no_answer() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let example = original_workbench_example("single_choice");
    for (kind, version) in [("missing", 1), ("single_choice", 2), ("questions", 1)] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(WorkbenchSpec {
            kind: kind.into(),
            version,
            data: example.data.clone(),
        });
        assert_eq!(
            app.request_feedback(input).await.unwrap_err().code(),
            "INVALID_ARGUMENT"
        );
    }
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(example);
    let created = app.request_feedback(input).await.unwrap();
    let cancelled = app
        .cancel_feedback(CancelFeedbackInput {
            request_id: created.request_id,
            reason: "No decision".into(),
        })
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&cancelled)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(package.manifest.workbench.unwrap()).unwrap()["result"],
        Value::Null
    );
    store.close().await;
}

#[tokio::test]
async fn submission_distinguishes_no_human_input_from_incomplete_workbench() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    for kind in ["ramble", "questions", "single_choice"] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(original_workbench_example(kind));
        let created = app.request_feedback(input).await.unwrap();
        let state = match kind {
            "questions" => {
                json!({"type":"questions","answers":[{"id":"audience","value":" \n ","label":"","wasCustom":true}]})
            }
            "single_choice" => json!({"type":"single_choice","selected_option_id":null}),
            _ => Value::Null,
        };
        let doc =
            json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state})
                .to_string();
        let mut revision = 0;
        for notes in [" \n ", "Human feedback"] {
            let saved = app
                .save_feedback_draft(SaveDraftInput {
                    request_id: created.request_id.clone(),
                    document_json: doc.clone(),
                    body_markdown: notes.into(),
                    expected_revision: revision,
                })
                .await
                .unwrap();
            revision = saved.saved_revision;
            let result = app
                .submit_feedback(SubmitFeedbackInput {
                    request_id: created.request_id.clone(),
                    expected_revision: revision,
                    cooked_markdown: None,
                    cooking_model: None,
                    uncooked_markdown: None,
                })
                .await;
            if !notes.trim().is_empty() && kind == "ramble" {
                assert!(result.is_ok());
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.code(), "INVALID_ARGUMENT");
                assert_eq!(
                    error.message(),
                    if notes.trim().is_empty() {
                        "Provide workbench input or write feedback before submitting"
                    } else {
                        "Complete the workbench questions or selection before submitting; notes are optional"
                    }
                );
            }
        }
    }
    store.close().await;
}

#[tokio::test]
async fn workbench_submission_requires_valid_answers_and_cas_keeps_notes_and_answers_together() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(original_workbench_example("questions"));
    let created = app.request_feedback(input).await.unwrap();
    let answer =
        json!({"id":"audience","value":"individuals","label":"Individuals","wasCustom":false});
    let mut revision = 0;
    for answers in [
        json!([]),
        json!([answer.clone()]),
        json!([answer.clone(), answer]),
        json!([
            {"id":"audience","value":"unknown","label":"Unknown","wasCustom":false},
            {"id":"scope","value":"review","label":"Review","wasCustom":false}
        ]),
    ] {
        let doc = json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":{"type":"questions","answers":answers}}).to_string();
        let saved = app
            .save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: doc.clone(),
                body_markdown: "These notes cannot answer a question".into(),
                expected_revision: revision,
            })
            .await
            .unwrap();
        revision = saved.saved_revision;
        let error = app
            .submit_feedback(SubmitFeedbackInput {
                request_id: created.request_id.clone(),
                expected_revision: revision,
                cooked_markdown: None,
                cooking_model: None,
                uncooked_markdown: None,
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), "INVALID_ARGUMENT");
        let loaded = app
            .get_feedback_workspace(created.request_id.clone())
            .await
            .unwrap();
        assert_eq!(loaded.draft.document_json.as_deref(), Some(doc.as_str()));
    }
    assert_eq!(
        app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: "{}".into(),
            body_markdown: "Stale overwrite".into(),
            expected_revision: 0
        })
        .await
        .unwrap_err()
        .code(),
        "DRAFT_CONFLICT"
    );
    // Reopening the store also retains independently persisted interaction state.
    store.close().await;
    let reopened = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let loaded = reopened
        .clone()
        .into_application()
        .get_feedback_workspace(created.request_id)
        .await
        .unwrap();
    assert!(
        loaded
            .draft
            .document_json
            .unwrap()
            .contains("workbenchState")
    );
    reopened.close().await;
}

#[tokio::test]
async fn document_review_roundtrips_restarts_and_publishes_immutable_anchors() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(serde_json::from_value(json!({"type":"document_review","version":1,"data":{
        "title":"Speech", "source_version":"draft-1", "paragraphs":[{"id":"opening","text":"你好😀 world"}]
    }})).unwrap());
    let mut approval = input.clone();
    approval.allow_finish = true;
    approval.final_summary = Some("Execute all changes".into());
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
    let WorkbenchData::DocumentReview(data) = &mut changed.workbench.as_mut().unwrap().data else {
        unreachable!()
    };
    data.paragraphs[0].text.push('!');
    assert_eq!(
        app.request_feedback(changed).await.unwrap_err().code(),
        "REQUEST_CONFLICT"
    );
    let document = json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":{
        "type":"document_review","verdict":"changes_requested","annotations":[{
            "id":"note-1","paragraph_id":"opening","start":2,"end":3,"quote":"😀",
            "kind":"suggestion","body":"Remove emoji","replacement":"","status":"open"
        }],"paragraph_marks":[{"paragraph_id":"opening","decision":"revise"}]
    }})
    .to_string();
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: document.clone(),
            body_markdown: String::new(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert_eq!(
        app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: "{}".into(),
            body_markdown: "Stale".into(),
            expected_revision: 0
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
    assert_eq!(
        loaded.draft.document_json.as_deref(),
        Some(document.as_str())
    );
    let submitted = app
        .submit_feedback(SubmitFeedbackInput {
            request_id: created.request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&submitted)
        .await
        .unwrap()
        .unwrap();
    assert!(package.markdown.trim().is_empty());
    let workbench = serde_json::to_value(package.manifest.workbench.unwrap()).unwrap();
    assert_eq!(workbench["data"]["paragraphs"][0]["text"], "你好😀 world");
    assert_eq!(workbench["result"]["source_version"], "draft-1");
    assert_eq!(workbench["result"]["annotations"][0]["quote"], "😀");
    assert_eq!(workbench["result"]["annotations"][0]["replacement"], "");
    assert!(
        workbench["result"]["annotations"][0]
            .get("status")
            .is_none()
    );
    input.request_id = Some(Uuid::now_v7().to_string());
    let created = app.request_feedback(input).await.unwrap();
    let cancelled = app
        .cancel_feedback(CancelFeedbackInput {
            request_id: created.request_id,
            reason: "Skip this review".into(),
        })
        .await
        .unwrap();
    assert!(
        app.read_feedback_package(&cancelled)
            .await
            .unwrap()
            .unwrap()
            .manifest
            .workbench
            .unwrap()
            .result
            .is_none()
    );
    store.close().await;
}

#[tokio::test]
async fn future_workbench_requests_are_readable_but_reject_mutation_and_submission() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let input = workspace.request(Uuid::now_v7().to_string());
    let created = app.request_feedback(input).await.unwrap();
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: "{}".into(),
            body_markdown: "Text cannot bypass an unsupported contract".into(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    let future = json!({"type":"future_canvas","version":7,"data":{"shapes":[{"kind":"circle"}]}});
    sqlx::query("UPDATE feedback_requests SET workbench_json = ?2 WHERE id = ?1")
        .bind(&created.request_id)
        .bind(future.to_string())
        .execute(&store.pool)
        .await
        .unwrap();
    let loaded = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(loaded.workbench.unwrap()).unwrap(),
        future
    );
    let error = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: "{}".into(),
            body_markdown: "Overwrite".into(),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("read-only"));
    let error = app
        .submit_feedback(SubmitFeedbackInput {
            request_id: created.request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("read-only"));
    let error = app
        .reorder_feedback_attachments(ReorderAttachmentsInput {
            request_id: created.request_id.clone(),
            attachment_ids: vec![],
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("read-only"));
    let error = app
        .approve_feedback(ApproveFeedbackInput {
            request_id: created.request_id.clone(),
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("read-only"));
    let loaded = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(loaded.draft.saved_revision, saved.saved_revision);
    let cancelled = app
        .cancel_feedback(CancelFeedbackInput {
            request_id: created.request_id,
            reason: "Use a compatible app later".into(),
        })
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&cancelled)
        .await
        .unwrap()
        .unwrap();
    let workbench = package.manifest.workbench.unwrap();
    assert_eq!(serde_json::to_value(workbench.input).unwrap(), future);
    assert!(workbench.result.is_none());
    store.close().await;
}

#[tokio::test]
async fn structured_contract_cannot_use_a_legacy_approval_flag_to_skip_answers() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(
        describe_workbench(&DescribeWorkbenchInput {
            kind: "document_review".into(),
            version: Some(1),
        })
        .unwrap()
        .example,
    );
    let created = app.request_feedback(input).await.unwrap();
    // A historical/imported approval flag cannot weaken the active contract.
    sqlx::query(
        "UPDATE feedback_requests SET allow_finish = 1, final_summary = 'Done' WHERE id = ?1",
    )
    .bind(&created.request_id)
    .execute(&store.pool)
    .await
    .unwrap();
    assert_eq!(
        app.approve_feedback(ApproveFeedbackInput {
            request_id: created.request_id.clone()
        })
        .await
        .unwrap_err()
        .code(),
        "REQUEST_TERMINAL"
    );
    assert_eq!(
        app.get_feedback_workspace(created.request_id)
            .await
            .unwrap()
            .request
            .status,
        FeedbackStatus::Waiting
    );
    store.close().await;
}
