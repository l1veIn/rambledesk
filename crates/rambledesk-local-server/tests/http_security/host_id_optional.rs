use super::*;

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
