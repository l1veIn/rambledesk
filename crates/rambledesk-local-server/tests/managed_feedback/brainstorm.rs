//! Real MCP/command requests, SQLite drafts and immutable packages across rounds.
use super::*;
use rambledesk_core::{AddAttachmentInput, SaveDraftInput, SubmitFeedbackInput};

fn first_input(id: &str) -> Value {
    json!({"request_id":id,"what_happened":"Brainstorm a small first release", "workbench":{
        "type":"brainstorm","version":1,"data":{
            "title":"First release","inherit_from_request_id":null,
            "groups":[{"id":"seed","title":"Initial ideas"}],
            "cards":[
                {"id":"idea-a","title":"Quick start","body":"Agent seed", "group_id":"seed","source_card_ids":[]},
                {"id":"idea-b","title":"Team setup","body":"Later", "group_id":null,"source_card_ids":[]}
            ]
        }
    }})
}

fn next_input(id: &str, parent: &str) -> Value {
    json!({"request_id":id,"what_happened":"Combine the directions the human selected", "workbench":{
        "type":"brainstorm","version":1,"data":{
            "title":"First release","inherit_from_request_id":parent,
            "groups":[{"id":"proposals","title":"New proposals"}],
            "cards":[{"id":"combined","title":"A concrete first task","body":"New Agent proposal",
                "group_id":"proposals","source_card_ids":["idea-a","human-idea"]}]
        }
    }})
}

#[tokio::test]
async fn brainstorm_managed_rounds_preserve_human_edits_choices_materials_and_retry_drafts()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let endpoint = fixture.provider.bind(&fixture.sessions[0]).await?;
    let session = fixture.initialize(&endpoint).await?;
    let other = fixture.provider.bind(&fixture.sessions[1]).await?;
    let app = fixture.store.clone().into_application();
    let first = uuid::Uuid::now_v7().to_string();
    let created = fixture
        .call(&endpoint, &session, "request_feedback", first_input(&first))
        .await?;
    assert_eq!(
        created["structuredContent"]["request_id"], first,
        "{created}"
    );
    let workspace = app.get_feedback_workspace(first.clone()).await?;
    let resolved = serde_json::to_value(workspace.workbench)?;
    assert_eq!(resolved["data"]["round"], 1);
    assert_eq!(resolved["data"]["board_id"], first);
    assert_eq!(resolved["data"]["source_version"], first);
    let bytes = b"Human reference: start with one concrete task.".to_vec();
    let uploaded = app
        .add_feedback_attachment(AddAttachmentInput {
            request_id: first.clone(),
            file_name: "reference.txt".into(),
            contents: bytes.clone(),
            expected_revision: 0,
        })
        .await?;
    let parent_attachment = uploaded.attachments[0].attachment_id.clone();
    let mut state = resolved["data"]["initial_board"].clone();
    state["type"] = json!("brainstorm");
    state["phase"] = json!("focus");
    state["intent"] = json!("combine");
    state["instruction"] = json!("Keep the human wording and combine only these two ideas.");
    state["groups"][0]["title"] = json!("Human grouping");
    state["groups"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"human-group","title":"Try first"}));
    state["cards"][0]["title"] = json!("人修改的标题😀");
    state["cards"][0]["body"] = json!(format!(
        "人修改的文字😀\n\n[reference.txt](attachment://{parent_attachment})"
    ));
    state["cards"][0]["group_id"] = json!("human-group");
    state["cards"][1]["parked"] = json!(true);
    state["cards"].as_array_mut().unwrap().push(json!({
        "id":"human-idea","title":"My constraint","body":"One person can finish it",
        "group_id":"human-group","source_card_ids":[],"origin":"human","origin_request_id":first,"parked":false,
    }));
    state["selected_card_ids"] = json!(["idea-a", "human-idea"]);
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: first.clone(),
            document_json:
                json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state})
                    .to_string(),
            body_markdown: String::new(),
            expected_revision: uploaded.draft.saved_revision,
        })
        .await?;
    app.submit_feedback(SubmitFeedbackInput {
        request_id: first.clone(),
        expected_revision: saved.saved_revision,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    })
    .await?;
    let published = fixture
        .call(
            &endpoint,
            &session,
            "get_feedback",
            json!({"request_id":first}),
        )
        .await?;
    let parent_result =
        &published["structuredContent"]["feedback_package"]["manifest"]["workbench"]["result"];
    assert_eq!(parent_result["cards"][0]["title"], "人修改的标题😀");
    assert_eq!(
        parent_result["selected_card_ids"],
        json!(["idea-a", "human-idea"])
    );
    let first_package = published["structuredContent"]["feedback_package"].clone();

    let second = uuid::Uuid::now_v7().to_string();
    let input = next_input(&second, &first);
    let created: Value = fixture
        .command(&endpoint, "request", input.clone())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(created["request_id"], second);
    let child = app.get_feedback_workspace(second.clone()).await?;
    let child_spec = serde_json::to_value(&child.workbench)?;
    let board = &child_spec["data"]["initial_board"];
    assert_eq!(child_spec["data"]["board_id"], first);
    assert_eq!(child_spec["data"]["round"], 2);
    assert_eq!(child_spec["data"]["source_version"], second);
    assert_eq!(board["cards"].as_array().unwrap().len(), 4);
    assert_eq!(board["cards"][0]["title"], "人修改的标题😀");
    assert_eq!(board["cards"][0]["group_id"], "human-group");
    assert_eq!(board["cards"][1]["parked"], true);
    assert_eq!(board["cards"][2], parent_result["cards"][2]);
    assert_eq!(
        board["cards"][3]["source_card_ids"],
        json!(["idea-a", "human-idea"])
    );
    assert_eq!(board["cards"][3]["origin_request_id"], second);
    assert_eq!(board["groups"][0]["title"], "Human grouping");
    assert_eq!(board["selected_card_ids"], json!(["idea-a", "human-idea"]));
    assert_eq!(board["phase"], "focus");
    assert_eq!(child.attachments.len(), 1);
    let child_attachment = &child.attachments[0];
    assert_ne!(child_attachment.attachment_id, parent_attachment);
    assert_eq!(child_attachment.sha256, uploaded.attachments[0].sha256);
    assert_eq!(
        app.read_feedback_attachment(second.clone(), child_attachment.attachment_id.clone())
            .await?,
        bytes
    );
    assert_eq!(
        board["cards"][0]["body"],
        format!(
            "人修改的文字😀\n\n[reference.txt](attachment://{})",
            child_attachment.attachment_id
        )
    );
    let mut child_document: Value =
        serde_json::from_str(child.draft.document_json.as_deref().unwrap())?;
    assert!(child_document["workbenchState"]["intent"].is_null());
    assert_eq!(child_document["workbenchState"]["instruction"], "");
    child_document["workbenchState"]["cards"][0]["body"] = json!(format!(
        "Second-round human edit\n\n[reference.txt](attachment://{})",
        child_attachment.attachment_id
    ));
    child_document["workbenchState"]["intent"] = json!("proceed");
    child_document["workbenchState"]["selected_card_ids"] = json!(["combined"]);
    let second_saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: second.clone(),
            document_json: child_document.to_string(),
            body_markdown: String::new(),
            expected_revision: child.draft.saved_revision,
        })
        .await?;
    let replay = fixture
        .call(&endpoint, &session, "request_feedback", input.clone())
        .await?;
    assert_eq!(replay["structuredContent"]["request_id"], second);
    let after_retry = app.get_feedback_workspace(second.clone()).await?;
    assert_eq!(
        after_retry.draft.document_json.as_deref(),
        Some(child_document.to_string()).as_deref()
    );
    assert_eq!(
        after_retry.draft.saved_revision,
        second_saved.saved_revision
    );
    assert_eq!(after_retry.attachments, child.attachments);

    // A different trusted binding cannot inherit this board, even with its known ID.
    let cross = fixture
        .command(
            &other,
            "request",
            next_input(&uuid::Uuid::now_v7().to_string(), &first),
        )
        .send()
        .await?;
    assert_eq!(cross.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(cross.json::<Value>().await?["code"], "REQUEST_NOT_FOUND");
    // Reusing an existing card ID cannot overwrite human edits in a new request.
    let mut collision = next_input(&uuid::Uuid::now_v7().to_string(), &first);
    collision["workbench"]["data"]["cards"][0]["id"] = json!("idea-a");
    let collision = fixture
        .command(&endpoint, "request", collision)
        .send()
        .await?;
    assert_eq!(collision.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(collision.json::<Value>().await?["code"], "INVALID_ARGUMENT");
    app.submit_feedback(SubmitFeedbackInput {
        request_id: second.clone(),
        expected_revision: second_saved.saved_revision,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    })
    .await?;
    let published_child = fixture
        .call(
            &endpoint,
            &session,
            "get_feedback",
            json!({"request_id":second}),
        )
        .await?;
    let child_package = &published_child["structuredContent"]["feedback_package"];
    assert_eq!(
        child_package["manifest"]["workbench"]["result"]["intent"],
        "proceed"
    );
    assert_eq!(
        child_package["manifest"]["workbench"]["result"]["selected_card_ids"],
        json!(["combined"])
    );
    assert_eq!(
        child_package["manifest"]["attachments"][0]["id"],
        child_attachment.attachment_id
    );
    assert_eq!(
        child_package["manifest"]["attachments"][0]["sha256"],
        uploaded.attachments[0].sha256
    );
    let reread = fixture
        .call(
            &endpoint,
            &session,
            "get_feedback",
            json!({"request_id":first}),
        )
        .await?;
    assert_eq!(
        reread["structuredContent"]["feedback_package"],
        first_package
    );
    fixture.server.shutdown().await?;
    fixture.store.close().await;
    Ok(())
}

async fn external_request(fixture: &Fixture, input: &Value) -> anyhow::Result<reqwest::Response> {
    Ok(fixture
        .client
        .post(format!(
            "http://{}/api/feedback/request",
            fixture.server.address()
        ))
        .bearer_auth(GLOBAL_TOKEN)
        .json(input)
        .send()
        .await?)
}

#[tokio::test]
async fn brainstorm_external_rounds_preserve_state_and_reject_mixed_or_cross_conversation_inheritance()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let endpoint = fixture.provider.bind(&fixture.sessions[0]).await?;
    let app = fixture.store.clone().into_application();
    let first = uuid::Uuid::now_v7().to_string();
    let mut input = first_input(&first);
    input["host_id"] = json!(fixture.sessions[0].host_id);
    input["host_session_id"] = json!("external-brainstorm-conversation");
    let response = external_request(&fixture, &input)
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    assert_eq!(response["request_id"], first);
    let workspace = app.get_feedback_workspace(first.clone()).await?;
    let spec = serde_json::to_value(&workspace.workbench)?;
    let mut state = spec["data"]["initial_board"].clone();
    state["type"] = json!("brainstorm");
    state["intent"] = json!("expand");
    state["instruction"] = json!("Make this feasible for one person");
    state["selected_card_ids"] = json!(["idea-a"]);
    state["cards"][0]["title"] = json!("A human-authored direction");
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: first.clone(),
            document_json:
                json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state})
                    .to_string(),
            body_markdown: String::new(),
            expected_revision: workspace.draft.saved_revision,
        })
        .await?;
    app.submit_feedback(SubmitFeedbackInput {
        request_id: first.clone(),
        expected_revision: saved.saved_revision,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    })
    .await?;
    let second = uuid::Uuid::now_v7().to_string();
    let mut next = next_input(&second, &first);
    next["host_id"] = input["host_id"].clone();
    next["host_session_id"] = input["host_session_id"].clone();
    next["workbench"]["data"]["cards"][0]["source_card_ids"] = json!(["idea-a"]);
    let response = external_request(&fixture, &next)
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    assert_eq!(response["request_id"], second);
    let child = app.get_feedback_workspace(second.clone()).await?;
    let child_spec = serde_json::to_value(child.workbench)?;
    assert_eq!(child_spec["data"]["round"], 2);
    assert_eq!(
        child_spec["data"]["initial_board"]["cards"][0]["title"],
        "A human-authored direction"
    );
    assert_eq!(
        child_spec["data"]["initial_board"]["selected_card_ids"],
        json!(["idea-a"])
    );
    assert_eq!(
        fixture.store.get_request(&second).await?.managed_session_id,
        None
    );
    let mut cross = next.clone();
    cross["request_id"] = json!(uuid::Uuid::now_v7().to_string());
    cross["host_session_id"] = json!("different-external-conversation");
    let denied = external_request(&fixture, &cross).await?;
    assert_eq!(denied.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(denied.json::<Value>().await?["code"], "REQUEST_NOT_FOUND");
    let mut mixed = next;
    mixed["request_id"] = json!(uuid::Uuid::now_v7().to_string());
    let denied = fixture.command(&endpoint, "request", mixed).send().await?;
    assert_eq!(denied.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(denied.json::<Value>().await?["code"], "REQUEST_NOT_FOUND");
    fixture.server.shutdown().await?;
    fixture.store.close().await;
    Ok(())
}
