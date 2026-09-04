use super::*;

pub(super) async fn feedback_tool_result(
    application: &FeedbackApplication,
    result: Result<FeedbackRequestView, ApplicationError>,
    include_package_when_terminal: bool,
) -> CallToolResult {
    feedback_result(application, result, include_package_when_terminal, false).await
}

pub(super) async fn managed_feedback_tool_result(
    application: &FeedbackApplication,
    result: Result<FeedbackRequestView, ApplicationError>,
    include_package_when_terminal: bool,
) -> CallToolResult {
    feedback_result(application, result, include_package_when_terminal, true).await
}

async fn feedback_result(
    application: &FeedbackApplication,
    result: Result<FeedbackRequestView, ApplicationError>,
    include_package_when_terminal: bool,
    managed: bool,
) -> CallToolResult {
    let value = match result {
        Ok(value) => value,
        Err(error) => return application_error_result(error),
    };

    let mut package = None;
    if include_package_when_terminal
        && matches!(
            value.status,
            FeedbackStatus::Completed | FeedbackStatus::Cancelled
        )
    {
        package = match application.read_feedback_package(&value).await {
            Ok(Some(package)) => Some(package),
            Ok(None) => None,
            Err(error) => return application_error_result(error),
        };
    }

    let summary = match value.status {
        FeedbackStatus::Waiting | FeedbackStatus::InProgress if managed => format!(
            "Feedback request {} is waiting for the human in RambleDesk. End this Agent turn now. RambleDesk will automatically continue this same Agent session when feedback is ready; then call get_feedback with this original request_id. Do not poll, block on another tool, ask for external confirmation, or create a replacement request.",
            value.request_id
        ),
        FeedbackStatus::Waiting => format!(
            "Feedback request {} is waiting for the human. You MUST wait now: call this host's interactive confirmation tool (ask_question / ask / ask_user_question) with a message that the human should complete the request in RambleDesk, and keep this turn blocked inside that confirmation call. When the human answers, call get_feedback with this request_id and continue. Only if the host has no such tool, end this turn and resume when notified. Do not poll.",
            value.request_id
        ),
        FeedbackStatus::InProgress => format!(
            "Feedback request {} is in progress. You MUST wait now: call this host's interactive confirmation tool (ask_question / ask / ask_user_question) with a message that the human should complete the request in RambleDesk, and keep this turn blocked inside that confirmation call. When the human answers, call get_feedback with this request_id and continue. Only if the host has no such tool, end this turn and resume when notified.",
            value.request_id
        ),
        FeedbackStatus::Completed => {
            let mut summary = format!("Feedback request {} is completed.", value.request_id);
            if let Some(package) = package.as_ref() {
                summary.push_str(
                    "\n\nThe human submitted a feedback package. The full feedback is NOT inlined in this text (attachments can be binary); read the files below. The complete package is also available in structured_content.feedback_package for clients that support it.\n",
                );
                if let Some(feedback) = value.feedback.as_ref() {
                    summary.push_str(&format!(
                        "- Feedback markdown: {}\n",
                        feedback.markdown_path
                    ));
                    summary.push_str(&format!(
                        "- Package directory: {}\n",
                        feedback.directory_path
                    ));
                    if package.manifest.uncooked_markdown.is_some() {
                        summary.push_str(&format!(
                            "- Uncooked markdown: {}\n",
                            std::path::Path::new(&feedback.directory_path)
                                .join("uncooked.md")
                                .to_string_lossy()
                        ));
                    }
                }
                if !package.attachment_paths.is_empty() {
                    summary.push_str("\nAttachments (read with read_file):\n");
                    for path in &package.attachment_paths {
                        summary.push_str(&format!("- {path}\n"));
                    }
                }
                if !package.request_attachment_paths.is_empty() {
                    summary.push_str("\nRequest attachments (read with read_file):\n");
                    for path in &package.request_attachment_paths {
                        summary.push_str(&format!("- {path}\n"));
                    }
                }
                let preview: String = package.markdown.chars().take(800).collect();
                summary.push_str("\nPreview of feedback markdown:\n");
                summary.push_str(&preview);
                if package.markdown.chars().count() > 800 {
                    summary.push_str(
                        "\n… (preview truncated — read the markdown file for the full feedback)\n",
                    );
                }
            }
            summary
        }
        FeedbackStatus::Cancelled => {
            format!("Feedback request {} is cancelled.", value.request_id)
        }
    };

    let mut structured = serde_json::to_value(&value).expect("application result must serialize");
    let object = structured
        .as_object_mut()
        .expect("feedback request view must serialize as an object");
    if managed {
        object.remove("poll_after_ms");
        object.remove("execution_mode");
    }

    if let Some(host) = current_request_host() {
        object.insert("host".to_owned(), serde_json::Value::String(host));
    }

    if let Some(package) = package {
        object.insert(
            "feedback_package".to_owned(),
            serde_json::to_value(package).expect("feedback package must serialize"),
        );
    }

    let mut result = CallToolResult::structured(structured);
    result.content = vec![ContentBlock::text(summary)];
    result
}

pub(super) fn application_error_result(error: ApplicationError) -> CallToolResult {
    structured_error_result(error.code(), error.message(), error.retryable())
}

pub(super) fn structured_error_result(
    code: &str,
    message: &str,
    retryable: bool,
) -> CallToolResult {
    let mut result = CallToolResult::structured_error(serde_json::json!({
        "code": code,
        "message": message,
        "retryable": retryable,
    }));
    result.content = vec![ContentBlock::text(format!(
        "RambleDesk {}: {}",
        code, message
    ))];
    result
}
