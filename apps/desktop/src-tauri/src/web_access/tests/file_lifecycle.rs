use std::{sync::Arc, time::Duration};

use rambledesk_core::{
    ActionInput, ApplicationChangeHub, ApplicationCommandFacade, RequestFeedbackInput,
    SaveDraftInput, WorkbenchTerminalOperations,
};
use rambledesk_local_server::{
    DurableWebAccessToken, RUNTIME_GENERATION_HEADER, SpaAsset, SpaAssetCachePolicy,
    SpaAssetSource, WebAccessServerConfig, WebSessionManager, start_web_access_server,
};
use rambledesk_storage::SqliteFeedbackStore;
use reqwest::{Client, Response, StatusCode};
use serde_json::{Value, json};

use super::super::{
    ActiveWebAccess, WebAccessCredentialStore, WebAccessFailureCode, WebAccessLifecycle,
    WebAccessLifecycleState, WebAccessStatus, credential::FileWebAccessCredentialStore,
    web_access_start_failure,
};
use super::test_lifecycle;

// Only the static asset boundary is trivial here. Credentials, lifecycle,
// listener, HTTP authorization, application commands and SQLite are real.
struct FixtureAssets;

impl SpaAssetSource for FixtureAssets {
    fn load(&self, path: &str) -> Option<SpaAsset> {
        (path == "index.html").then(|| SpaAsset {
            bytes: b"<main>Web Access lifecycle fixture</main>".to_vec(),
            mime_type: "text/html".to_owned(),
            content_security_policy: None,
            cache_policy: SpaAssetCachePolicy::NoStore,
        })
    }
}

async fn start_fixture(
    lifecycle: &mut WebAccessLifecycle,
    credentials: &FileWebAccessCredentialStore,
    commands: Arc<ApplicationCommandFacade>,
    changes: Arc<ApplicationChangeHub>,
    port: u16,
) -> anyhow::Result<String> {
    let status = lifecycle
        .start(|| async {
            let token = credentials
                .load_or_create()
                .map_err(|_| WebAccessFailureCode::CredentialStoreUnavailable)?;
            let sessions = Arc::new(WebSessionManager::new(
                token.clone(),
                changes.metadata().runtime_generation,
            ));
            let listener = start_web_access_server(
                WebAccessServerConfig {
                    port,
                    ..Default::default()
                },
                commands,
                changes,
                sessions.clone(),
                Arc::new(FixtureAssets),
            )
            .await
            .map_err(|error| web_access_start_failure(&error))?;
            Ok(ActiveWebAccess::new(listener, sessions, token))
        })
        .await;
    assert_eq!(status.state, WebAccessLifecycleState::Running);
    lifecycle.active_url().await.map_err(anyhow::Error::msg)
}

async fn bootstrap(
    client: &Client,
    origin: &str,
    token: &DurableWebAccessToken,
) -> anyhow::Result<Response> {
    Ok(client
        .post(format!("{origin}/api/auth/session"))
        .header(reqwest::header::ORIGIN, origin)
        .bearer_auth(token.secret())
        .send()
        .await?)
}

async fn session_token(response: Response) -> anyhow::Result<String> {
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await?;
    Ok(body["session_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("bootstrap omitted its session credential"))?
        .to_owned())
}

async fn health(client: &Client, origin: &str, session: &str) -> anyhow::Result<StatusCode> {
    Ok(client
        .post(format!("{origin}/api/health"))
        .header(reqwest::header::ORIGIN, origin)
        .bearer_auth(session)
        .send()
        .await?
        .status())
}

#[tokio::test]
async fn file_credential_survives_listener_restart_and_rotation_preserves_the_saved_draft()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let credential_directory = directory.path().join("auth");
    let credentials = FileWebAccessCredentialStore::new(credential_directory.clone());
    let store = SqliteFeedbackStore::connect(&directory.path().join("feedback.sqlite3")).await?;
    let changes = Arc::new(ApplicationChangeHub::new());
    let application = store
        .clone()
        .into_application()
        .with_change_observer(changes.clone());
    let request_id = uuid::Uuid::now_v7().to_string();
    application
        .request_feedback(RequestFeedbackInput {
            request_id: Some(request_id.clone()),
            host_id: Some("test".into()),
            host_session_id: "file-credential-lifecycle".into(),
            title: Some("Credential restart fixture".into()),
            what_happened: "Retain the real saved draft across Web Access restart.".into(),
            actions: vec![ActionInput {
                id: "review".into(),
                instruction: "Review the saved draft.".into(),
            }],
            context_refs: vec![],
            attachments: vec![],
            source_hint: None,
            allow_finish: false,
            final_summary: None,
        })
        .await?;
    let commands = Arc::new(ApplicationCommandFacade::new(
        application.clone(),
        WorkbenchTerminalOperations::without_observer(application.clone()),
        vec![],
    ));
    let client = Client::builder().timeout(Duration::from_secs(5)).build()?;
    let mut lifecycle = test_lifecycle();
    let origin = start_fixture(
        &mut lifecycle,
        &credentials,
        commands.clone(),
        changes.clone(),
        0,
    )
    .await?;
    let port = reqwest::Url::parse(&origin)?.port().expect("bound port");
    let original_token = credentials.load_or_create().map_err(anyhow::Error::msg)?;
    let original_session =
        session_token(bootstrap(&client, &origin, &original_token).await?).await?;
    assert_eq!(
        health(&client, &origin, &original_session).await?,
        StatusCode::OK
    );

    let document = json!({
        "schemaVersion": 2,
        "doc": {"type": "doc", "content": [{"type": "paragraph", "content": [
            {"type": "text", "text": "Saved through HTTP before the listener restarted."}
        ]}]}
    })
    .to_string();
    let markdown = "Saved through HTTP before the listener restarted.";
    let saved = client
        .post(format!("{origin}/api/application/saveFeedbackDraft"))
        .header(reqwest::header::ORIGIN, &origin)
        .header(
            RUNTIME_GENERATION_HEADER,
            changes.metadata().runtime_generation,
        )
        .bearer_auth(&original_session)
        .json(&SaveDraftInput {
            request_id: request_id.clone(),
            expected_revision: 0,
            document_json: document.clone(),
            body_markdown: markdown.into(),
        })
        .send()
        .await?;
    assert_eq!(saved.status(), StatusCode::OK);
    assert_eq!(saved.json::<Value>().await?["saved_revision"], 1);

    assert_eq!(lifecycle.stop().await, WebAccessStatus::stopped());
    // A fresh store object reads the durable credential; the in-memory browser
    // session belongs to the old listener and must not survive its replacement.
    let reopened = FileWebAccessCredentialStore::new(credential_directory.clone());
    assert!(reopened.load_or_create().map_err(anyhow::Error::msg)? == original_token);
    let restarted_origin = start_fixture(
        &mut lifecycle,
        &reopened,
        commands.clone(),
        changes.clone(),
        port,
    )
    .await?;
    assert_eq!(restarted_origin, origin);
    assert_eq!(
        health(&client, &origin, &original_session).await?,
        StatusCode::UNAUTHORIZED
    );
    let replacement_session =
        session_token(bootstrap(&client, &origin, &original_token).await?).await?;
    assert_eq!(
        health(&client, &origin, &replacement_session).await?,
        StatusCode::OK
    );

    let workspace = client
        .post(format!("{origin}/api/application/getFeedbackWorkspace"))
        .header(reqwest::header::ORIGIN, &origin)
        .bearer_auth(&replacement_session)
        .json(&json!({"request_id": request_id}))
        .send()
        .await?;
    assert_eq!(workspace.status(), StatusCode::OK);
    let workspace: Value = workspace.json().await?;
    assert_eq!(workspace["draft"]["saved_revision"], 1);
    assert_eq!(workspace["draft"]["document_json"], document);
    assert_eq!(workspace["draft"]["body_markdown"], markdown);

    let rotated = reopened.rotate().map_err(anyhow::Error::msg)?;
    assert!(rotated != original_token);
    lifecycle.rotate_token(rotated.clone()).await;
    assert!(lifecycle.active_token().await.map_err(anyhow::Error::msg)? == rotated);
    let after_rotation = FileWebAccessCredentialStore::new(credential_directory);
    assert!(
        after_rotation
            .load_or_create()
            .map_err(anyhow::Error::msg)?
            == rotated
    );
    assert_eq!(
        health(&client, &origin, &replacement_session).await?,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        bootstrap(&client, &origin, &original_token).await?.status(),
        StatusCode::UNAUTHORIZED
    );
    let rotated_session = session_token(bootstrap(&client, &origin, &rotated).await?).await?;
    assert_eq!(
        health(&client, &origin, &rotated_session).await?,
        StatusCode::OK
    );

    assert_eq!(lifecycle.stop().await, WebAccessStatus::stopped());
    let persisted = application.get_feedback_workspace(request_id).await?;
    assert_eq!(persisted.draft.saved_revision, 1);
    assert_eq!(
        persisted.draft.document_json.as_deref(),
        Some(document.as_str())
    );
    assert_eq!(persisted.draft.body_markdown, markdown);
    store.close().await;
    Ok(())
}
