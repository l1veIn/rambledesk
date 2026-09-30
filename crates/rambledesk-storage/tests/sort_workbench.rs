use rambledesk_core::{
    DescribeWorkbenchInput, FeedbackStatus, GetFeedbackInput, ListFeedbackRequestsInput,
    ListWorkbenchesInput, RequestFeedbackInput, SaveDraftInput, SubmitFeedbackInput, WorkbenchData,
    WorkbenchPackage, WorkbenchResult, WorkbenchSpec, describe_workbench, list_workbenches,
};
use rambledesk_storage::SqliteFeedbackStore;
use serde_json::{Value, json};
use uuid::Uuid;

fn spec() -> WorkbenchSpec {
    serde_json::from_value(json!({"type":"sort","version":1,"data":{
        "title":"Arrange the next tasks", "items":[
            {"id":"first","label":"First task"},
            {"id":"second","label":"Second task"},
            {"id":"third","label":"Third task"}
        ]
    }}))
    .unwrap()
}

fn request(workbench: WorkbenchSpec) -> RequestFeedbackInput {
    RequestFeedbackInput {
        request_id: Some(Uuid::now_v7().to_string()),
        host_id: Some("sort-test".into()),
        host_session_id: "sort-session".into(),
        title: Some("Priority review".into()),
        what_happened: "Prioritize the tasks before continuing.".into(),
        workbench: Some(workbench),
        actions: vec![],
        context_refs: vec![],
        attachments: vec![],
        source_hint: None,
        allow_finish: false,
        final_summary: None,
    }
}

fn document(state: Value) -> String {
    json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state}).to_string()
}

fn submit(request_id: &str, revision: u64) -> SubmitFeedbackInput {
    SubmitFeedbackInput {
        request_id: request_id.into(),
        expected_revision: revision,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    }
}

#[test]
fn sort_is_discoverable_with_separate_typed_schemas_and_future_inputs_remain_opaque() {
    let catalog = list_workbenches(&ListWorkbenchesInput {
        offset: 0,
        limit: Some(20),
    })
    .unwrap();
    assert!(catalog.workbenches.iter().any(|entry| entry.kind == "sort"));
    let description = describe_workbench(&DescribeWorkbenchInput {
        kind: "sort".into(),
        version: Some(1),
    })
    .unwrap();
    let input = serde_json::to_value(description.input_schema).unwrap();
    assert_eq!(input["properties"]["items"]["minItems"], 2);
    assert_eq!(input["properties"]["items"]["maxItems"], 30);
    let result = serde_json::to_value(description.result_schema).unwrap();
    assert_eq!(result["properties"]["order"]["type"], "array");
    let item_type = &result["properties"]["items"]["type"];
    assert!(
        item_type == "array"
            || item_type
                .as_array()
                .is_some_and(|types| types.contains(&json!("array")))
    );
    assert_eq!(result["properties"]["removed_ids"]["type"], "array");
    assert!(rambledesk_core::validate_workbench(&description.example).is_ok());
    assert!(
        rambledesk_core::workbench_actions(&description.example)
            .unwrap()
            .is_empty()
    );
    let broad = serde_json::to_string(&schemars::schema_for!(RequestFeedbackInput)).unwrap();
    assert!(!broad.contains("SortData"));
    let future = json!({"type":"sort","version":2,"data":{
        "title":"Future", "items":[], "future":{"preserve":true}
    }});
    let decoded: WorkbenchSpec = serde_json::from_value(future.clone()).unwrap();
    assert!(matches!(decoded.data, WorkbenchData::Unknown(_)));
    assert_eq!(serde_json::to_value(decoded).unwrap(), future);
    let mut historical = serde_json::to_value(spec()).unwrap();
    historical["result"] = json!({"order":["third","second","first"]});
    let decoded: WorkbenchPackage = serde_json::from_value(historical).unwrap();
    assert!(
        matches!(decoded.result, Some(WorkbenchResult::Sort(result)) if result.order == ["third","second","first"])
    );
}

#[tokio::test]
async fn sort_reopens_edited_and_deleted_items_and_publishes_immutable_history() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("state.sqlite");
    let store = SqliteFeedbackStore::connect(&database).await.unwrap();
    let app = store.clone().into_application();
    let input = request(spec());
    let created = app.request_feedback(input.clone()).await.unwrap();
    assert_eq!(
        app.request_feedback(input.clone())
            .await
            .unwrap()
            .request_id,
        created.request_id
    );
    let mut changed = input.clone();
    let WorkbenchData::Sort(data) = &mut changed.workbench.as_mut().unwrap().data else {
        panic!("typed sort")
    };
    data.items[0].label.push('!');
    assert_eq!(
        app.request_feedback(changed).await.unwrap_err().code(),
        "REQUEST_CONFLICT"
    );
    let ready = document(json!({"type":"sort","order":["third","first"],
    "removed_ids":["second"], "edited_items":[
        {"id":"first","label":"First task, revised"},
        {"id":"second","label":"Second task, preserved for restore"}
    ]}));
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: ready.clone(),
            body_markdown: String::new(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    store.close().await;

    let store = SqliteFeedbackStore::connect(&database).await.unwrap();
    let app = store.clone().into_application();
    let recovered = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(recovered.workbench, input.workbench);
    assert!(recovered.actions.is_empty());
    assert_eq!(
        recovered.draft.document_json.as_deref(),
        Some(ready.as_str())
    );
    assert_eq!(recovered.draft.saved_revision, saved.saved_revision);
    assert_eq!(
        app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: document(json!({"type":"sort","order":["first","second","third"]})),
            body_markdown: "Stale overwrite".into(),
            expected_revision: 0,
        })
        .await
        .unwrap_err()
        .code(),
        "DRAFT_CONFLICT"
    );
    assert_eq!(
        app.submit_feedback(submit(&created.request_id, 0))
            .await
            .unwrap_err()
            .code(),
        "DRAFT_CONFLICT"
    );
    let submitted = app
        .submit_feedback(submit(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&submitted)
        .await
        .unwrap()
        .unwrap();
    assert!(package.markdown.trim().is_empty());
    let workbench = package.manifest.workbench.unwrap();
    assert_eq!(workbench.input, input.workbench.unwrap());
    assert!(
        matches!(&workbench.result, Some(WorkbenchResult::Sort(result)) if result.order == ["third","first"])
    );
    let frozen = serde_json::to_value(workbench).unwrap();
    assert_eq!(
        frozen["result"]["items"],
        json!([
            {"id":"third","label":"Third task"},
            {"id":"first","label":"First task, revised"}
        ])
    );
    assert_eq!(frozen["result"]["removed_ids"], json!(["second"]));
    assert_eq!(
        app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: document(json!({"type":"sort","order":["first","second","third"]})),
            body_markdown: "After publication".into(),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap_err()
        .code(),
        "REQUEST_TERMINAL"
    );
    let repeated = app
        .submit_feedback(submit(&created.request_id, 0))
        .await
        .unwrap();
    assert_eq!(repeated.feedback, submitted.feedback);
    let history = app
        .list_feedback_requests(ListFeedbackRequestsInput {
            host_id: Some("sort-test".into()),
            host_session_id: Some("sort-session".into()),
            status: Some(vec![FeedbackStatus::Completed]),
            archived: None,
            search: None,
            limit: None,
            cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(history.requests.len(), 1);
    assert_eq!(history.requests[0].request_id, created.request_id);
    store.close().await;

    let store = SqliteFeedbackStore::connect(&database).await.unwrap();
    let app = store.clone().into_application();
    let historical = app
        .get_feedback(GetFeedbackInput {
            request_id: created.request_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(historical.status, FeedbackStatus::Completed);
    let published = app
        .read_feedback_package(&historical)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(published.manifest.workbench.unwrap()).unwrap(),
        frozen
    );
    assert_eq!(
        app.get_feedback_workspace(created.request_id)
            .await
            .unwrap()
            .draft
            .document_json
            .as_deref(),
        Some(ready.as_str())
    );
    store.close().await;
}

#[tokio::test]
async fn invalid_sort_states_cannot_replace_drafts_or_publish_through_body_text() {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteFeedbackStore::connect(&temp.path().join("state.sqlite"))
        .await
        .unwrap();
    let app = store.clone().into_application();
    let created = app.request_feedback(request(spec())).await.unwrap();
    let absent = document(Value::Null);
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: absent.clone(),
            body_markdown: "Body text cannot replace a sort result.".into(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert_eq!(
        app.submit_feedback(submit(&created.request_id, saved.saved_revision))
            .await
            .unwrap_err()
            .code(),
        "INVALID_ARGUMENT"
    );
    for state in [
        json!({"type":"sort","order":[]}),
        json!({"type":"sort","order":["first","second"]}),
        json!({"type":"sort","order":["first","first","third"]}),
        json!({"type":"sort","order":["first","second","foreign"]}),
        json!({"type":"sort","order":["first","second","third","foreign"]}),
        json!({"type":"sort","order":["first","second","third"],"unknown":true}),
        json!({"type":"sort","order":"first,second,third"}),
        json!({"type":"sort"}),
        json!({"type":"sort","order":["first","second","third"],"removed_ids":["third"]}),
        json!({"type":"sort","order":["first","third"],"removed_ids":["second","second"]}),
        json!({"type":"sort","order":["first","third"],"removed_ids":["foreign"]}),
        json!({"type":"sort","order":["first"],"removed_ids":["second"]}),
        json!({"type":"sort","order":["first","second","third"],"edited_items":[{"id":"foreign","label":"Unknown"}]}),
        json!({"type":"sort","order":["first","second","third"],"edited_items":[{"id":"first","label":"One"},{"id":"first","label":"Again"}]}),
        json!({"type":"sort","order":["first","second","third"],"edited_items":[{"id":"first","label":"x".repeat(201)}]}),
        json!({"type":"sort","order":["first","second","third"],"edited_items":[{"id":"first","label":"nul\u{0}label"}]}),
        json!({"type":"sort","order":["first","second","third"],"edited_items":[{"id":"first","label":"Changed","unknown":true}]}),
        json!({"type":"sort","order":["first","second","third"],"removed_ids":"second"}),
    ] {
        assert_eq!(
            app.save_feedback_draft(SaveDraftInput {
                request_id: created.request_id.clone(),
                document_json: document(state.clone()),
                body_markdown: "Notes cannot authorize invalid state.".into(),
                expected_revision: saved.saved_revision,
            })
            .await
            .unwrap_err()
            .code(),
            "INVALID_ARGUMENT",
            "{state}"
        );
        let loaded = app
            .get_feedback_workspace(created.request_id.clone())
            .await
            .unwrap();
        assert_eq!(loaded.draft.saved_revision, saved.saved_revision);
        assert_eq!(loaded.draft.document_json.as_deref(), Some(absent.as_str()));
        assert!(loaded.feedback.is_none());
    }
    // Confirming the original order is a valid preference; moving an item is optional.
    let ready = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: document(json!({"type":"sort","order":["first","second","third"]})),
            body_markdown: String::new(),
            expected_revision: saved.saved_revision,
        })
        .await
        .unwrap();
    assert!(
        app.submit_feedback(submit(&created.request_id, ready.saved_revision))
            .await
            .is_ok()
    );
    store.close().await;
}

#[tokio::test]
async fn unfinished_sort_labels_survive_restart_but_require_repair_or_explicit_removal() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("state.sqlite");
    let store = SqliteFeedbackStore::connect(&database).await.unwrap();
    let app = store.clone().into_application();
    let created = app.request_feedback(request(spec())).await.unwrap();
    let blank = document(json!({"type":"sort","order":["first","second","third"],
        "edited_items":[{"id":"first","label":" "}]}));
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: blank.clone(),
            body_markdown: "These notes must not authorize an unfinished item label.".into(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    store.close().await;

    let store = SqliteFeedbackStore::connect(&database).await.unwrap();
    let app = store.clone().into_application();
    let recovered = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(
        recovered.draft.document_json.as_deref(),
        Some(blank.as_str())
    );
    assert_eq!(
        app.submit_feedback(submit(&created.request_id, saved.saved_revision))
            .await
            .unwrap_err()
            .code(),
        "INVALID_ARGUMENT"
    );
    assert!(
        app.get_feedback_workspace(created.request_id.clone())
            .await
            .unwrap()
            .feedback
            .is_none()
    );

    let removed = document(json!({"type":"sort","order":["second","third"],
        "removed_ids":["first"], "edited_items":[{"id":"first","label":" "}]}));
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: removed.clone(),
            body_markdown: String::new(),
            expected_revision: recovered.draft.saved_revision,
        })
        .await
        .unwrap();
    let published = app
        .submit_feedback(submit(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&published)
        .await
        .unwrap()
        .unwrap();
    let result = serde_json::to_value(package.manifest.workbench.unwrap().result.unwrap()).unwrap();
    assert_eq!(result["order"], json!(["second", "third"]));
    assert_eq!(result["removed_ids"], json!(["first"]));
    assert_eq!(
        result["items"],
        json!([
            {"id":"second","label":"Second task"},
            {"id":"third","label":"Third task"}
        ])
    );
    assert_eq!(
        app.get_feedback_workspace(created.request_id)
            .await
            .unwrap()
            .draft
            .document_json
            .as_deref(),
        Some(removed.as_str())
    );
    store.close().await;
}

#[tokio::test]
async fn explicitly_removing_every_sort_item_publishes_a_meaningful_empty_list() {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteFeedbackStore::connect(&temp.path().join("state.sqlite"))
        .await
        .unwrap();
    let app = store.clone().into_application();
    let created = app.request_feedback(request(spec())).await.unwrap();
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: document(
                json!({"type":"sort","order":[],"removed_ids":["third","first","second"]}),
            ),
            body_markdown: String::new(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    let published = app
        .submit_feedback(submit(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&published)
        .await
        .unwrap()
        .unwrap();
    let result = serde_json::to_value(package.manifest.workbench.unwrap().result.unwrap()).unwrap();
    assert_eq!(result["order"], json!([]));
    assert_eq!(result["items"], json!([]));
    let mut removed = serde_json::from_value::<Vec<String>>(result["removed_ids"].clone()).unwrap();
    removed.sort();
    assert_eq!(removed, ["first", "second", "third"]);
    store.close().await;
}

#[tokio::test]
async fn invalid_sort_request_data_is_rejected_before_creating_a_request() {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteFeedbackStore::connect(&temp.path().join("state.sqlite"))
        .await
        .unwrap();
    let app = store.clone().into_application();
    for data in [
        json!({"title":"Sort","items":[]}),
        json!({"title":"Sort","items":[{"id":"only","label":"Only"}]}),
        json!({"title":"Sort","items":[{"id":"same","label":"One"},{"id":"same","label":"Two"}]}),
        json!({"title":"Sort","items":[{"id":"one","label":" "},{"id":"two","label":"Two"}]}),
        json!({"title":"Sort","items":[{"id":"one","label":"One"},{"id":"two","label":"Two"}],"unknown":true}),
        json!({"title":"Sort","items":(0..31).map(|n| json!({"id":format!("item-{n}"),"label":format!("Item {n}")})).collect::<Vec<_>>()}),
    ] {
        let workbench =
            serde_json::from_value(json!({"type":"sort","version":1,"data":data.clone()})).unwrap();
        assert_eq!(
            app.request_feedback(request(workbench))
                .await
                .unwrap_err()
                .code(),
            "INVALID_ARGUMENT",
            "{data}"
        );
    }
    assert!(app.list_open_feedback_requests().await.unwrap().is_empty());
    store.close().await;
}
