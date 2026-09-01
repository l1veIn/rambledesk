use std::{collections::HashMap, sync::Arc};

use futures::StreamExt;
use rambledesk_core::{
    ApplicationChangeHub, ApplicationCommandFacade, WorkbenchTerminalOperations,
};
use rambledesk_local_server::{
    DurableWebAccessToken, SpaAsset, SpaAssetCachePolicy, SpaAssetSource, WebAccessServerConfig,
    WebSessionManager, WebSessionPolicy, start_web_access_server,
};
use rambledesk_storage::SqliteFeedbackStore;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::client::IntoClientRequest,
};

const DURABLE_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

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
