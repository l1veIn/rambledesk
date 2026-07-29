use std::collections::HashMap;

use anyhow::Context;
use rambledesk_core::{ActionInput, ProjectInput, RequestFeedbackInput};
use rambledesk_mcp::{AccessToken, ServerConfig, start_server};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientInfo},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};

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
    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;
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
    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;
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

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn official_client_exercises_health_feedback_and_errors() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;
    assert!(server.address().ip().is_loopback());

    let config = StreamableHttpClientTransportConfig::with_uri(server.endpoint().to_owned())
        .auth_header(TEST_TOKEN);
    let transport = StreamableHttpClientTransport::from_config(config);
    let client = ClientInfo::default().serve(transport).await?;

    let tools = client.peer().list_tools(Default::default()).await?;
    assert!(
        tools
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == "rambledesk_health")
    );
    for expected in ["request_feedback", "get_feedback", "cancel_feedback"] {
        assert!(
            tools
                .tools
                .iter()
                .any(|tool| tool.name.as_ref() == expected),
            "missing {expected}"
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
    assert!(!properties.contains_key("requestId"));

    let result = client
        .call_tool(
            CallToolRequestParams::new("rambledesk_health")
                .with_arguments(HashMap::new().into_iter().collect()),
        )
        .await
        .context("call rambledesk_health")?;
    assert_ne!(result.is_error, Some(true));

    let request_id = uuid::Uuid::now_v7().to_string();
    let request = RequestFeedbackInput {
        request_id: Some(request_id.clone()),
        agent: "official-rust-sdk".to_owned(),
        session_id: "http-security-test".to_owned(),
        project: ProjectInput {
            project_id: None,
            name: "RambleDesk MCP test".to_owned(),
            root_path: Some(directory.path().to_string_lossy().into_owned()),
        },
        what_happened: "The MCP feedback tools were connected.".to_owned(),
        actions: vec![ActionInput {
            id: "verify".to_owned(),
            instruction: "Verify the persisted feedback request.".to_owned(),
        }],
        context_refs: Vec::new(),
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
    assert!(
        serde_json::to_value(&created.content)?[0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("is waiting"))
    );
    assert_eq!(
        created
            .structured_content
            .as_ref()
            .and_then(|value| value.get("request_id"))
            .and_then(serde_json::Value::as_str),
        Some(request_id.as_str())
    );

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
