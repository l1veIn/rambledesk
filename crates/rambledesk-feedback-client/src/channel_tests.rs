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
    assert_eq!(
        call(&old_address, "skip", &json!({"reason":"user_opt_out"}))
            .await
            .unwrap_err(),
        ClientError::RevokedCapability
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

#[test]
fn denied_ipc_is_actionable_without_changing_the_closed_channel_contract() {
    let error = connection_error(std::io::ErrorKind::PermissionDenied.into());
    let result = error.json(Some("original-request"));
    assert_eq!(result["code"], "ipc_access_denied");
    assert_eq!(result["request_id"], "original-request");
    assert_eq!(result["retryable"], false);
    assert_eq!(
        connection_error(std::io::ErrorKind::NotFound.into()),
        ClientError::RevokedCapability
    );
    assert_eq!(
        connection_error(std::io::ErrorKind::ConnectionRefused.into()),
        ClientError::RevokedCapability
    );
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

#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_sandbox_child_reports_ipc_access_denied() {
    let Ok(address) = std::env::var("RAMBLEDESK_TEST_SANDBOX_CHANNEL") else {
        return;
    };
    // Prove the kernel denied this live socket, rather than assuming every
    // failed connection means the owning Agent instance was stopped.
    let error = std::os::unix::net::UnixStream::connect(&address).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(1)); // EPERM from macOS Seatbelt
    let error = call(&address, "recover", &json!({})).await.unwrap_err();
    assert_eq!(error.json(None)["code"], "ipc_access_denied");
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_sandbox_denial_does_not_claim_a_live_capability_was_revoked() {
    let (endpoint, server) = server().await;
    let channel = FeedbackChannel::start(endpoint).unwrap();
    let input = json!({"reason":"user_opt_out"});
    assert!(call(channel.address(), "skip", &input).await.unwrap().0);
    channel.begin_turn();

    let mut command = tokio::process::Command::new("/usr/bin/sandbox-exec");
    command
        .args(["-p", "(version 1)(allow default)(deny network*)"])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "channel::tests::macos_sandbox_child_reports_ipc_access_denied",
            "--nocapture",
        ])
        .env("RAMBLEDESK_TEST_SANDBOX_CHANNEL", channel.address())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .unwrap()
        .unwrap();
    assert!(
        output.status.success(),
        "sandbox child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!channel.receipt().attempted);
    assert!(call(channel.address(), "skip", &input).await.unwrap().0);
    channel.close().await;
    server.abort();
}
