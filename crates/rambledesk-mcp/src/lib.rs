//! Generic MCP adapter tool surface for RambleDesk.

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
        input.host_id = host;
    }
    input
}

#[tool_router]
impl RambleDeskMcp {
    #[tool(
        name = "request_feedback",
        description = "Persist a feedback request and return immediately with a durable handle (request_id). Optional attachments let the agent provide review artifacts: use attachments[].markdown with a .md/.markdown file_name for Markdown, or attachments[].contents_base64 for a PNG/JPEG/GIF/WebP image. After creating, if the host provides an interactive confirmation tool (ask / ask_choice / similar), you may use it to wait for the human to finish and then call get_feedback with the same request_id; otherwise stop the current turn and wait to be resumed. Do not poll. Reusing request_id with identical input is idempotent. Auto-registered clients set RAMBLEDESK_HOST / X-RambleDesk-Host so host_id is known without guessing."
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
        description = "Read the current state of a durable feedback request without changing it. Use after manual continuation or for diagnostics. When status is completed, the response includes the full feedback package (manifest, markdown, attachment paths). Do not poll while waiting; end the turn after request_feedback and resume when notified."
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

    let summary = match value.status {
        FeedbackStatus::Waiting => format!(
            "Feedback request {} is waiting for the human. End this turn; do not poll. When resumed, call get_feedback with this request_id.",
            value.request_id
        ),
        FeedbackStatus::InProgress => format!(
            "Feedback request {} is in progress. End this turn; when resumed, call get_feedback with this request_id.",
            value.request_id
        ),
        FeedbackStatus::Completed => {
            format!("Feedback request {} is completed.", value.request_id)
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

    if include_package_when_terminal
        && matches!(
            value.status,
            FeedbackStatus::Completed | FeedbackStatus::Cancelled
        )
    {
        match application.read_feedback_package(&value).await {
            Ok(Some(package)) => {
                object.insert(
                    "feedback_package".to_owned(),
                    serde_json::to_value(package).expect("feedback package must serialize"),
                );
            }
            Ok(None) => {}
            Err(error) => {
                return application_error_result(error);
            }
        }
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
If the host has an interactive confirmation tool (ask / ask_choice), use it after creating the request to wait for the human to finish, then call get_feedback(request_id); otherwise end the current turn — do not poll and do not wait on a long MCP tool call. \
After the human submits feedback or after a disconnect, call get_feedback(request_id) to load the current server state and package. \
Attach Markdown review documents with attachments[].markdown and images with attachments[].contents_base64 when useful. \
Auto-registered clients set RAMBLEDESK_HOST (and X-RambleDesk-Host) so host identity is known without guessing.",
            )
    }
}
