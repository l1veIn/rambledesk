use anyhow::Context;
use rambledesk_core::{
    ActionInput, ApproveFeedbackInput, RequestAttachmentInput, RequestFeedbackInput,
    SaveDraftInput, SubmitFeedbackInput,
};
use rambledesk_local_server::{AccessToken, HOST_HEADER, ServerConfig, start_server};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientInfo},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};

#[path = "http_security/host_header.rs"]
mod host_header;

const TEST_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

async fn test_application()
-> anyhow::Result<(rambledesk_core::FeedbackApplication, tempfile::TempDir)> {
    let directory = tempfile::tempdir()?;
    let store = rambledesk_storage::SqliteFeedbackStore::connect(
        &directory.path().join("rambledesk.sqlite3"),
    )
    .await?;
    Ok((store.into_application(), directory))
}

#[tokio::test]
async fn rejects_missing_and_wrong_bearer_tokens() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application.clone()).await?;
    let client = reqwest::Client::new();

    let missing = client.post(server.endpoint()).send().await?;
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        missing
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer realm=\"RambleDesk\"")
    );

    let wrong = client
        .post(server.endpoint())
        .bearer_auth("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .send()
        .await?;
    assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn rejects_disallowed_origin_and_host() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application.clone()).await?;
    let client = reqwest::Client::new();

    let bad_origin = client
        .post(server.endpoint())
        .bearer_auth(TEST_TOKEN)
        .header(reqwest::header::ORIGIN, "https://evil.example")
        .body("{}")
        .send()
        .await?;
    assert_eq!(bad_origin.status(), reqwest::StatusCode::FORBIDDEN);

    let bad_host = client
        .post(server.endpoint())
        .bearer_auth(TEST_TOKEN)
        .header(reqwest::header::HOST, "evil.example")
        .body("{}")
        .send()
        .await?;
    assert_eq!(bad_host.status(), reqwest::StatusCode::FORBIDDEN);

    let api = format!("http://{}/api/feedback/get", server.address());
    let bad_api_origin = client
        .post(&api)
        .bearer_auth(TEST_TOKEN)
        .header(reqwest::header::ORIGIN, "https://evil.example")
        .json(&serde_json::json!({ "request_id": uuid::Uuid::now_v7().to_string() }))
        .send()
        .await?;
    assert_eq!(bad_api_origin.status(), reqwest::StatusCode::FORBIDDEN);

    let bad_api_host = client
        .post(api)
        .bearer_auth(TEST_TOKEN)
        .header(reqwest::header::HOST, "evil.example")
        .json(&serde_json::json!({ "request_id": uuid::Uuid::now_v7().to_string() }))
        .send()
        .await?;
    assert_eq!(bad_api_host.status(), reqwest::StatusCode::FORBIDDEN);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn official_client_exercises_feedback_lifecycle_and_errors() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application.clone()).await?;
    assert!(server.address().ip().is_loopback());

    let config = StreamableHttpClientTransportConfig::with_uri(server.endpoint().to_owned())
        .auth_header(TEST_TOKEN);
    let transport = StreamableHttpClientTransport::from_config(config);
    let client = ClientInfo::default().serve(transport).await?;

    let tools = client.peer().list_tools(Default::default()).await?;
    let tool_names: Vec<_> = tools
        .tools
        .iter()
        .map(|tool| tool.name.as_ref().to_owned())
        .collect();
    assert_eq!(tool_names.len(), 3);
    for expected in ["request_feedback", "get_feedback", "cancel_feedback"] {
        assert!(
            tool_names.iter().any(|name| name == expected),
            "missing {expected} in {tool_names:?}"
        );
    }
    let request_schema = tools
        .tools
        .iter()
        .find(|tool| tool.name.as_ref() == "request_feedback")
        .expect("request_feedback schema");
    let properties = request_schema
        .input_schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .expect("request_feedback properties");
    assert!(properties.contains_key("request_id"));
    assert!(properties.contains_key("allow_finish"));
    assert!(properties.contains_key("final_summary"));
    assert!(properties.contains_key("attachments"));
    assert!(!properties.contains_key("requestId"));

    let request_id = uuid::Uuid::now_v7().to_string();
    let review_markdown = format!("# Review artifact\n\n{}", "x".repeat(300 * 1024));
    let request = RequestFeedbackInput {
        request_id: Some(request_id.clone()),
        host_id: Some("official-rust-sdk".to_owned()),
        host_session_id: "http-security-test".to_owned(),
        title: Some("MCP connection review".to_owned()),
        what_happened: "The MCP feedback tools were connected.".to_owned(),
        actions: vec![ActionInput {
            id: "verify".to_owned(),
            instruction: "Verify the persisted feedback request.".to_owned(),
        }],
        context_refs: Vec::new(),
        attachments: vec![RequestAttachmentInput {
            file_name: "review.md".to_owned(),
            markdown: Some(review_markdown.clone()),
            contents_base64: None,
        }],
        source_hint: Some("local server HTTP test".to_owned()),
        allow_finish: false,
        final_summary: None,
    };
    let arguments = serde_json::to_value(request)?
        .as_object()
        .cloned()
        .expect("request object");
    let created = client
        .call_tool(CallToolRequestParams::new("request_feedback").with_arguments(arguments))
        .await
        .context("call request_feedback")?;
    assert_ne!(created.is_error, Some(true));
    let created_content = created
        .structured_content
        .as_ref()
        .context("created structured content")?;
    assert!(
        serde_json::to_value(&created.content)?[0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("is waiting") && text.contains("use it to wait"))
    );
    assert_eq!(
        created_content
            .get("request_id")
            .and_then(serde_json::Value::as_str),
        Some(request_id.as_str())
    );
    let opened = application
        .get_feedback_workspace(request_id.clone())
        .await
        .context("open request attachment workspace")?;
    assert_eq!(opened.request_attachments.len(), 1);
    let attachment_bytes = application
        .read_request_attachment(
            request_id.clone(),
            opened.request_attachments[0].attachment_id.clone(),
        )
        .await
        .context("read request attachment")?;
    assert_eq!(attachment_bytes, review_markdown.as_bytes());
    let get_arguments = serde_json::json!({ "request_id": request_id })
        .as_object()
        .cloned()
        .expect("get arguments");
    let fetched = client
        .call_tool(CallToolRequestParams::new("get_feedback").with_arguments(get_arguments))
        .await
        .context("call get_feedback")?;
    assert_eq!(
        fetched
            .structured_content
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("waiting")
    );
    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: format!(
                "The real MCP client observes the completed package.\n\n{}\nEND-OF-FEEDBACK-MARKER",
                "middle-content-".repeat(200)
            ),
            expected_revision: 0,
        })
        .await
        .context("save operator draft")?;
    application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
        })
        .await
        .context("submit operator feedback")?;

    let completed_arguments = serde_json::json!({ "request_id": request_id })
        .as_object()
        .cloned()
        .expect("completed get arguments");
    let completed = client
        .call_tool(CallToolRequestParams::new("get_feedback").with_arguments(completed_arguments))
        .await
        .context("call completed get_feedback")?;
    let completed_content = completed
        .structured_content
        .as_ref()
        .context("completed structured content")?;
    assert_eq!(
        completed_content
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("completed")
    );
    let feedback = completed_content
        .get("feedback")
        .and_then(serde_json::Value::as_object)
        .context("completed feedback paths")?;
    for path in [
        "package_uri",
        "directory_path",
        "markdown_path",
        "manifest_path",
    ] {
        assert!(
            feedback
                .get(path)
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| !value.is_empty()),
            "missing {path}"
        );
    }
    let package = completed_content
        .get("feedback_package")
        .and_then(serde_json::Value::as_object)
        .context("completed feedback package")?;
    assert!(
        package
            .get("markdown")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|markdown| markdown.contains("real MCP client"))
    );
    assert!(package.get("manifest").is_some());
    let completed_text_json = serde_json::to_value(&completed.content)?;
    let completed_text = completed_text_json[0]["text"]
        .as_str()
        .expect("completed text");
    assert!(
        completed_text.contains("Feedback markdown:"),
        "completed text must name the feedback markdown path for text-only clients"
    );
    assert!(
        completed_text.contains("real MCP client"),
        "completed text must preview the feedback markdown"
    );
    assert!(
        completed_text.contains("preview truncated"),
        "long feedback must be truncated in text"
    );
    assert!(
        !completed_text.contains("END-OF-FEEDBACK-MARKER")
            && !completed_text.contains("MCP connection review"),
        "text must not inline the full feedback or manifest-only fields"
    );

    let final_request_id = uuid::Uuid::now_v7().to_string();
    let final_arguments = serde_json::to_value(RequestFeedbackInput {
        request_id: Some(final_request_id.clone()),
        host_id: Some("official-rust-sdk".to_owned()),
        host_session_id: "final-approval-session".to_owned(),
        title: Some("Approve final summary".to_owned()),
        what_happened: "The agent prepared its exact final summary.".to_owned(),
        actions: vec![ActionInput {
            id: "approve".to_owned(),
            instruction: "Approve the exact final summary.".to_owned(),
        }],
        context_refs: Vec::new(),
        attachments: Vec::new(),
        source_hint: None,
        allow_finish: true,
        final_summary: Some("Everything requested is complete.".to_owned()),
    })?
    .as_object()
    .cloned()
    .expect("final request arguments");
    let final_created = client
        .call_tool(CallToolRequestParams::new("request_feedback").with_arguments(final_arguments))
        .await
        .context("create final approval through MCP")?;
    assert_ne!(final_created.is_error, Some(true));
    application
        .approve_feedback(ApproveFeedbackInput {
            request_id: final_request_id.clone(),
        })
        .await
        .context("approve final summary as operator")?;
    let final_get_arguments = serde_json::json!({ "request_id": final_request_id })
        .as_object()
        .cloned()
        .expect("final get arguments");
    let approved = client
        .call_tool(CallToolRequestParams::new("get_feedback").with_arguments(final_get_arguments))
        .await
        .context("get approved final summary through MCP")?;
    let approved_content = approved
        .structured_content
        .as_ref()
        .context("approved structured content")?;
    assert_eq!(
        approved_content
            .get("resolution")
            .and_then(serde_json::Value::as_str),
        Some("approved")
    );
    assert!(approved_content.get("feedback_package").is_none());

    let invalid_arguments = serde_json::json!({ "request_id": "not-a-uuid" })
        .as_object()
        .cloned()
        .expect("invalid arguments");
    let invalid = client
        .call_tool(CallToolRequestParams::new("get_feedback").with_arguments(invalid_arguments))
        .await
        .context("call invalid get_feedback")?;
    assert_eq!(invalid.is_error, Some(true));
    assert!(
        serde_json::to_value(&invalid.content)?[0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("INVALID_ARGUMENT"))
    );
    assert_eq!(
        invalid
            .structured_content
            .as_ref()
            .and_then(|value| value.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("INVALID_ARGUMENT")
    );
    client.cancel().await?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn local_api_supports_pi_request_and_blocking_wait() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application.clone()).await?;
    let client = reqwest::Client::new();
    let api = format!("http://{}/api/feedback", server.address());
    let request_id = uuid::Uuid::now_v7().to_string();

    let request = RequestFeedbackInput {
        request_id: Some(request_id.clone()),
        host_id: Some("model-filled-host".to_owned()),
        host_session_id: "pi-tool-call".to_owned(),
        title: Some("Pi package wait".to_owned()),
        what_happened: "The Pi package direct API path was exercised.".to_owned(),
        actions: vec![ActionInput {
            id: "verify".to_owned(),
            instruction: "Submit feedback and resume the same Pi tool call.".to_owned(),
        }],
        context_refs: Vec::new(),
        attachments: Vec::new(),
        source_hint: Some("Pi API test".to_owned()),
        allow_finish: false,
        final_summary: None,
    };
    let created = client
        .post(format!("{api}/request"))
        .bearer_auth(TEST_TOKEN)
        .header(HOST_HEADER, "pi")
        .json(&request)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(created["request_id"], request_id);
    assert_eq!(created["status"], "waiting");
    assert_eq!(created["host_id"], "pi");

    let wait_client = client.clone();
    let wait_api = api.clone();
    let wait_request_id = request_id.clone();
    let waiter = tokio::spawn(async move {
        wait_client
            .post(format!("{wait_api}/wait"))
            .bearer_auth(TEST_TOKEN)
            .json(&serde_json::json!({ "request_id": wait_request_id }))
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await
    });

    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            body_markdown: "Pi waited inside the tool call and received this package.".to_owned(),
            expected_revision: 0,
        })
        .await
        .context("save operator draft")?;
    application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
        })
        .await
        .context("submit operator feedback")?;

    let completed = waiter.await??;
    assert_eq!(completed["request_id"], request_id);
    assert_eq!(completed["status"], "completed");
    assert_eq!(completed["execution_mode"], "wait");
    assert!(
        completed
            .pointer("/feedback_package/markdown")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|markdown| markdown.contains("Pi waited inside the tool call"))
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn request_without_host_id_defaults_to_generic() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application.clone()).await?;

    let config = StreamableHttpClientTransportConfig::with_uri(server.endpoint().to_owned())
        .auth_header(TEST_TOKEN);
    let transport = StreamableHttpClientTransport::from_config(config);
    let client = ClientInfo::default().serve(transport).await?;

    let arguments = serde_json::json!({
        "request_id": uuid::Uuid::now_v7().to_string(),
        "host_session_id": "host-id-optional-test".to_owned(),
        "what_happened": "No host_id was supplied; the server should default to generic.".to_owned(),
        "actions": [
            { "id": "ack", "instruction": "Acknowledge." }
        ],
    })
    .as_object()
    .cloned()
    .expect("arguments object");
    let created = client
        .peer()
        .call_tool(CallToolRequestParams::new("request_feedback").with_arguments(arguments))
        .await?;
    assert_ne!(created.is_error, Some(true));
    let created_content = created
        .structured_content
        .as_ref()
        .context("created structured content")?;
    assert_eq!(
        created_content
            .get("host_id")
            .and_then(serde_json::Value::as_str),
        Some("generic")
    );

    server.shutdown().await?;
    Ok(())
}
