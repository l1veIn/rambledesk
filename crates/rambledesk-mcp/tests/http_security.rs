use std::collections::HashMap;

use anyhow::Context;
use rambledesk_mcp::{AccessToken, ServerConfig, start_server};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientInfo},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};

const TEST_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::test]
async fn rejects_missing_and_wrong_bearer_tokens() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let server = start_server(ServerConfig::new(token).with_port(0)).await?;
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
    let server = start_server(ServerConfig::new(token).with_port(0)).await?;
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
async fn official_client_lists_and_calls_health_tool() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let server = start_server(ServerConfig::new(token).with_port(0)).await?;
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

    let result = client
        .call_tool(
            CallToolRequestParams::new("rambledesk_health")
                .with_arguments(HashMap::new().into_iter().collect()),
        )
        .await
        .context("call rambledesk_health")?;
    assert_ne!(result.is_error, Some(true));

    client.cancel().await?;
    server.shutdown().await?;
    Ok(())
}
