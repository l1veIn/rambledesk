use std::{collections::HashMap, sync::Arc};

use futures::StreamExt;
use rambledesk_core::{
    ActionInput, ApplicationChangeHub, ApplicationCommandFacade, FeedbackApplication,
    GetFeedbackInput, RequestFeedbackInput, SaveDraftInput, WorkbenchTerminalOperations,
};
use rambledesk_local_server::{
    AccessToken, DurableWebAccessToken, MAX_APPLICATION_JSON_BODY_BYTES,
    MAX_ATTACHMENT_UPLOAD_BODY_BYTES, RUNTIME_GENERATION_HEADER, ServerConfig, SpaAsset,
    SpaAssetCachePolicy, SpaAssetSource, WebAccessServerConfig, WebSessionManager,
    WebSessionPolicy, start_server, start_web_access_server,
};
use rambledesk_storage::SqliteFeedbackStore;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WebSocketError, client::IntoClientRequest},
};
use uuid::Uuid;

const DURABLE_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const LOCAL_INTEGRATION_TOKEN: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const RUNTIME_GENERATION: &str = "runtime-a";

struct TestAssets {
    assets: HashMap<String, SpaAsset>,
}

impl SpaAssetSource for TestAssets {
    fn load(&self, path: &str) -> Option<SpaAsset> {
        self.assets.get(path).cloned()
    }
}

fn asset(bytes: &str, mime_type: &str, cache_policy: SpaAssetCachePolicy) -> SpaAsset {
    SpaAsset {
        bytes: bytes.as_bytes().to_vec(),
        mime_type: mime_type.into(),
        content_security_policy: None,
        cache_policy,
    }
}

fn test_assets() -> Arc<TestAssets> {
    Arc::new(TestAssets {
        assets: HashMap::from([(
            "index.html".into(),
            asset(
                "<main>Workbench</main>",
                "text/html; charset=utf-8",
                SpaAssetCachePolicy::NoStore,
            ),
        )]),
    })
}

fn test_request(request_id: String, host_session_id: &str) -> RequestFeedbackInput {
    RequestFeedbackInput {
        request_id: Some(request_id),
        host_id: Some("codex".into()),
        host_session_id: host_session_id.into(),
        title: Some("Web Access acceptance".into()),
        what_happened: "Exercise the Web Access security and lifecycle contract.".into(),
        actions: vec![ActionInput {
            id: "verify".into(),
            instruction: "Verify the transport boundary.".into(),
        }],
        context_refs: vec![],
        attachments: vec![],
        source_hint: Some("web access acceptance test".into()),
        allow_finish: false,
        final_summary: None,
    }
}

fn test_commands(application: FeedbackApplication) -> Arc<ApplicationCommandFacade> {
    Arc::new(ApplicationCommandFacade::new(
        application.clone(),
        WorkbenchTerminalOperations::without_observer(application),
        vec![],
    ))
}

async fn bootstrap_session(
    client: &reqwest::Client,
    server: &rambledesk_local_server::WebAccessServerHandle,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{}/api/auth/session", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth(DURABLE_TOKEN)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let payload = response.json::<serde_json::Value>().await?;
    Ok(payload["session_token"]
        .as_str()
        .expect("bootstrap session token")
        .to_owned())
}

fn authorized_application_post(
    client: &reqwest::Client,
    server: &rambledesk_local_server::WebAccessServerHandle,
    session_token: &str,
    operation: &str,
) -> reqwest::RequestBuilder {
    client
        .post(format!("{}/api/application/{operation}", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth(session_token)
}

async fn connect_events(
    server: &rambledesk_local_server::WebAccessServerHandle,
    session_token: &str,
) -> anyhow::Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
    let mut request = format!("ws://{}/api/events", server.address()).into_client_request()?;
    request
        .headers_mut()
        .insert("Origin", server.origin().parse()?);
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("rambledesk-events, rambledesk-session.{session_token}").parse()?,
    );
    Ok(connect_async(request).await?.0)
}

async fn event_connection_status(
    server: &rambledesk_local_server::WebAccessServerHandle,
    session_token: &str,
) -> anyhow::Result<Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, u16>> {
    let mut request = format!("ws://{}/api/events", server.address()).into_client_request()?;
    request
        .headers_mut()
        .insert("Origin", server.origin().parse()?);
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("rambledesk-events, rambledesk-session.{session_token}").parse()?,
    );
    match connect_async(request).await {
        Ok((socket, _response)) => Ok(Ok(socket)),
        Err(WebSocketError::Http(response)) => Ok(Err(response.status().as_u16())),
        Err(error) => Err(error.into()),
    }
}

#[tokio::test]
async fn independent_server_serves_spa_history_and_fingerprinted_assets() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = SqliteFeedbackStore::connect(&directory.path().join("state.sqlite3")).await?;
    let changes = Arc::new(ApplicationChangeHub::with_runtime_generation("runtime-a"));
    let application = store
        .into_application()
        .with_change_observer(changes.clone());
    let commands = Arc::new(ApplicationCommandFacade::new(
        application.clone(),
        WorkbenchTerminalOperations::without_observer(application),
        vec![],
    ));
    let assets = Arc::new(TestAssets {
        assets: HashMap::from([
            (
                "index.html".into(),
                asset(
                    "<main>Workbench</main>",
                    "text/html; charset=utf-8",
                    SpaAssetCachePolicy::NoStore,
                ),
            ),
            (
                "assets/app-1234abcd.js".into(),
                asset(
                    "export const ready = true",
                    "text/javascript",
                    SpaAssetCachePolicy::Immutable,
                ),
            ),
            (
                "browser-speech/runtime/sherpa-onnx-wasm-web.wasm".into(),
                asset("wasm", "application/wasm", SpaAssetCachePolicy::NoCache),
            ),
        ]),
    });
    let sessions = Arc::new(WebSessionManager::with_policy(
        DurableWebAccessToken::parse(DURABLE_TOKEN)?,
        "runtime-a",
        WebSessionPolicy {
            idle_timeout_seconds: 1,
            absolute_timeout_seconds: 1,
            max_sessions: 4,
        },
    ));
    let server = start_web_access_server(
        WebAccessServerConfig {
            port: 0,
            max_event_connections: 2,
            max_http_requests: 8,
            max_bootstrap_attempts_per_minute: 5,
        },
        commands.clone(),
        changes,
        sessions,
        assets,
    )
    .await?;
    let client = reqwest::Client::new();

    for (label, request, expected) in [
        (
            "missing Origin",
            client
                .post(format!("{}/api/auth/session", server.origin()))
                .bearer_auth(DURABLE_TOKEN),
            reqwest::StatusCode::FORBIDDEN,
        ),
        (
            "wrong Origin",
            client
                .post(format!("{}/api/auth/session", server.origin()))
                .header(reqwest::header::ORIGIN, "http://attacker.example")
                .bearer_auth(DURABLE_TOKEN),
            reqwest::StatusCode::FORBIDDEN,
        ),
        (
            "wrong Host",
            client
                .post(format!("{}/api/auth/session", server.origin()))
                .header(reqwest::header::HOST, "attacker.example")
                .header(reqwest::header::ORIGIN, server.origin())
                .bearer_auth(DURABLE_TOKEN),
            reqwest::StatusCode::FORBIDDEN,
        ),
        (
            "missing bearer",
            client
                .post(format!("{}/api/auth/session", server.origin()))
                .header(reqwest::header::ORIGIN, server.origin()),
            reqwest::StatusCode::UNAUTHORIZED,
        ),
    ] {
        let response = request.send().await?;
        assert_eq!(response.status(), expected, "{label}");
    }

    let rejected = client
        .post(format!("{}/api/auth/session", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth("wrong-token")
        .send()
        .await?;
    assert_eq!(rejected.status(), reqwest::StatusCode::UNAUTHORIZED);

    let bootstrap = client
        .post(format!("{}/api/auth/session", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth(DURABLE_TOKEN)
        .send()
        .await?;
    assert_eq!(bootstrap.status(), reqwest::StatusCode::OK);
    assert_eq!(
        bootstrap.headers()[reqwest::header::CACHE_CONTROL],
        "no-store"
    );
    let bootstrap_body = bootstrap.text().await?;
    assert!(
        !bootstrap_body.trim().is_empty(),
        "bootstrap response body: {bootstrap_body:?}"
    );
    let session_token =
        serde_json::from_str::<serde_json::Value>(&bootstrap_body)?["session_token"]
            .as_str()
            .expect("session token")
            .to_owned();

    for path in ["/", "/sessions/codex/example"] {
        let response = client
            .get(format!("{}{path}", server.origin()))
            .header(reqwest::header::ACCEPT, "text/html")
            .send()
            .await?;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response.headers()[reqwest::header::CACHE_CONTROL],
            "no-store"
        );
        let csp = response.headers()[reqwest::header::CONTENT_SECURITY_POLICY].to_str()?;
        assert!(csp.contains("script-src 'self' 'wasm-unsafe-eval'"));
        assert!(csp.contains("worker-src 'self'"));
        assert!(csp.contains("connect-src 'self' ws://"));
        assert!(csp.contains("https://www.modelscope.cn"));
        assert!(csp.contains("https://cdn-lfs-cn-1.modelscope.cn"));
        assert!(!csp.contains("*.modelscope.cn"));
        assert!(!csp.contains("'unsafe-eval'"));
        assert_eq!(response.text().await?, "<main>Workbench</main>");
    }

    let asset = client
        .get(format!("{}/assets/app-1234abcd.js", server.origin()))
        .send()
        .await?;
    assert_eq!(asset.status(), reqwest::StatusCode::OK);
    assert_eq!(
        asset.headers()[reqwest::header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(asset.headers()["x-content-type-options"], "nosniff");
    assert_eq!(asset.headers()["x-frame-options"], "DENY");

    let wasm = client
        .head(format!(
            "{}/browser-speech/runtime/sherpa-onnx-wasm-web.wasm",
            server.origin()
        ))
        .send()
        .await?;
    assert_eq!(wasm.status(), reqwest::StatusCode::OK);
    assert_eq!(
        wasm.headers()[reqwest::header::CONTENT_TYPE],
        "application/wasm"
    );
    assert_eq!(wasm.headers()["x-content-type-options"], "nosniff");

    for path in ["/assets/missing", "/missing.js", "/a%2Fb"] {
        let response = client
            .get(format!("{}{path}", server.origin()))
            .header(reqwest::header::ACCEPT, "text/html")
            .send()
            .await?;
        assert_ne!(response.status(), reqwest::StatusCode::OK, "{path}");
        assert_ne!(response.text().await?, "<main>Workbench</main>", "{path}");
    }

    let non_navigation = client
        .get(format!("{}/sessions/codex/example", server.origin()))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await?;
    assert_eq!(non_navigation.status(), reqwest::StatusCode::NOT_FOUND);

    let api_miss = client
        .post(format!("{}/api/not-an-operation", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth(session_token)
        .send()
        .await?;
    assert_eq!(api_miss.status(), reqwest::StatusCode::NOT_FOUND);
    assert_ne!(api_miss.text().await?, "<main>Workbench</main>");

    let wrong_host = client
        .get(server.origin())
        .header(reqwest::header::HOST, "attacker.example")
        .send()
        .await?;
    assert_eq!(wrong_host.status(), reqwest::StatusCode::FORBIDDEN);

    let fresh_session = client
        .post(format!("{}/api/auth/session", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth(DURABLE_TOKEN)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?["session_token"]
        .as_str()
        .expect("fresh session")
        .to_owned();
    let mut socket = connect_events(&server, &fresh_session).await?;
    assert!(socket.next().await.is_some(), "ready frame");
    let closed = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next()).await?;
    assert!(
        closed.is_none() || closed.is_some_and(|frame| frame.is_ok_and(|frame| frame.is_close())),
        "expired Web Session must close its existing event socket"
    );

    let shutdown_session = client
        .post(format!("{}/api/auth/session", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth(DURABLE_TOKEN)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?["session_token"]
        .as_str()
        .expect("shutdown session")
        .to_owned();
    let mut shutdown_socket = connect_events(&server, &shutdown_session).await?;
    assert!(shutdown_socket.next().await.is_some(), "ready frame");
    let rate_limited = client
        .post(format!("{}/api/auth/session", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth(DURABLE_TOKEN)
        .send()
        .await?;
    assert_eq!(
        rate_limited.status(),
        reqwest::StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(rate_limited.headers()[reqwest::header::RETRY_AFTER], "60");
    server.cancel();
    let closed =
        tokio::time::timeout(std::time::Duration::from_secs(1), shutdown_socket.next()).await?;
    assert!(
        closed.is_none() || closed.is_some_and(|frame| frame.is_ok_and(|frame| frame.is_close())),
        "server lifecycle cancellation must close upgraded event sockets"
    );
    commands.list_feedback_inbox().await?;
    server.shutdown().await?;
    Ok(())
}

#[test]
fn default_security_limits_are_a_read_only_snapshot_of_runtime_defaults() {
    let config = WebAccessServerConfig::default();
    let limits = config.security_limits();
    let sessions = WebSessionPolicy::default();

    assert_eq!(limits.loopback_address, std::net::Ipv4Addr::LOCALHOST);
    assert_eq!(limits.port, config.port);
    assert_eq!(
        limits.max_bootstrap_attempts_per_minute,
        config.max_bootstrap_attempts_per_minute
    );
    assert_eq!(limits.max_http_requests, config.max_http_requests);
    assert_eq!(limits.max_event_connections, config.max_event_connections);
    assert_eq!(limits.max_json_body_bytes, MAX_APPLICATION_JSON_BODY_BYTES);
    assert_eq!(
        limits.max_attachment_upload_body_bytes,
        MAX_ATTACHMENT_UPLOAD_BODY_BYTES
    );
    assert_eq!(
        limits.session_idle_timeout_seconds,
        sessions.idle_timeout_seconds
    );
    assert_eq!(
        limits.session_absolute_timeout_seconds,
        sessions.absolute_timeout_seconds
    );
    assert_eq!(limits.max_sessions, sessions.max_sessions);
}

#[tokio::test]
async fn authenticated_event_connection_budget_rejects_then_recovers() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = SqliteFeedbackStore::connect(&directory.path().join("events.sqlite3")).await?;
    let changes = Arc::new(ApplicationChangeHub::with_runtime_generation(
        RUNTIME_GENERATION,
    ));
    let application = store
        .into_application()
        .with_change_observer(changes.clone());
    let sessions = Arc::new(WebSessionManager::new(
        DurableWebAccessToken::parse(DURABLE_TOKEN)?,
        RUNTIME_GENERATION,
    ));
    let server = start_web_access_server(
        WebAccessServerConfig {
            port: 0,
            max_event_connections: 1,
            max_http_requests: 8,
            max_bootstrap_attempts_per_minute: 8,
        },
        test_commands(application),
        changes,
        sessions,
        test_assets(),
    )
    .await?;
    let client = reqwest::Client::new();
    let session_token = bootstrap_session(&client, &server).await?;

    let mut first = connect_events(&server, &session_token).await?;
    assert!(
        first.next().await.is_some(),
        "first socket must receive ready"
    );
    let rejected = event_connection_status(&server, &session_token).await?;
    assert!(
        matches!(
            rejected,
            Err(status) if status == reqwest::StatusCode::SERVICE_UNAVAILABLE.as_u16()
        ),
        "the authenticated connection above the fixed budget must be rejected"
    );

    first.close(None).await?;
    let mut recovered = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match event_connection_status(&server, &session_token).await? {
                Ok(socket) => break Ok::<_, anyhow::Error>(socket),
                Err(status) if status == reqwest::StatusCode::SERVICE_UNAVAILABLE.as_u16() => {
                    tokio::task::yield_now().await;
                }
                Err(status) => anyhow::bail!("unexpected event connection status {status}"),
            }
        }
    })
    .await??;
    assert!(
        recovered.next().await.is_some(),
        "released event budget must admit a replacement socket"
    );
    recovered.close(None).await?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn authenticated_body_limits_reject_before_draft_or_attachment_mutation() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = SqliteFeedbackStore::connect(&directory.path().join("body-limits.sqlite3")).await?;
    let changes = Arc::new(ApplicationChangeHub::with_runtime_generation(
        RUNTIME_GENERATION,
    ));
    let application = store
        .into_application()
        .with_change_observer(changes.clone());
    let request_id = Uuid::now_v7().to_string();
    application
        .request_feedback(test_request(request_id.clone(), "body-limits"))
        .await?;
    let sessions = Arc::new(WebSessionManager::new(
        DurableWebAccessToken::parse(DURABLE_TOKEN)?,
        RUNTIME_GENERATION,
    ));
    let server = start_web_access_server(
        WebAccessServerConfig {
            port: 0,
            ..WebAccessServerConfig::default()
        },
        test_commands(application.clone()),
        changes.clone(),
        sessions,
        test_assets(),
    )
    .await?;
    let client = reqwest::Client::new();
    let session_token = bootstrap_session(&client, &server).await?;
    let before = changes.metadata();

    let oversized_draft =
        authorized_application_post(&client, &server, &session_token, "saveFeedbackDraft")
            .header(RUNTIME_GENERATION_HEADER, RUNTIME_GENERATION)
            .json(&SaveDraftInput {
                request_id: request_id.clone(),
                document_json: r#"{"type":"doc","content":[]}"#.into(),
                body_markdown: "x".repeat(MAX_APPLICATION_JSON_BODY_BYTES + 1),
                expected_revision: 0,
            })
            .send()
            .await?;
    assert_eq!(
        oversized_draft.status(),
        reqwest::StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(changes.metadata(), before);
    let workspace = application
        .get_feedback_workspace(request_id.clone())
        .await?;
    assert_eq!(workspace.draft.saved_revision, 0);
    assert!(workspace.draft.body_markdown.is_empty());

    let oversized_attachment =
        authorized_application_post(&client, &server, &session_token, "addFeedbackAttachment")
            .header(RUNTIME_GENERATION_HEADER, RUNTIME_GENERATION)
            .multipart(
                reqwest::multipart::Form::new()
                    .text("request_id", request_id.clone())
                    .text("file_name", "oversized.bin")
                    .text("expected_revision", "0")
                    .part(
                        "file",
                        reqwest::multipart::Part::bytes(vec![
                            0;
                            MAX_ATTACHMENT_UPLOAD_BODY_BYTES + 1
                        ]),
                    ),
            )
            .send()
            .await?;
    assert_eq!(
        oversized_attachment.status(),
        reqwest::StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(changes.metadata(), before);
    let workspace = application.get_feedback_workspace(request_id).await?;
    assert!(workspace.attachments.is_empty());
    assert_eq!(workspace.draft.saved_revision, 0);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn stopping_web_access_leaves_same_application_local_integration_writable()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = SqliteFeedbackStore::connect(&directory.path().join("shared.sqlite3")).await?;
    let changes = Arc::new(ApplicationChangeHub::with_runtime_generation(
        RUNTIME_GENERATION,
    ));
    let application = store
        .into_application()
        .with_change_observer(changes.clone());
    let local = start_server(
        ServerConfig::new(AccessToken::parse(LOCAL_INTEGRATION_TOKEN)?).with_port(0),
        application.clone(),
    )
    .await?;
    let web = start_web_access_server(
        WebAccessServerConfig {
            port: 0,
            ..WebAccessServerConfig::default()
        },
        test_commands(application.clone()),
        changes,
        Arc::new(WebSessionManager::new(
            DurableWebAccessToken::parse(DURABLE_TOKEN)?,
            RUNTIME_GENERATION,
        )),
        test_assets(),
    )
    .await?;
    let client = reqwest::Client::new();
    let session_token = bootstrap_session(&client, &web).await?;

    let first_id = Uuid::now_v7().to_string();
    let first_write = client
        .post(format!("http://{}/api/feedback/request", local.address()))
        .bearer_auth(LOCAL_INTEGRATION_TOKEN)
        .json(&test_request(first_id.clone(), "shared-before-stop"))
        .send()
        .await?;
    assert_eq!(first_write.status(), reqwest::StatusCode::OK);
    let web_inbox = authorized_application_post(&client, &web, &session_token, "listFeedbackInbox")
        .send()
        .await?;
    assert_eq!(web_inbox.status(), reqwest::StatusCode::OK);
    assert!(
        web_inbox
            .json::<serde_json::Value>()
            .await?
            .as_array()
            .is_some_and(|requests| requests
                .iter()
                .any(|request| request["request_id"] == first_id))
    );

    web.shutdown().await?;

    let second_id = Uuid::now_v7().to_string();
    let second_write = client
        .post(format!("http://{}/api/feedback/request", local.address()))
        .bearer_auth(LOCAL_INTEGRATION_TOKEN)
        .json(&test_request(second_id.clone(), "shared-after-stop"))
        .send()
        .await?;
    assert_eq!(second_write.status(), reqwest::StatusCode::OK);
    assert_eq!(
        second_write.json::<serde_json::Value>().await?["request_id"],
        second_id
    );
    let direct = application
        .get_feedback(GetFeedbackInput {
            request_id: second_id,
        })
        .await?;
    assert_eq!(direct.status, rambledesk_core::FeedbackStatus::Waiting);

    local.shutdown().await?;
    Ok(())
}
