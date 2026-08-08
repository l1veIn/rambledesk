//! Generic MCP Adapter scheme for RambleDesk.
//!
//! The complete adapter, mirroring `packages/pi-rambledesk`: the server tool
//! surface plus a client-side detect/install engine. All per-host knowledge
//! (executable names, config paths, `ConfigFormat`) lives in
//! `rambledesk-hosts`; this crate only executes against it.

use std::future::Future;

use rambledesk_core::{
    ApplicationError, CancelFeedbackInput, FeedbackApplication, FeedbackRequestView,
    FeedbackStatus, GetFeedbackInput, RequestFeedbackInput,
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

mod install;

pub use install::{McpHostView, McpInstallResult, detect_hosts, install_hosts};

tokio::task_local! {
    static REQUEST_HOST: Option<String>;
}

pub async fn with_request_host<F>(host: Option<String>, future: F) -> F::Output
where
    F: Future,
{
    REQUEST_HOST.scope(host, future).await
}

pub fn current_request_host() -> Option<String> {
    REQUEST_HOST.try_with(|host| host.clone()).ok().flatten()
}

#[derive(Clone)]
pub struct RambleDeskMcp {
    tool_router: ToolRouter<Self>,
    application: FeedbackApplication,
}

impl RambleDeskMcp {
    pub fn new(application: FeedbackApplication) -> Self {
        Self {
            tool_router: Self::tool_router(),
            application,
        }
    }
}

fn apply_request_host(mut input: RequestFeedbackInput) -> RequestFeedbackInput {
    if let Some(host) = current_request_host() {
        input.host_id = Some(host);
    }
    input
}

#[tool_router]
impl RambleDeskMcp {
    #[tool(
        name = "request_feedback",
        description = "Persist a feedback request and return immediately with a durable handle (request_id). Optional attachments let the agent provide review artifacts: use attachments[].markdown with a .md/.markdown file_name for Markdown, or attachments[].contents_base64 for a PNG/JPEG/GIF/WebP image. After creating, if this host has an interactive confirmation tool (ask / ask_choice / similar), use it to wait for the human to finish, then call get_feedback with the same request_id; only stop the current turn when no such tool exists. Do not poll. Reusing request_id with identical input is idempotent. host_id is optional: auto-registered clients (RAMBLEDESK_HOST / X-RambleDesk-Host) have it injected by the server, otherwise pass your host family id (e.g. reasonix, claude, codex, opencode) or generic. host_session_id is the current session identifier. allow_finish: set true ONLY when the request needs a simple final approval or rejection from the human and no feedback body is expected; in that case final_summary (the exact closing statement) is required. For requests that gather feedback, review, or opinions (proofreading, checking work, answering questions), omit allow_finish so the human submits detailed feedback instead of a shortcut finish."
    )]
    async fn request_feedback(
        &self,
        Parameters(input): Parameters<RequestFeedbackInput>,
    ) -> CallToolResult {
        let input = apply_request_host(input);
        feedback_tool_result(
            &self.application,
            self.application.request_feedback(input).await,
            false,
        )
        .await
    }

    #[tool(
        name = "get_feedback",
        description = "Read the current state of a durable feedback request without changing it. Use after manual continuation or for diagnostics. When status is completed, the reply text names the feedback markdown path plus attachment paths and a short preview; read the markdown file for the full feedback (text-only clients see only the reply text). The complete package (manifest, markdown, attachment paths) is also in structured_content.feedback_package for clients that support it. Do not poll while waiting: after request_feedback, wait via this host's interactive confirmation tool (ask / ask_choice) if available, otherwise end the turn and resume when notified."
    )]
    async fn get_feedback(
        &self,
        Parameters(input): Parameters<GetFeedbackInput>,
    ) -> CallToolResult {
        feedback_tool_result(
            &self.application,
            self.application.get_feedback(input).await,
            true,
        )
        .await
    }

    #[tool(
        name = "cancel_feedback",
        description = "Cancel a waiting or in-progress feedback request. Repeated cancellation preserves the first cancellation."
    )]
    async fn cancel_feedback(
        &self,
        Parameters(input): Parameters<CancelFeedbackInput>,
    ) -> CallToolResult {
        feedback_tool_result(
            &self.application,
            self.application.cancel_feedback(input).await,
            false,
        )
        .await
    }
}

async fn feedback_tool_result(
    application: &FeedbackApplication,
    result: Result<FeedbackRequestView, ApplicationError>,
    include_package_when_terminal: bool,
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
        FeedbackStatus::Waiting => format!(
            "Feedback request {} is waiting for the human. If this host has an interactive confirmation tool (ask / ask_choice), use it to wait for the human, then call get_feedback with this request_id; otherwise end this turn and resume when notified. Do not poll.",
            value.request_id
        ),
        FeedbackStatus::InProgress => format!(
            "Feedback request {} is in progress. If this host has an interactive confirmation tool (ask / ask_choice), use it to wait for the human, then call get_feedback with this request_id; otherwise end this turn and resume when notified.",
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

fn application_error_result(error: ApplicationError) -> CallToolResult {
    structured_error_result(error.code(), error.message(), error.retryable())
}

fn structured_error_result(code: &str, message: &str, retryable: bool) -> CallToolResult {
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

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RambleDeskMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rambledesk", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "RambleDesk tools: request_feedback, get_feedback, cancel_feedback. \
Create a durable request with request_feedback; it returns immediately with a request_id. \
If this host has an interactive confirmation tool (ask / ask_choice), use it after creating the request to wait for the human to finish, then call get_feedback(request_id); only end the current turn when no such tool exists — do not poll and do not wait on a long MCP tool call. \
After the human submits feedback or after a disconnect, call get_feedback(request_id) to load the current server state and package. \
Attach Markdown review documents with attachments[].markdown and images with attachments[].contents_base64 when useful. \
host_id is optional: auto-registered clients (RAMBLEDESK_HOST / X-RambleDesk-Host) have it injected by the server, otherwise pass your host family id (e.g. reasonix, claude, codex, opencode) or generic.",
            )
    }
}
