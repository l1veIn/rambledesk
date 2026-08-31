use rambledesk_core::{ActionInput, RequestFeedbackInput, SaveDraftInput, SubmitFeedbackInput};
use rambledesk_local_server::{AccessToken, ServerConfig, start_server};

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

fn application_url(address: std::net::SocketAddr, operation: &str) -> String {
    format!("http://{address}/api/application/{operation}")
}

async fn seed_request(application: &rambledesk_core::FeedbackApplication) -> String {
    let request_id = uuid::Uuid::now_v7().to_string();
    application
        .request_feedback(RequestFeedbackInput {
            request_id: Some(request_id.clone()),
            host_id: Some("codex".into()),
            host_session_id: "application-api-session".into(),
            title: Some("Review the application API".into()),
            what_happened: "The HTTP transport needs shared read contracts.".into(),
            actions: vec![ActionInput {
                id: "verify".into(),
                instruction: "Verify the read projections.".into(),
            }],
            context_refs: vec![],
            attachments: vec![],
            source_hint: Some("application API test".into()),
            allow_finish: false,
            final_summary: None,
        })
        .await
        .expect("request should be created");
    request_id
}

#[tokio::test]
async fn application_routes_use_the_existing_bearer_middleware() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;

    let response = reqwest::Client::new()
        .post(application_url(server.address(), "listHostProfiles"))
        .send()
        .await?;

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn read_and_list_routes_use_shared_request_and_response_shapes() -> anyhow::Result<()> {
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let request_id = seed_request(&application).await;
    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;
    let client = reqwest::Client::new();

    let inbox = client
        .post(application_url(server.address(), "listFeedbackInbox"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(inbox[0]["request_id"], request_id);

    let sessions = client
        .post(application_url(server.address(), "listHostSessions"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(sessions[0]["host_id"], "codex");
    assert_eq!(sessions[0]["host_session_id"], "application-api-session");

    let archived = client
        .post(application_url(
            server.address(),
            "listArchivedHostSessions",
        ))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "search": null }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(archived, serde_json::json!([]));

    let profiles = client
        .post(application_url(server.address(), "listHostProfiles"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert!(
        profiles
            .as_array()
            .is_some_and(|profiles| { profiles.iter().any(|profile| profile["id"] == "codex") })
    );

    let requests = client
        .post(application_url(server.address(), "listFeedbackRequests"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "host_id": "codex",
            "host_session_id": "application-api-session",
            "status": ["waiting", "in_progress"],
            "archived": false,
            "search": null,
            "limit": 100,
            "cursor": null
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(requests["requests"][0]["request_id"], request_id);
    assert!(requests["next_cursor"].is_null());

    let workspace = client
        .post(application_url(server.address(), "getFeedbackWorkspace"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": request_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(workspace["request"]["host_id"], "codex");
    assert!(workspace.get("draft").is_some());
    assert!(workspace.get("attachments").is_some());

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn published_projection_hides_storage_paths_and_errors_stay_structured() -> anyhow::Result<()>
{
    let token = AccessToken::parse(TEST_TOKEN)?;
    let (application, _directory) = test_application().await?;
    let request_id = seed_request(&application).await;
    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            document_json: "{}".into(),
            body_markdown: "Operator feedback".into(),
            expected_revision: 0,
        })
        .await
        .expect("draft should save");
    application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .expect("feedback should publish");

    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;
    let client = reqwest::Client::new();
    let published = client
        .post(application_url(server.address(), "readPublishedFeedback"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": request_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert!(published.get("manifest").is_some());
    assert!(published.get("markdown").is_some());
    assert!(published.get("attachment_paths").is_none());
    assert!(published.get("request_attachment_paths").is_none());

    let invalid = client
        .post(application_url(server.address(), "getFeedbackWorkspace"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({ "request_id": "not-a-uuid" }))
        .send()
        .await?;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid.json::<serde_json::Value>().await?,
        serde_json::json!({
            "code": "INVALID_ARGUMENT",
            "message": "request_id must be a UUID",
            "retryable": false
        })
    );

    server.shutdown().await?;
    Ok(())
}
