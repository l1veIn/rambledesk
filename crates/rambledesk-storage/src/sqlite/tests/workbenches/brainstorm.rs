use super::*;

mod attachments;

#[test]
fn real_playground_brainstorm_fixture_matches_advertised_agent_contract() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../playground/workbenches/fixtures/14-brainstorm.json"
    ))
    .unwrap();
    let spec: WorkbenchSpec = serde_json::from_value(fixture["workbench"].clone()).unwrap();
    assert!(rambledesk_core::validate_workbench_request(&spec).is_ok());
    let WorkbenchData::Brainstorm(rambledesk_core::BrainstormWireData::Request(input)) = spec.data
    else {
        panic!("raw Agent input")
    };
    let data =
        rambledesk_core::resolve_brainstorm_request(&input, &Uuid::now_v7().to_string(), None)
            .unwrap();
    assert_eq!(data.initial_board.cards.len(), 5);
    assert_eq!(data.initial_board.groups.len(), 2);
    assert!(
        data.initial_board
            .cards
            .iter()
            .all(|card| card.origin == rambledesk_core::BrainstormOrigin::Agent)
    );
}

fn request(workspace: &TestWorkspace, parent: Option<&str>) -> RequestFeedbackInput {
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(
        serde_json::from_value(json!({"type":"brainstorm","version":1,"data":{
            "title":"First experience", "inherit_from_request_id":parent,
            "groups":[], "cards":[{"id":if parent.is_some() { "expanded" } else { "small-task" },
                "title":"One small task","body":"Agent proposal","group_id":null,
                "source_card_ids":if parent.is_some() { vec!["small-task"] } else { vec![] }}]
        }}))
        .unwrap(),
    );
    input
}
fn document(state: Value) -> String {
    json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state}).to_string()
}
fn submission(request_id: &str, expected_revision: u64) -> SubmitFeedbackInput {
    SubmitFeedbackInput {
        request_id: request_id.into(),
        expected_revision,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    }
}
async fn published_parent(
    store: &SqliteFeedbackStore,
    input: RequestFeedbackInput,
    attachment: bool,
) -> (String, Value) {
    let app = store.clone().into_application();
    let created = app.request_feedback(input).await.unwrap();
    let initial = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(initial.draft.saved_revision, 0);
    let mut state = serde_json::from_str::<Value>(initial.draft.document_json.as_ref().unwrap())
        .unwrap()["workbenchState"]
        .clone();
    state["cards"][0]["title"] = json!("Human title survives");
    state["cards"][0]["body"] = json!("Human details survive");
    state["phase"] = json!("focus");
    state["selected_card_ids"] = json!(["small-task"]);
    state["intent"] = json!("expand");
    state["instruction"] = json!("Explore this choice");
    let mut revision = 0;
    if attachment {
        let uploaded = app
            .add_feedback_attachment(AddAttachmentInput {
                request_id: created.request_id.clone(),
                file_name: "evidence.txt".into(),
                contents: b"Preserve this evidence".to_vec(),
                expected_revision: revision,
            })
            .await
            .unwrap();
        state["cards"][0]["body"] = json!(format!(
            "[evidence](attachment://{})",
            uploaded.attachments[0].attachment_id
        ));
        revision = uploaded.draft.saved_revision;
    }
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: document(state.clone()),
            body_markdown: String::new(),
            expected_revision: revision,
        })
        .await
        .unwrap();
    app.submit_feedback(submission(&created.request_id, saved.saved_revision))
        .await
        .unwrap();
    (created.request_id, state)
}

#[tokio::test]
async fn brainstorm_inherits_published_state_and_retries_never_reset_human_edits_after_reopen() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let (parent, parent_state) = published_parent(&store, request(&workspace, None), false).await;
    // Published source drafts are immutable even at the database boundary.
    assert!(
        sqlx::query("UPDATE drafts SET document_json = '{}' WHERE request_id = ?1")
            .bind(&parent)
            .execute(&store.pool)
            .await
            .is_err()
    );
    let child = request(&workspace, Some(&parent));
    let child_id = child.request_id.clone().unwrap();
    let app = store.clone().into_application();
    app.request_feedback(child.clone()).await.unwrap();
    let initial = app.get_feedback_workspace(child_id.clone()).await.unwrap();
    let mut state = serde_json::from_str::<Value>(initial.draft.document_json.as_ref().unwrap())
        .unwrap()["workbenchState"]
        .clone();
    assert_eq!(state["cards"][0], parent_state["cards"][0]);
    assert_eq!(
        state["selected_card_ids"],
        parent_state["selected_card_ids"]
    );
    assert_eq!(state["phase"], "focus");
    assert_eq!(state["intent"], Value::Null);
    assert_eq!(state["instruction"], "");
    assert_eq!(state["cards"][1]["origin_request_id"], child_id);
    assert_eq!(state["cards"][1]["source_card_ids"], json!(["small-task"]));
    state["cards"][0]["body"] = json!("Edited again in round two");
    state["selected_card_ids"] = json!(["expanded"]);
    state["intent"] = json!("proceed");
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: child_id.clone(),
            document_json: document(state.clone()),
            body_markdown: "Overall round two".into(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    app.request_feedback(child.clone()).await.unwrap();
    let reopened = app.get_feedback_workspace(child_id.clone()).await.unwrap();
    assert_eq!(reopened.draft.saved_revision, saved.saved_revision);
    assert_eq!(reopened.draft.body_markdown, "Overall round two");
    assert_eq!(
        reopened.draft.document_json.as_deref(),
        Some(document(state.clone()).as_str())
    );
    let mut different = child.clone();
    different.workbench.as_mut().unwrap().data = serde_json::from_value::<WorkbenchSpec>(json!({
        "type":"brainstorm","data":{"title":"Different","inherit_from_request_id":parent,"groups":[],"cards":[]}
    })).unwrap().data;
    assert_eq!(
        app.request_feedback(different).await.unwrap_err().code(),
        "REQUEST_CONFLICT"
    );
    let published = app
        .submit_feedback(submission(&child_id, saved.saved_revision))
        .await
        .unwrap();
    let package = app
        .read_feedback_package(&published)
        .await
        .unwrap()
        .unwrap();
    let value = serde_json::to_value(package.manifest.workbench.unwrap()).unwrap();
    assert_eq!(value["result"]["source_version"], child_id);
    assert_eq!(value["result"]["board_id"], parent);
    assert_eq!(value["result"]["round"], 2);
    assert_eq!(value["result"]["cards"], state["cards"]);
    assert_eq!(value["result"]["selected_card_ids"], json!(["expanded"]));
    assert_eq!(value["result"]["intent"], "proceed");
    app.request_feedback(child).await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn brainstorm_rejects_unpublished_parents_cross_sessions_and_agent_forged_resolved_input() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let open = app
        .request_feedback(request(&workspace, None))
        .await
        .unwrap();
    let child = request(&workspace, Some(&open.request_id));
    let id = child.request_id.clone().unwrap();
    assert_eq!(
        app.request_feedback(child).await.unwrap_err().code(),
        "REQUEST_NOT_TERMINAL"
    );
    assert!(store.get_request_impl(&id).await.is_err());
    let (parent, _) = published_parent(&store, request(&workspace, None), false).await;
    let mut child = request(&workspace, Some(&parent));
    child.host_session_id = "different-session".into();
    assert_eq!(
        app.request_feedback(child).await.unwrap_err().code(),
        "REQUEST_NOT_FOUND"
    );
    let initial = app
        .get_feedback_workspace(open.request_id.clone())
        .await
        .unwrap();
    let wire = serde_json::to_value(initial).unwrap();
    let mut forged = request(&workspace, None);
    forged.workbench = Some(serde_json::from_value(wire["workbench"].clone()).unwrap());
    assert_eq!(
        app.request_feedback(forged).await.unwrap_err().code(),
        "INVALID_ARGUMENT"
    );
    store.close().await;
}

#[tokio::test]
async fn brainstorm_rolls_back_copied_files_and_all_database_rows_when_seeding_fails() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let (parent, _) = published_parent(&store, request(&workspace, None), true).await;
    let child = request(&workspace, Some(&parent));
    let child_id = child.request_id.clone().unwrap();
    // Fail after attachment copies and metadata insertion, at the final draft seed.
    sqlx::query("CREATE TRIGGER fail_brainstorm_seed BEFORE INSERT ON drafts BEGIN SELECT RAISE(ABORT, 'seed probe'); END")
        .execute(&store.pool).await.unwrap();
    let app = store.clone().into_application();
    assert!(app.request_feedback(child.clone()).await.is_err());
    for table in [
        "feedback_requests",
        "drafts",
        "attachments",
        "request_attachments",
    ] {
        let key = if table == "feedback_requests" {
            "id"
        } else {
            "request_id"
        };
        let count: i64 =
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE {key} = ?1"))
                .bind(&child_id)
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert_eq!(count, 0, "rolled back {table}");
    }
    let directory = store
        .library_root()
        .join("drafts")
        .join(&child_id)
        .join("attachments");
    assert!(
        tokio::fs::read_dir(&directory)
            .await
            .unwrap()
            .next_entry()
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("DROP TRIGGER fail_brainstorm_seed")
        .execute(&store.pool)
        .await
        .unwrap();
    app.request_feedback(child.clone()).await.unwrap();
    let restored = app.get_feedback_workspace(child_id.clone()).await.unwrap();
    assert_eq!(restored.attachments.len(), 1);
    let attachment_id = restored.attachments[0].attachment_id.clone();
    app.request_feedback(child).await.unwrap();
    let retry = app.get_feedback_workspace(child_id).await.unwrap();
    assert_eq!(retry.attachments.len(), 1);
    assert_eq!(retry.attachments[0].attachment_id, attachment_id);
    store.close().await;
}

#[tokio::test]
async fn brainstorm_cancellation_of_untouched_seed_preserves_board_and_inherited_attachments() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let (parent, _) = published_parent(&store, request(&workspace, None), true).await;
    let app = store.clone().into_application();
    for parent_id in [None, Some(parent.as_str())] {
        let created = app
            .request_feedback(request(&workspace, parent_id))
            .await
            .unwrap();
        let initial = app
            .get_feedback_workspace(created.request_id.clone())
            .await
            .unwrap();
        assert_eq!(initial.draft.saved_revision, 0);
        let cancelled = app
            .cancel_feedback(CancelFeedbackInput {
                request_id: created.request_id.clone(),
                reason: "Stop this round".into(),
            })
            .await
            .unwrap();
        assert_eq!(cancelled.status, FeedbackStatus::Cancelled);
        let after = app
            .get_feedback_workspace(created.request_id.clone())
            .await
            .unwrap();
        assert_eq!(after.draft.document_json, initial.draft.document_json);
        assert_eq!(after.draft.saved_revision, 1);
        let package = app
            .read_feedback_package(&cancelled)
            .await
            .unwrap()
            .unwrap();
        let workbench = package.manifest.workbench.as_ref().unwrap();
        assert_eq!(Some(&workbench.input), initial.workbench.as_ref());
        assert!(workbench.result.is_none());
        if parent_id.is_some() {
            assert_eq!(package.attachment_paths.len(), 1);
            assert_eq!(
                tokio::fs::read(&package.attachment_paths[0]).await.unwrap(),
                b"Preserve this evidence"
            );
        }
        assert_eq!(
            app.request_feedback(request(&workspace, Some(&created.request_id)))
                .await
                .unwrap_err()
                .code(),
            "INVALID_ARGUMENT"
        );
    }
    store.close().await;
}
