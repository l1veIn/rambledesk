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
    // Snapshots report the canonical directory without Windows' verbatim prefix.
    let canonical_cwd = directory.path().canonicalize()?;
    let expected_cwd = canonical_cwd.to_string_lossy();
    assert_eq!(
        opened.cwd,
        expected_cwd.strip_prefix(r"\\?\").unwrap_or(&expected_cwd)
    );
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
    let first_stopped = snapshot(
        &client,
        address,
        "stopTerminalSession",
        json!({"request_id":request,"session_id":session}),
    )
    .await?;
    assert_eq!(first_stopped.status, TerminalSessionStatus::Stopped);
    let restarted = snapshot(
        &client,
        address,
        "openTerminalSession",
        json!({"request_id":request,"cols":80,"rows":24}),
    )
    .await?;
    assert_ne!(restarted.session_id, *session);
    assert_eq!(restarted.status, TerminalSessionStatus::Running);
    let preserved = snapshot(&client, address, "readTerminalSession", read.clone()).await?;
    assert_eq!(preserved, first_stopped);
    assert!(preserved.output.contains("33020"));
    let session = &restarted.session_id;
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
    assert!(!stopped.output.contains("33020"));
    assert_eq!(
        snapshot(&client, address, "readTerminalSession", read).await?,
        preserved,
        "request cancellation must retain every previous session's evidence"
    );
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

#[tokio::test]
async fn terminal_restarted_sessions_freeze_together_and_completed_request_cannot_reopen()
-> anyhow::Result<()> {
    let (application, directory) = test_application().await?;
    let request = seed_terminal_request(&application, directory.path()).await?;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();
    let address = server.address();
    let mut sessions = Vec::new();
    for _ in 0..2 {
        let opened = snapshot(
            &client,
            address,
            "openTerminalSession",
            json!({"request_id":request,"cols":80,"rows":24}),
        )
        .await?;
        let stopped = snapshot(
            &client,
            address,
            "stopTerminalSession",
            json!({"request_id":request,"session_id":opened.session_id}),
        )
        .await?;
        assert_eq!(stopped.status, TerminalSessionStatus::Stopped);
        assert!(stopped.exit_code.is_some());
        sessions.push(json!({
            "id": stopped.session_id, "cwd": stopped.cwd, "shell": stopped.shell,
            "cols": stopped.cols, "rows": stopped.rows, "status": stopped.status,
            "exit_code": stopped.exit_code, "output": stopped.output,
            "screen": "", "truncated": stopped.truncated,
        }));
    }
    assert_ne!(sessions[0]["id"], sessions[1]["id"]);
    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request.clone(),
            document_json: json!({
                "schemaVersion":2, "doc":{"type":"doc","content":[]},
                "workbenchState":{"type":"terminal","sessions":sessions},
            })
            .to_string(),
            body_markdown: "Reviewed both CLI trials.".into(),
            expected_revision: 0,
        })
        .await?;
    let result = application
        .submit_feedback(SubmitFeedbackInput {
            request_id: request.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await?;
    let package = application.read_feedback_package(&result).await?.unwrap();
    let published = serde_json::to_value(package.manifest.workbench.unwrap())?;
    assert_eq!(published["result"]["sessions"], json!(sessions));
    assert_eq!(
        call(
            &client,
            address,
            "openTerminalSession",
            json!({"request_id":request,"cols":80,"rows":24}),
        )
        .await?
        .status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    for session in sessions {
        let retained = snapshot(
            &client,
            address,
            "readTerminalSession",
            json!({"request_id":request,"session_id":session["id"],"after_sequence":null}),
        )
        .await?;
        assert_eq!(retained.output, session["output"]);
        assert_eq!(
            retained.exit_code,
            serde_json::from_value::<Option<i32>>(session["exit_code"].clone())?
        );
    }
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn terminal_publication_checks_owned_runtime_and_rejects_running_drafts() -> anyhow::Result<()>
{
    let (application, directory) = test_application().await?;
    let request = seed_terminal_request(&application, directory.path()).await?;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();
    let address = server.address();
    let opened = snapshot(
        &client,
        address,
        "openTerminalSession",
        json!({"request_id":request,"cols":80,"rows":24}),
    )
    .await?;
    let session = json!({"id":opened.session_id,"cwd":opened.cwd,"shell":opened.shell,
        "cols":opened.cols,"rows":opened.rows,"status":"stopped","exit_code":null,
        "output":"client claims the process stopped","screen":"","truncated":false});
    let document = |session: Value| {
        json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},
        "workbenchState":{"type":"terminal","sessions":[session]}})
        .to_string()
    };
    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request.clone(),
            document_json: document(session.clone()),
            body_markdown: "Review the captured CLI trial.".into(),
            expected_revision: 0,
        })
        .await?;
    let submit = |revision| SubmitFeedbackInput {
        request_id: request.clone(),
        expected_revision: revision,
        cooked_markdown: None,
        cooking_model: None,
        uncooked_markdown: None,
    };
    let rejected = application
        .submit_feedback(submit(saved.saved_revision))
        .await
        .unwrap_err();
    assert!(
        rejected.message().contains("Stop the terminal"),
        "a client stopped flag must not prove OS ownership ended"
    );
    assert!(
        application
            .get_feedback_workspace(request.clone())
            .await?
            .feedback
            .is_none()
    );
    let mut running = session;
    running["status"] = json!("running");
    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request.clone(),
            document_json: document(running.clone()),
            body_markdown: "A running trial is a valid recoverable draft.".into(),
            expected_revision: saved.saved_revision,
        })
        .await?;
    let stopped = snapshot(
        &client,
        address,
        "stopTerminalSession",
        json!({"request_id":request,"session_id":opened.session_id}),
    )
    .await?;
    assert!(stopped.exit_code.is_some());
    assert!(!stopped.truncated);
    assert!(
        application
            .submit_feedback(submit(saved.saved_revision))
            .await
            .is_err(),
        "after actual cleanup, a running draft still cannot become a published result"
    );
    running["status"] = json!(stopped.status);
    running["exit_code"] = json!(stopped.exit_code);
    running["output"] = json!(stopped.output);
    let saved = application
        .save_feedback_draft(SaveDraftInput {
            request_id: request.clone(),
            document_json: document(running),
            body_markdown: "Finished reviewing the CLI trial.".into(),
            expected_revision: saved.saved_revision,
        })
        .await?;
    let published = application
        .submit_feedback(submit(saved.saved_revision))
        .await?;
    assert_eq!(published.status, rambledesk_core::FeedbackStatus::Completed);
    server.shutdown().await?;
    Ok(())
}
