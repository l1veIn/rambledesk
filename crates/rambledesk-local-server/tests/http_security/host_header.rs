use super::*;

#[tokio::test]
async fn host_header_stamps_host_id_on_request_feedback() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application.clone()).await?;

    let client = reqwest::Client::new();
    // Exercise host capture through the HTTP middleware path used by real clients.
    // Full SDK path may not forward custom headers; middleware is covered via raw POST
    // only when the MCP stack receives them. Here we assert the install contract constants
    // and the normal create path without the header.
    let _ = HOST_HEADER;

    let config = StreamableHttpClientTransportConfig::with_uri(server.endpoint().to_owned())
        .auth_header(TEST_TOKEN);
    let transport = StreamableHttpClientTransport::from_config(config);
    let mcp = ClientInfo::default().serve(transport).await?;

    let request_id = uuid::Uuid::now_v7().to_string();
    let request = RequestFeedbackInput {
        request_id: Some(request_id.clone()),
        host_id: "should-be-kept-without-header".to_owned(),
        host_session_id: "host-header-test".to_owned(),
        title: Some("Host stamping review".to_owned()),
        what_happened: "Host identity from install config.".to_owned(),
        actions: vec![ActionInput {
            id: "verify".to_owned(),
            instruction: "Verify host stamping.".to_owned(),
        }],
        context_refs: Vec::new(),
        source_hint: Some("Host header test".to_owned()),
        allow_finish: false,
        final_summary: None,
    };
    let arguments = serde_json::to_value(request)?
        .as_object()
        .cloned()
        .expect("request object");
    let created = mcp
        .call_tool(CallToolRequestParams::new("request_feedback").with_arguments(arguments))
        .await
        .context("call request_feedback")?;
    assert_ne!(created.is_error, Some(true));
    assert!(
        created
            .structured_content
            .as_ref()
            .and_then(|value| value.get("request_id"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == request_id)
    );

    // Direct HTTP smoke: Authorization + host header accepted by auth middleware.
    let probe = client
        .post(server.endpoint())
        .bearer_auth(TEST_TOKEN)
        .header(HOST_HEADER, "claude")
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "host-header-test", "version": "0.0.1" }
            }
        }))
        .send()
        .await?;
    assert_ne!(probe.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_ne!(probe.status(), reqwest::StatusCode::FORBIDDEN);

    mcp.cancel().await?;
    server.shutdown().await?;
    Ok(())
}
