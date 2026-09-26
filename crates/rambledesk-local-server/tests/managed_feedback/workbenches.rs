use super::*;

#[tokio::test]
async fn workbench_discovery_and_typed_requests_share_mcp_and_managed_command_contracts()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let endpoint = fixture.provider.bind(&fixture.sessions[0]).await?;
    let session = fixture.initialize(&endpoint).await?;
    let search = fixture
        .call(&endpoint, &session, "list_workbenches", json!({"limit":1}))
        .await?;
    assert_eq!(
        search["structuredContent"]["workbenches"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(search["structuredContent"]["next_offset"], 1);
    assert!(!search.to_string().contains("input_schema"));
    let client = &fixture.client;
    // The generic JSON adapter exposes the same authenticated catalog.
    let generic: Value = client
        .post(format!(
            "http://{}/api/workbenches/list",
            fixture.server.address()
        ))
        .bearer_auth(GLOBAL_TOKEN)
        .json(&json!({}))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        generic["workbenches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["ramble", "questions", "document_review"]
    );
    let legacy_description = fixture
        .command(
            &endpoint,
            "describe_workbench",
            json!({"type":"single_choice"}),
        )
        .send()
        .await?;
    assert_eq!(
        legacy_description.status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    for kind in ["ramble", "questions", "single_choice", "document_review"] {
        let mut workbench = if kind == "single_choice" {
            // Already integrated clients keep their original request/result contract.
            json!({"type":"single_choice","version":1,"data":{
                "prompt":"Which layout should we use?","options":[
                    {"id":"compact","label":"Compact layout"},
                    {"id":"spacious","label":"Spacious layout"}
                ]
            }})
        } else {
            let description: Value = fixture
                .command(&endpoint, "describe_workbench", json!({"type":kind}))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let mcp_description = fixture
                .call(
                    &endpoint,
                    &session,
                    "describe_workbench",
                    json!({"type":kind}),
                )
                .await?;
            assert_eq!(description, mcp_description["structuredContent"]);
            description["example"].clone()
        };
        if kind == "questions" {
            // Extend the single-question discovery example to cover long custom answers too.
            workbench["data"]["questions"].as_array_mut().unwrap().push(json!({
                "id":"scope","prompt":"What should we build first?","allowOther":true,
                "options":[{"value":"feedback","label":"Feedback"},{"value":"review","label":"Review"}]
            }));
        }
        let request_id = uuid::Uuid::now_v7().to_string();
        let input = json!({"request_id":request_id,"what_happened":"Review this example", "workbench":workbench});
        let request = fixture
            .call(&endpoint, &session, "request_feedback", input.clone())
            .await?;
        assert_eq!(request["structuredContent"]["request_id"], request_id);
        let replay: Value = fixture
            .command(&endpoint, "request", input)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        assert_eq!(replay["request_id"], request_id);
        let app = fixture.store.clone().into_application();
        let state = match kind {
            "questions" => json!({"type":"questions","answers":[
                {"id":"audience","value":"individuals","label":"Individuals","wasCustom":false,"index":1},
                {"id":"scope","value":"自定义回答".repeat(400),"label":"自定义回答".repeat(400),"wasCustom":true}
            ]}),
            "single_choice" => json!({"type":"single_choice","selected_option_id":"compact"}),
            "document_review" => {
                json!({"type":"document_review","verdict":"ready","annotations":[],"paragraph_marks":[]})
            }
            _ => Value::Null,
        };
        let doc =
            json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state});
        let draft = app
            .save_feedback_draft(rambledesk_core::SaveDraftInput {
                request_id: request_id.clone(),
                document_json: doc.to_string(),
                body_markdown: if kind == "ramble" {
                    "Feedback".into()
                } else {
                    String::new()
                },
                expected_revision: 0,
            })
            .await?;
        app.submit_feedback(rambledesk_core::SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: draft.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await?;
        let result = fixture
            .call(
                &endpoint,
                &session,
                "get_feedback",
                json!({"request_id":request_id}),
            )
            .await?;
        assert_eq!(
            result["structuredContent"]["feedback_package"]["manifest"]["workbench"]["type"],
            kind
        );
        let text = result["content"][0]["text"].as_str().unwrap();
        let manifest_path = result["structuredContent"]["feedback"]["manifest_path"]
            .as_str()
            .unwrap();
        assert!(text.contains(manifest_path));
        if kind == "ramble" {
            assert!(text.contains("Preview of feedback markdown:\nFeedback"));
            assert!(!text.contains("Preview of structured workbench result"));
        } else {
            assert!(text.contains("workbench.result"));
            assert!(text.contains("No supplemental notes were submitted."));
            assert!(!text.contains("Preview of feedback markdown:"));
            assert!(text.contains(if kind == "questions" {
                "individuals"
            } else if kind == "document_review" {
                "draft-1"
            } else {
                "compact"
            }));
            assert!(
                result["structuredContent"]["feedback_package"]["markdown"]
                    .as_str()
                    .unwrap()
                    .trim()
                    .is_empty()
            );
            if kind == "questions" {
                assert!(text.contains("preview truncated — read workbench.result"));
                assert_eq!(
                    result["structuredContent"]["feedback_package"]["manifest"]["workbench"]["result"]
                        ["answers"][1]["value"],
                    "自定义回答".repeat(400)
                );
            }
        }
    }
    fixture
        .provider
        .revoke(&fixture.sessions[0].session_id)
        .await?;
    assert_eq!(
        fixture
            .command(&endpoint, "list_workbenches", json!({}))
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    fixture.server.shutdown().await?;
    fixture.store.close().await;
    Ok(())
}
