use super::*;

#[tokio::test]
async fn directory_browser_requires_application_auth_and_returns_host_directories()
-> anyhow::Result<()> {
    let (application, temp) = test_application().await?;
    let project = temp.path().join("project");
    std::fs::create_dir(&project)?;
    std::fs::create_dir(project.join("child"))?;
    std::fs::write(project.join("private.txt"), "not exposed")?;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();
    let url = application_url(server.address(), "browseProjectDirectories");
    let body = serde_json::json!({ "path": project });
    assert_eq!(
        client.post(&url).json(&body).send().await?.status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let response = client
        .post(&url)
        .bearer_auth(TEST_TOKEN)
        .json(&body)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response.headers()[rambledesk_local_server::RUNTIME_GENERATION_HEADER],
        "test-runtime"
    );
    let listing: rambledesk_core::ProjectDirectoryListing = response.json().await?;
    assert_eq!(listing.directories.len(), 1);
    assert_eq!(listing.directories[0].name, "child");
    assert!(std::path::Path::new(&listing.path).is_absolute());
    assert!(listing.parent_path.is_some());
    assert!(!listing.roots.is_empty());
    for (path, status, code) in [
        (
            "relative".to_owned(),
            reqwest::StatusCode::BAD_REQUEST,
            "INVALID_DIRECTORY_PATH",
        ),
        (
            project.join("missing").to_str().unwrap().into(),
            reqwest::StatusCode::NOT_FOUND,
            "DIRECTORY_NOT_FOUND",
        ),
    ] {
        let response = client
            .post(&url)
            .bearer_auth(TEST_TOKEN)
            .json(&serde_json::json!({ "path": path }))
            .send()
            .await?;
        assert_eq!(response.status(), status);
        assert_eq!(response.json::<serde_json::Value>().await?["code"], code);
    }
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn directory_browser_is_not_exposed_to_the_agent_integration_server() -> anyhow::Result<()> {
    let (application, _temp) = test_application().await?;
    let token = AccessToken::parse(TEST_TOKEN)?;
    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;
    let response = reqwest::Client::new()
        .post(application_url(
            server.address(),
            "browseProjectDirectories",
        ))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "path": null }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    server.shutdown().await?;
    Ok(())
}
