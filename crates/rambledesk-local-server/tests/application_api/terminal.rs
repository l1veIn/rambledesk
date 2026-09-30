use super::*;
use rambledesk_core::{
    CancelFeedbackInput, TerminalCommand, TerminalData, TerminalSessionSnapshot,
    TerminalSessionStatus, WorkbenchData, WorkbenchSpec,
};
use serde_json::{Value, json};
use std::time::Duration;

async fn seed_terminal_request(
    application: &rambledesk_core::FeedbackApplication,
    cwd: &std::path::Path,
) -> anyhow::Result<String> {
    Ok(application
        .request_feedback(RequestFeedbackInput {
            workbench: Some(WorkbenchSpec {
                kind: "terminal".into(),
                version: 1,
                data: WorkbenchData::Terminal(TerminalData {
                    cwd: cwd.to_string_lossy().into(),
                    shell: None,
                    commands: vec![TerminalCommand {
                        id: "help".into(),
                        title: "Inspect CLI".into(),
                        command: "example --help".into(),
                        description: None,
                    }],
                }),
            }),
            request_id: None,
            host_id: Some("codex".into()),
            host_session_id: "terminal-api-session".into(),
            title: Some("CLI trial".into()),
            what_happened: "Try this prepared CLI".into(),
            actions: vec![],
            context_refs: vec![],
            attachments: vec![],
            source_hint: None,
            allow_finish: false,
            final_summary: None,
        })
        .await?
        .request_id)
}

async fn call(
    client: &reqwest::Client,
    address: std::net::SocketAddr,
    operation: &str,
    input: Value,
) -> anyhow::Result<reqwest::Response> {
    Ok(client
        .post(application_url(address, operation))
        .bearer_auth(TEST_TOKEN)
        .header(
            rambledesk_local_server::RUNTIME_GENERATION_HEADER,
            "test-runtime",
        )
        .json(&input)
        .send()
        .await?)
}

async fn snapshot(
    client: &reqwest::Client,
    address: std::net::SocketAddr,
    operation: &str,
    input: Value,
) -> anyhow::Result<TerminalSessionSnapshot> {
    let response = call(client, address, operation, input).await?;
    assert_eq!(
        response.headers()[rambledesk_local_server::RUNTIME_GENERATION_HEADER],
        "test-runtime"
    );
    Ok(response.error_for_status()?.json().await?)
}

#[tokio::test]
async fn terminal_application_http_reconnects_resizes_and_cleans_up_host_cancellation()
-> anyhow::Result<()> {
    let (application, directory) = test_application().await?;
    let request = seed_terminal_request(&application, directory.path()).await?;
    let another = seed_terminal_request(&application, directory.path()).await?;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();
    let address = server.address();
    let open = json!({"request_id":request,"cols":80,"rows":24});
    assert_eq!(
        client
            .post(application_url(address, "openTerminalSession"))
            .json(&open)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let opened = snapshot(&client, address, "openTerminalSession", open.clone()).await?;
    assert_eq!(opened.request_id, request);
    assert_eq!(opened.cwd, directory.path().to_string_lossy());
    let session = &opened.session_id;
    let read = json!({"request_id":request,"session_id":session,"after_sequence":null});
    if cfg!(windows) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let found = snapshot(&client, address, "readTerminalSession", read.clone())
                    .await
                    .unwrap();
                if found.output.contains("\x1b[6n") {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;
        snapshot(
            &client,
            address,
            "writeTerminalSession",
            json!({"request_id":request,"session_id":session,"data":"\u{1b}[1;1R"}),
        )
        .await?;
    }
    let command = if cfg!(windows) {
        "set /a 33001+19\r"
    } else {
        "printf '%s\\n' $((33001+19))\r"
    };
    snapshot(
        &client,
        address,
        "writeTerminalSession",
        json!({"request_id":request,"session_id":session,"data":command}),
    )
    .await?;
    let latest = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let found = snapshot(&client, address, "readTerminalSession", read.clone())
                .await
                .unwrap();
            if found.output.contains("33020") {
                break found;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    assert!(
        !latest.output.contains("example --help"),
        "suggested commands must never autoexecute"
    );
    let reopened = snapshot(&client, address, "openTerminalSession", open).await?;
    assert_eq!(reopened.session_id, *session);
    assert!(reopened.output.contains("33020"));
    let delta = snapshot(
        &client,
        address,
        "readTerminalSession",
        json!({"request_id":request,"session_id":session,"after_sequence":latest.next_sequence}),
    )
    .await?;
    assert_eq!(delta.first_sequence, latest.next_sequence);
    let resized = snapshot(
        &client,
        address,
        "resizeTerminalSession",
        json!({"request_id":request,"session_id":session,"cols":100,"rows":30}),
    )
    .await?;
    assert_eq!((resized.cols, resized.rows), (100, 30));
    assert_eq!(
        call(
            &client,
            address,
            "readTerminalSession",
            json!({"request_id":another,"session_id":session,"after_sequence":null})
        )
        .await?
        .status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &client,
            address,
            "openTerminalSession",
            json!({"request_id":request,"cols":80,"rows":24,"cwd":"elsewhere"})
        )
        .await?
        .status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    // The requesting host bypasses the UI facade. Its terminal transition must
    // still shut down the very same PTY serving desktop and Web Access clients.
    application
        .cancel_feedback(CancelFeedbackInput {
            request_id: request.clone(),
            reason: "Host cancelled the trial".into(),
        })
        .await?;
    assert_eq!(
        call(
            &client,
            address,
            "writeTerminalSession",
            json!({"request_id":request,"session_id":session,"data":"secret-key-input"})
        )
        .await?
        .status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &client,
            address,
            "openTerminalSession",
            json!({"request_id":request,"cols":80,"rows":24})
        )
        .await?
        .status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    let stopped = snapshot(
        &client,
        address,
        "stopTerminalSession",
        json!({"request_id":request,"session_id":session}),
    )
    .await?;
    assert_eq!(stopped.status, TerminalSessionStatus::Stopped);
    assert!(stopped.exit_code.is_some());
    assert!(stopped.output.contains("33020"));
    assert!(!serde_json::to_string(&stopped)?.contains("secret-key-input"));
    assert!(
        !serde_json::to_value(&stopped)?
            .as_object()
            .unwrap()
            .contains_key("input")
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn terminal_commands_are_not_exposed_to_agent_integration_server() -> anyhow::Result<()> {
    let (application, directory) = test_application().await?;
    let request = seed_terminal_request(&application, directory.path()).await?;
    let token = AccessToken::parse(TEST_TOKEN)?;
    let server = start_server(ServerConfig::new(token).with_port(0), application).await?;
    let client = reqwest::Client::new();
    for operation in [
        "openTerminalSession",
        "readTerminalSession",
        "writeTerminalSession",
        "resizeTerminalSession",
        "stopTerminalSession",
    ] {
        assert_eq!(
            client
                .post(application_url(server.address(), operation))
                .bearer_auth(TEST_TOKEN)
                .json(&json!({"request_id":request,"cols":80,"rows":24}))
                .send()
                .await?
                .status(),
            reqwest::StatusCode::NOT_FOUND
        );
    }
    server.shutdown().await?;
    Ok(())
}
