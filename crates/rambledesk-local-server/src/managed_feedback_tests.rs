use super::*;
use std::time::Duration;

use rambledesk_core::{SessionManagement, SessionProtocol};
use rambledesk_storage::SqliteFeedbackStore;
use serde_json::json;

async fn provider() -> (
    tempfile::TempDir,
    SqliteFeedbackStore,
    Arc<LocalManagedFeedbackProvider>,
    ManagedFeedbackEndpoint,
) {
    let directory = tempfile::tempdir().unwrap();
    let store = SqliteFeedbackStore::connect(&directory.path().join("test.sqlite3"))
        .await
        .unwrap();
    let provider = Arc::new(LocalManagedFeedbackProvider::new(
        store.clone().into_application(),
    ));
    provider
        .configure(
            "127.0.0.1:37642".parse().unwrap(),
            vec![],
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let endpoint = provider
        .bind(&SessionRecord {
            session_id: "managed-a".into(),
            host_id: "fixture".into(),
            host_session_id: "managed-a".into(),
            title: "Fixture".into(),
            created_at: "2026-09-04T00:00:00Z".into(),
            updated_at: "2026-09-04T00:00:00Z".into(),
            management: SessionManagement::Managed {
                protocol: SessionProtocol::Acp,
                agent_config_id: "fixture".into(),
                cwd: directory.path().to_string_lossy().into_owned(),
                remote_session_id: None,
            },
        })
        .await
        .unwrap();
    (directory, store, provider, endpoint)
}

fn request(endpoint: &ManagedFeedbackEndpoint, body: Body, session: Option<&str>) -> Request<Body> {
    let mut request = Request::post("/mcp-managed")
        .header(HOST, "127.0.0.1:37642")
        .header(AUTHORIZATION, format!("Bearer {}", endpoint.bearer_token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream");
    if let Some(session) = session {
        request = request.header("Mcp-Session-Id", session);
    }
    request.body(body).unwrap()
}

#[tokio::test]
async fn revoke_does_not_wait_for_an_admitted_but_unfinished_initialize_body() {
    let (_directory, store, provider, endpoint) = provider().await;
    let binding = provider.bindings.read().await["managed-a"].clone();
    let (admitted, body_polled) = tokio::sync::oneshot::channel();
    let body = Body::from_stream(futures::stream::once(async move {
        let _ = admitted.send(());
        std::future::pending::<Result<axum::body::Bytes, std::io::Error>>().await
    }));
    let pending = tokio::spawn(handle_request(
        State(provider.clone()),
        request(&endpoint, body, None),
    ));
    body_polled.await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), provider.revoke("managed-a"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert!(!*binding.active.read().await);
    assert!(binding.sessions.sessions.read().await.is_empty());
    // The authenticated request cannot complete initialization after revocation.
    let denied = handle_request(
        State(provider.clone()),
        request(&endpoint, Body::empty(), None),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    provider.shutdown().await;
    store.close().await;
}

#[tokio::test]
async fn revoke_releases_initialized_workers_instead_of_leaving_the_manager_alive() {
    let (_directory, store, provider, endpoint) = provider().await;
    let binding = provider.bindings.read().await["managed-a"].clone();
    assert_eq!(binding.sessions.session_config.keep_alive, None);
    assert!(binding.sessions.session_config.init_timeout.is_some());
    let response = handle_request(State(provider.clone()), request(&endpoint, Body::from(json!({
        "jsonrpc":"2.0", "id":1, "method":"initialize", "params":{
            "protocolVersion":"2025-03-26", "capabilities":{}, "clientInfo":{"name":"fixture","version":"1"}
        }
    }).to_string()), None)).await;
    assert!(response.status().is_success());
    let session = response.headers()["Mcp-Session-Id"]
        .to_str()
        .unwrap()
        .to_owned();
    drop(response);
    let response = handle_request(
        State(provider.clone()),
        request(
            &endpoint,
            Body::from(
                json!({
                    "jsonrpc":"2.0", "method":"notifications/initialized"
                })
                .to_string(),
            ),
            Some(&session),
        ),
    )
    .await;
    assert!(response.status().is_success());
    drop(response);
    assert_eq!(binding.sessions.sessions.read().await.len(), 1);
    let manager = Arc::downgrade(&binding.sessions);
    provider.revoke("managed-a").await.unwrap();
    assert!(binding.sessions.sessions.read().await.is_empty());
    drop(binding);
    // rmcp's spawned session task owns the manager until its worker terminates.
    tokio::time::timeout(Duration::from_secs(1), async {
        while manager.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    provider.shutdown().await;
    store.close().await;
}
