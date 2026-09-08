use super::*;
use axum::{Json, Router, routing::post};

async fn server() -> (ManagedFeedbackEndpoint, JoinHandle<()>) {
    let router = Router::new().route(
        "/agent-feedback/{operation}",
        post(
            |headers: axum::http::HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(
                    headers["authorization"],
                    format!("Bearer {}", "ab".repeat(32))
                );
                Json(body)
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = ManagedFeedbackEndpoint {
        url: format!("http://{}/agent-feedback", listener.local_addr().unwrap()),
        bearer_token: "ab".repeat(32),
    };
    (
        endpoint,
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() }),
    )
}

#[tokio::test]
async fn receipt_requires_actual_handoff_and_resets_for_each_turn() {
    let (endpoint, server) = server().await;
    let channel = FeedbackChannel::start(endpoint).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let directory = std::path::Path::new(channel.address()).parent().unwrap();
        assert_eq!(
            std::fs::metadata(directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    for (operation, response, expected) in [
        (
            "request",
            json!({"request_id":"one", "status":"waiting"}),
            true,
        ),
        ("get", json!({"status":"waiting"}), true),
        ("recover", json!({"status":"in_progress"}), true),
        (
            "get",
            json!({"status":"submitted", "resolution":"feedback_submitted"}),
            false,
        ),
        (
            "get",
            json!({"status":"submitted", "resolution":"approved"}),
            true,
        ),
        ("recover", json!({"status":"cancelled"}), true),
        ("skip", json!({"reason":"user_opt_out"}), true),
        ("skip", json!({"reason":"task_finished"}), true),
        ("skip", json!({"reason":"request_cancelled"}), true),
        ("skip", json!({"reason":"answer_looks_done"}), false),
    ] {
        channel.begin_turn();
        assert!(!channel.receipt().attempted && !channel.receipt().handed_off);
        call(channel.address(), operation, &response).await.unwrap();
        assert!(channel.receipt().attempted);
        assert_eq!(
            channel.receipt().handed_off,
            expected,
            "{operation}: {response}"
        );
    }
    channel.close().await;
    assert!(
        call(channel.address(), "recover", &json!({}))
            .await
            .is_err()
    );
    server.abort();
}

#[tokio::test]
async fn stopped_channel_cannot_affect_a_sibling_or_replacement() {
    let (endpoint, server) = server().await;
    let first = FeedbackChannel::start(endpoint.clone()).unwrap();
    let second = FeedbackChannel::start(endpoint.clone()).unwrap();
    let old_address = first.address().to_owned();
    first.close().await;
    drop(first);
    let replacement = FeedbackChannel::start(endpoint).unwrap();
    assert_ne!(replacement.address(), old_address);
    assert!(
        call(&old_address, "skip", &json!({"reason":"user_opt_out"}))
            .await
            .is_err()
    );
    assert!(!replacement.receipt().handed_off);
    assert!(
        call(second.address(), "request", &json!({"status":"waiting"}))
            .await
            .unwrap()
            .0
    );
    assert!(second.receipt().handed_off);
    assert!(!replacement.receipt().handed_off);
    second.close().await;
    replacement.close().await;
    server.abort();
}

#[tokio::test]
async fn late_response_does_not_satisfy_a_new_turn() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (started, ready) = (entered.clone(), release.clone());
    let router = Router::new().route(
        "/agent-feedback/request",
        post(move || {
            let (started, ready) = (started.clone(), ready.clone());
            async move {
                started.notify_one();
                ready.notified().await;
                Json(json!({"status":"waiting"}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let channel = FeedbackChannel::start(ManagedFeedbackEndpoint {
        url: format!("http://{}/agent-feedback", listener.local_addr().unwrap()),
        bearer_token: "ab".repeat(32),
    })
    .unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let address = channel.address().to_owned();
    let request = tokio::spawn(async move { call(&address, "request", &json!({})).await.unwrap() });
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    assert!(channel.receipt().attempted);
    channel.begin_turn();
    release.notify_one();
    assert!(request.await.unwrap().0);
    assert!(!channel.receipt().attempted && !channel.receipt().handed_off);

    // Teardown also releases a command already waiting for its HTTP response.
    let address = channel.address().to_owned();
    let pending = tokio::spawn(async move { call(&address, "request", &json!({})).await });
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    channel.close().await;
    assert!(
        tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(!channel.receipt().handed_off);
    release.notify_one();
    server.abort();
}

#[tokio::test]
async fn oversized_and_truncated_frames_are_rejected_before_parsing() {
    let (mut writer, mut reader) = tokio::io::duplex(32);
    writer.write_u32(u32::MAX).await.unwrap();
    assert_eq!(
        read_frame(&mut reader, MAX_INPUT_BYTES).await.unwrap_err(),
        ClientError::InvalidInput
    );
    writer.write_u32(10).await.unwrap();
    writer.write_all(b"{}").await.unwrap();
    drop(writer);
    assert_eq!(
        read_frame(&mut reader, MAX_INPUT_BYTES).await.unwrap_err(),
        ClientError::InvalidResponse
    );
}
