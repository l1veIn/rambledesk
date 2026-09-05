use super::*;
use axum::{Json, Router, http::StatusCode, response::Redirect, routing::post};
use serde_json::json;

fn endpoint(url: &str) -> ManagedFeedbackEndpoint {
    ManagedFeedbackEndpoint {
        url: url.into(),
        bearer_token: "ab".repeat(32),
    }
}

#[test]
fn capabilities_are_literal_loopback_and_never_select_external_routes() {
    for url in [
        "http://127.0.0.1:4321/agent-feedback",
        "http://[::1]:4321/agent-feedback",
    ] {
        assert!(validate_endpoint(&endpoint(url)).is_ok());
    }
    for url in [
        "http://localhost:4321/agent-feedback",
        "https://127.0.0.1/agent-feedback",
        "http://127.0.0.1:0/agent-feedback",
        "http://127.0.0.1/feedback",
        "http://127.0.0.1/api/feedback",
        "http://127.0.0.1/mcp-managed",
        "http://user:pass@127.0.0.1/agent-feedback",
        "http://127.0.0.1/agent-feedback?token=abc",
        "http://127.0.0.1/agent-feedback#scope",
        "http://192.168.0.1/agent-feedback",
    ] {
        assert!(
            matches!(
                validate_endpoint(&endpoint(url)),
                Err(ClientError::InvalidCapability)
            ),
            "{url}"
        );
    }
    let mut invalid = endpoint("http://127.0.0.1/agent-feedback");
    invalid.bearer_token = "short".into();
    assert!(validate_endpoint(&invalid).is_err());
}

#[tokio::test]
async fn client_keeps_single_json_calls_and_never_follows_redirects() {
    let router = Router::new()
        .route(
            "/agent-feedback/request",
            post(
                |headers: axum::http::HeaderMap, Json(body): Json<Value>| async move {
                    assert_eq!(
                        headers["authorization"],
                        format!("Bearer {}", "ab".repeat(32))
                    );
                    assert_eq!(body["what_happened"], "Review 中文");
                    Json(json!({"request_id":"original","status":"waiting"}))
                },
            ),
        )
        .route(
            "/agent-feedback/get",
            post(|| async { Redirect::temporary("/must-not-follow") }),
        )
        .route(
            "/must-not-follow",
            post(|| async { Json(json!({"followed": true})) }),
        )
        .route(
            "/agent-feedback/recover",
            post(|| async { StatusCode::UNAUTHORIZED }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = endpoint(&format!(
        "http://{}/agent-feedback",
        listener.local_addr().unwrap()
    ));
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let (success, result) = call(
        &endpoint,
        "request",
        &json!({"what_happened":"Review 中文"}),
    )
    .await
    .unwrap();
    assert!(success);
    assert_eq!(result["request_id"], "original");
    assert_eq!(
        call(&endpoint, "get", &json!({})).await.unwrap_err(),
        ClientError::InvalidResponse
    );
    assert_eq!(
        call(&endpoint, "recover", &json!({})).await.unwrap_err(),
        ClientError::RevokedCapability
    );
    assert_eq!(
        call(&endpoint, "approve", &json!({})).await.unwrap_err(),
        ClientError::InvalidInput
    );
    task.abort();
}

#[test]
fn uncertain_failure_preserves_id_without_secrets_or_a_replacement_request() {
    let result = ClientError::UpstreamUnavailable.json(Some("original"));
    assert_eq!(result["request_id"], "original");
    assert_eq!(result["retryable"], true);
    assert!(result["message"].as_str().unwrap().contains("Recover"));
}
