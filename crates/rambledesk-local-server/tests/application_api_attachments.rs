use rambledesk_core::{
    ActionInput, MAX_ATTACHMENT_BYTES, ReorderAttachmentsInput, RequestAttachmentInput,
    RequestFeedbackInput, WorkbenchTerminalOperations,
};
mod application_api_support;
use application_api_support::start_application_server;

const TEST_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REQUEST_ATTACHMENT_BYTES: &[u8] = b"# Request context\n\nInspect the transport contract.";

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

fn terminal_operations(
    application: &rambledesk_core::FeedbackApplication,
) -> WorkbenchTerminalOperations {
    WorkbenchTerminalOperations::without_observer(application.clone())
}

async fn seed_request(application: &rambledesk_core::FeedbackApplication) -> String {
    let request_id = uuid::Uuid::now_v7().to_string();
    application
        .request_feedback(RequestFeedbackInput {
            request_id: Some(request_id.clone()),
            host_id: Some("codex".into()),
            host_session_id: "application-api-attachment-session".into(),
            title: Some("Review attachment transport".into()),
            what_happened: "The HTTP transport needs attachment contracts.".into(),
            actions: vec![ActionInput {
                id: "verify".into(),
                instruction: "Verify attachment upload and download.".into(),
            }],
            context_refs: vec![],
            attachments: vec![RequestAttachmentInput {
                file_name: "request-context.md".into(),
                markdown: Some(String::from_utf8_lossy(REQUEST_ATTACHMENT_BYTES).into_owned()),
                contents_base64: None,
                path: None,
            }],
            source_hint: Some("application API attachment test".into()),
            allow_finish: false,
            final_summary: None,
        })
        .await
        .expect("request should be created");
    request_id
}

fn assert_no_path_keys(value: &serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                assert_no_path_keys(value);
            }
        }
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                assert!(!key.contains("path"), "storage path key leaked: {key}");
                assert_no_path_keys(value);
            }
        }
        _ => {}
    }
}

fn assert_attachment_response_headers(response: &reqwest::Response) {
    assert_eq!(
        response.headers().get(reqwest::header::CONTENT_TYPE),
        Some(&reqwest::header::HeaderValue::from_static(
            "application/octet-stream"
        ))
    );
    assert_eq!(
        response.headers().get(reqwest::header::CACHE_CONTROL),
        Some(&reqwest::header::HeaderValue::from_static("no-store"))
    );
    assert_eq!(
        response.headers().get("x-content-type-options"),
        Some(&reqwest::header::HeaderValue::from_static("nosniff"))
    );
}

#[tokio::test]
async fn attachment_routes_stream_bytes_preserve_dtos_and_enforce_cas() -> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let request_id = seed_request(&application).await;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;
    let client = reqwest::Client::new();
    let first_contents = b"first attachment bytes".to_vec();

    let first = client
        .post(application_url(server.address(), "addFeedbackAttachment"))
        .bearer_auth(TEST_TOKEN)
        .multipart(
            reqwest::multipart::Form::new()
                .text("request_id", request_id.clone())
                .text("file_name", "first.txt")
                .text("expected_revision", "0")
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(first_contents.clone())
                        .file_name("ignored-wire-name.txt"),
                ),
        )
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_no_path_keys(&first);
    assert_eq!(first["attachments"][0]["file_name"], "first.txt");
    assert_eq!(first["draft"]["saved_revision"], 1);
    let first_attachment_id = first["attachments"][0]["attachment_id"]
        .as_str()
        .expect("attachment id")
        .to_owned();
    let request_attachment_id = first["request_attachments"][0]["attachment_id"]
        .as_str()
        .expect("request attachment id")
        .to_owned();

    let direct_workspace = application
        .get_feedback_workspace(request_id.clone())
        .await?;
    assert_eq!(first, serde_json::to_value(direct_workspace)?);

    let feedback_download = client
        .post(application_url(server.address(), "readFeedbackAttachment"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": request_id,
            "attachment_id": first_attachment_id
        }))
        .send()
        .await?
        .error_for_status()?;
    assert_attachment_response_headers(&feedback_download);
    assert_eq!(feedback_download.bytes().await?.as_ref(), first_contents);

    let request_download = client
        .post(application_url(server.address(), "readRequestAttachment"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": request_id,
            "attachment_id": request_attachment_id
        }))
        .send()
        .await?
        .error_for_status()?;
    assert_attachment_response_headers(&request_download);
    assert_eq!(
        request_download.bytes().await?.as_ref(),
        REQUEST_ATTACHMENT_BYTES
    );

    let stale_input = ReorderAttachmentsInput {
        request_id: request_id.clone(),
        attachment_ids: vec![first_attachment_id.clone()],
        expected_revision: 0,
    };
    let stale_http = client
        .post(application_url(
            server.address(),
            "reorderFeedbackAttachments",
        ))
        .bearer_auth(TEST_TOKEN)
        .json(&stale_input)
        .send()
        .await?;
    assert_eq!(stale_http.status(), reqwest::StatusCode::CONFLICT);
    let stale_http_error = stale_http.json::<serde_json::Value>().await?;
    let stale_direct_error = application
        .reorder_feedback_attachments(stale_input)
        .await
        .expect_err("direct stale reorder should fail");
    assert_eq!(stale_http_error, serde_json::to_value(stale_direct_error)?);
    assert_no_path_keys(&stale_http_error);

    let contents_wire = client
        .post(application_url(server.address(), "addFeedbackAttachment"))
        .bearer_auth(TEST_TOKEN)
        .multipart(
            reqwest::multipart::Form::new()
                .text("request_id", request_id.clone())
                .text("file_name", "unsupported.txt")
                .text("expected_revision", "1")
                .part(
                    "contents",
                    reqwest::multipart::Part::bytes(b"bytes".to_vec()),
                ),
        )
        .send()
        .await?;
    assert_eq!(contents_wire.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        contents_wire.json::<serde_json::Value>().await?["code"],
        "INVALID_ARGUMENT"
    );

    let second = client
        .post(application_url(server.address(), "addFeedbackAttachment"))
        .bearer_auth(TEST_TOKEN)
        .multipart(
            reqwest::multipart::Form::new()
                .text("request_id", request_id.clone())
                .text("file_name", "second.txt")
                .text("expected_revision", "1")
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(b"second attachment bytes".to_vec()),
                ),
        )
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let second_attachment_id = second["attachments"][1]["attachment_id"]
        .as_str()
        .expect("second attachment id")
        .to_owned();

    let reordered = client
        .post(application_url(
            server.address(),
            "reorderFeedbackAttachments",
        ))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": request_id,
            "attachment_ids": [second_attachment_id, first_attachment_id],
            "expected_revision": 2
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(
        reordered["attachments"][0]["attachment_id"],
        second_attachment_id
    );
    assert_no_path_keys(&reordered);

    let removed = client
        .post(application_url(
            server.address(),
            "removeFeedbackAttachment",
        ))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": request_id,
            "attachment_id": first_attachment_id,
            "expected_revision": 3
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    assert_eq!(removed["attachments"].as_array().map(Vec::len), Some(1));
    assert_no_path_keys(&removed);

    let missing = client
        .post(application_url(server.address(), "readFeedbackAttachment"))
        .bearer_auth(TEST_TOKEN)
        .json(&serde_json::json!({
            "request_id": request_id,
            "attachment_id": first_attachment_id
        }))
        .send()
        .await?;
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
    let missing_error = missing.json::<serde_json::Value>().await?;
    assert_eq!(missing_error["code"], "ATTACHMENT_NOT_FOUND");
    assert_no_path_keys(&missing_error);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn upload_keeps_the_core_20_mib_content_limit_as_the_source_of_truth() -> anyhow::Result<()> {
    let (application, _directory) = test_application().await?;
    let request_id = seed_request(&application).await;
    let server =
        start_application_server(application.clone(), terminal_operations(&application)).await?;

    let oversized = reqwest::Client::new()
        .post(application_url(server.address(), "addFeedbackAttachment"))
        .bearer_auth(TEST_TOKEN)
        .multipart(
            reqwest::multipart::Form::new()
                .text("request_id", request_id)
                .text("file_name", "oversized.bin")
                .text("expected_revision", "0")
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(vec![0; MAX_ATTACHMENT_BYTES + 1]),
                ),
        )
        .send()
        .await?;
    assert_eq!(oversized.status(), reqwest::StatusCode::BAD_REQUEST);
    let error = oversized.json::<serde_json::Value>().await?;
    assert_eq!(error["code"], "INVALID_ARGUMENT");
    assert_eq!(error["message"], "attachment exceeds the 20 MiB limit");

    server.shutdown().await?;
    Ok(())
}
