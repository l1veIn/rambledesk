use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode, header::AUTHORIZATION},
    response::IntoResponse,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rambledesk_core::kernel::{
    AcpSessionLinkId, ArtifactRole, ContextReference, Core, CreateFeedbackRequest, DeliveryId,
    FeedbackAction, GetFeedback, GetFeedbackOutcome, RequestId, SessionId,
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CacheScope, CallToolResult, ContentBlock, Implementation, ListToolsResult,
        PaginatedRequestParams, ResultType, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio::{net::TcpListener, sync::RwLock, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tower_service::Service;

use crate::{AcpClientError, AcpErrorCode};

const MAX_BODY_BYTES: usize = 96 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct RequestFeedbackActionInput {
    id: String,
    instruction: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ContextReferenceInput {
    label: String,
    uri: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct RequestFeedbackToolInput {
    #[serde(default)]
    request_id: Option<String>,
    title: String,
    instructions: String,
    actions: Vec<RequestFeedbackActionInput>,
    #[serde(default)]
    context_refs: Vec<ContextReferenceInput>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct GetFeedbackToolInput {
    request_id: String,
}

#[derive(Clone)]
struct V3SessionToolset {
    router: ToolRouter<Self>,
    core: Arc<Core>,
    session_id: SessionId,
    source_link_id: Arc<RwLock<Option<AcpSessionLinkId>>>,
    observations: SessionToolObservationLog,
}

impl V3SessionToolset {
    fn new(
        core: Arc<Core>,
        session_id: SessionId,
        source_link_id: Arc<RwLock<Option<AcpSessionLinkId>>>,
        observations: SessionToolObservationLog,
    ) -> Self {
        Self {
            router: Self::tool_router(),
            core,
            session_id,
            source_link_id,
            observations,
        }
    }
}

#[tool_router]
impl V3SessionToolset {
    #[tool(
        name = "request_feedback",
        description = "Required end-of-Turn handoff for active managed Ramble work. Create a durable RambleDesk Feedback Request and return immediately, including when the task or current stage appears complete. Permission Requests, Ask Questions, and plain assistant messages do not replace this handoff. After this tool returns, end only the current Turn, never the Session. Do not poll; RambleDesk will resume the Session later and ask you to call get_feedback."
    )]
    async fn request_feedback(
        &self,
        Parameters(input): Parameters<RequestFeedbackToolInput>,
    ) -> CallToolResult {
        let request_id = match self
            .observations
            .resolve_request_id(input.request_id.as_deref())
        {
            Ok(request_id) => request_id,
            Err(result) => return result,
        };
        let request = CreateFeedbackRequest {
            request_id,
            session_id: self.session_id.clone(),
            source_link_id: self.source_link_id.read().await.clone(),
            title: input.title,
            instructions: input.instructions,
            actions: input
                .actions
                .into_iter()
                .map(|action| FeedbackAction {
                    id: action.id,
                    instruction: action.instruction,
                })
                .collect(),
            context_refs: input
                .context_refs
                .into_iter()
                .map(|reference| ContextReference {
                    label: reference.label,
                    uri: reference.uri,
                })
                .collect(),
            artifacts: Vec::new(),
        };
        match self.core.request_feedback(request).await {
            Ok(snapshot) => {
                self.observations
                    .record(SessionToolObservation::FeedbackRequested {
                        request_id: snapshot.request_id.clone(),
                    });
                tool_success(
                    json!({
                        "request_id": snapshot.request_id,
                        "session_id": snapshot.session_id,
                        "status": "waiting",
                        "created_at": snapshot.created_at,
                        "instruction": "End the current turn. Feedback may arrive in a future Agent Run."
                    }),
                    format!(
                        "Feedback request {} is waiting. End this turn; RambleDesk will resume the Session when the human responds.",
                        snapshot.request_id
                    ),
                )
            }
            Err(error) => tool_error(error.code().as_str(), error.message(), error.retryable()),
        }
    }

    #[tool(
        name = "get_feedback",
        description = "Read one durable Feedback Request by request_id. Call this when RambleDesk resumes the Session after feedback. The result is location-independent and never returns a local package path. De-duplicate terminal results by delivery_id."
    )]
    async fn get_feedback(
        &self,
        Parameters(input): Parameters<GetFeedbackToolInput>,
    ) -> CallToolResult {
        let result = self
            .core
            .get_feedback(GetFeedback {
                request_id: RequestId::new(input.request_id),
            })
            .await;
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                return tool_error(error.code().as_str(), error.message(), error.retryable());
            }
        };
        match location_independent_delivery(&self.session_id, outcome) {
            Ok(value) => {
                if let (Some(delivery_id), Some(request_id)) = (
                    value.get("delivery_id").and_then(Value::as_str),
                    value.get("request_id").and_then(Value::as_str),
                ) {
                    self.observations
                        .record(SessionToolObservation::FeedbackConsumed {
                            delivery_id: DeliveryId::new(delivery_id),
                            request_id: RequestId::new(request_id),
                        });
                }
                tool_success(value.clone(), delivery_summary(&value))
            }
            Err(error) => tool_error(
                format!("{:?}", error.code).as_str(),
                &error.message,
                error.retryable,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionToolObservation {
    FeedbackRequested {
        request_id: RequestId,
    },
    FeedbackConsumed {
        delivery_id: DeliveryId,
        request_id: RequestId,
    },
}

#[derive(Clone, Default)]
pub(crate) struct SessionToolObservationLog {
    state: Arc<Mutex<SessionToolObservationState>>,
}

#[derive(Default)]
struct SessionToolObservationState {
    generation: u64,
    observations: Vec<SessionToolObservation>,
    active_feedback_request_id: Option<RequestId>,
    required_feedback: Option<RequiredFeedback>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequiredFeedback {
    pub(crate) request_id: RequestId,
    pub(crate) delivery_id: DeliveryId,
}

impl SessionToolObservationLog {
    fn record(&self, observation: SessionToolObservation) {
        self.state
            .lock()
            .expect("Session Toolset observation lock poisoned")
            .observations
            .push(observation);
    }

    fn resolve_request_id(
        &self,
        supplied: Option<&str>,
    ) -> Result<Option<RequestId>, CallToolResult> {
        let state = self
            .state
            .lock()
            .expect("Session Toolset observation lock poisoned");
        if let Some(required) = &state.required_feedback
            && !state.observations.iter().any(|observation| {
                matches!(
                    observation,
                    SessionToolObservation::FeedbackConsumed {
                        request_id,
                        delivery_id,
                    } if request_id == &required.request_id && delivery_id == &required.delivery_id
                )
            })
        {
            return Err(tool_error(
                "FEEDBACK_NOT_CONSUMED",
                &format!(
                    "call get_feedback for request_id {} and consume delivery_id {} before request_feedback",
                    required.request_id, required.delivery_id
                ),
                false,
            ));
        }
        let active = state.active_feedback_request_id.clone();
        drop(state);
        match (active, supplied) {
            (Some(expected), Some(actual)) if actual != expected.as_str() => Err(tool_error(
                "RAMBLE_REQUEST_ID_MISMATCH",
                &format!("request_id must be `{expected}` for the active managed Ramble Turn"),
                false,
            )),
            (Some(expected), _) => Ok(Some(expected)),
            (None, supplied) => Ok(supplied.map(RequestId::new)),
        }
    }

    pub(crate) fn begin_managed_work(
        &self,
        request_id: RequestId,
        required_feedback: Option<RequiredFeedback>,
    ) -> u64 {
        let mut state = self
            .state
            .lock()
            .expect("Session Toolset observation lock poisoned");
        state.generation = state.generation.wrapping_add(1);
        state.observations.clear();
        state.active_feedback_request_id = Some(request_id);
        state.required_feedback = required_feedback;
        state.generation
    }

    pub(crate) fn end_managed_work(&self, request_id: &RequestId) {
        let mut state = self
            .state
            .lock()
            .expect("Session Toolset observation lock poisoned");
        if state.active_feedback_request_id.as_ref() == Some(request_id) {
            state.active_feedback_request_id = None;
            state.required_feedback = None;
        }
    }

    pub(crate) fn since(&self, generation: u64) -> Vec<SessionToolObservation> {
        let state = self
            .state
            .lock()
            .expect("Session Toolset observation lock poisoned");
        if state.generation == generation {
            state.observations.clone()
        } else {
            Vec::new()
        }
    }
}

#[tool_handler(router = self.router)]
impl ServerHandler for V3SessionToolset {
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            result_type: Some(ResultType::COMPLETE),
            tools: self.router.list_all(),
            meta: None,
            next_cursor: None,
            ttl_ms: Some(0),
            cache_scope: Some(CacheScope::Private),
        })
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "rambledesk-session-toolset",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "During active managed Ramble work, request_feedback is the required final handoff for every Prompt Turn. Permission Requests, Ask Questions, ordinary messages, and task completion do not replace it. The call is a short durable write: keep its request_id and end only the Turn. RambleDesk may resume this Session much later. When resumed, first call get_feedback(request_id); terminal responses carry a stable delivery_id and location-independent content, then continue work and finish with the next request_feedback handoff.",
            )
    }
}

fn location_independent_delivery(
    bound_session_id: &SessionId,
    outcome: GetFeedbackOutcome,
) -> Result<Value, AcpClientError> {
    match outcome {
        GetFeedbackOutcome::Waiting {
            request_id,
            session_id,
        } => {
            ensure_bound_session(bound_session_id, &session_id)?;
            Ok(json!({
                "request_id": request_id,
                "session_id": session_id,
                "status": "waiting"
            }))
        }
        GetFeedbackOutcome::Terminal(envelope) => {
            ensure_bound_session(bound_session_id, &envelope.session_id)?;
            let mut feedback_markdown = None;
            let mut uncooked_markdown = None;
            let mut artifacts = Vec::new();
            for artifact in envelope.artifacts {
                match artifact.role {
                    ArtifactRole::Feedback => {
                        feedback_markdown = String::from_utf8(artifact.contents).ok();
                    }
                    ArtifactRole::Uncooked => {
                        uncooked_markdown = String::from_utf8(artifact.contents).ok();
                    }
                    _ => artifacts.push(json!({
                        "artifact_id": artifact.artifact_id,
                        "role": artifact.role,
                        "display_name": artifact.display_name,
                        "media_type": artifact.media_type,
                        "size_bytes": artifact.size_bytes,
                        "sha256": artifact.sha256,
                        "locator": {
                            "kind": "inline_base64",
                            "value": BASE64.encode(artifact.contents)
                        }
                    })),
                }
            }
            Ok(json!({
                "delivery_id": envelope.delivery_id,
                "request_id": envelope.request_id,
                "session_id": envelope.session_id,
                "resolution": envelope.resolution,
                "package": envelope.package_id.map(|package_id| json!({
                    "package_id": package_id,
                    "content_digest": envelope.package_content_digest,
                    "manifest_digest": envelope.package_manifest_digest,
                    "feedback_markdown": feedback_markdown,
                    "uncooked_markdown": uncooked_markdown,
                    "artifacts": artifacts
                })),
                "reason": envelope.cancel_reason
            }))
        }
    }
}

fn ensure_bound_session(expected: &SessionId, actual: &SessionId) -> Result<(), AcpClientError> {
    if expected == actual {
        Ok(())
    } else {
        Err(AcpClientError::new(
            AcpErrorCode::InvalidArgument,
            "Feedback Request does not belong to this managed Session",
            false,
        ))
    }
}

fn delivery_summary(value: &Value) -> String {
    if value.get("status").and_then(Value::as_str) == Some("waiting") {
        return "Feedback is still waiting; end the turn and do not poll.".to_string();
    }
    match value.get("resolution").and_then(Value::as_str) {
        Some("submitted") => {
            let delivery_id = value
                .get("delivery_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let package = value.get("package").unwrap_or(&Value::Null);
            if let Some(feedback) = package
                .get("feedback_markdown")
                .and_then(Value::as_str)
                .filter(|feedback| !feedback.trim().is_empty())
            {
                format!(
                    "Feedback delivery {delivery_id} is submitted. De-duplicate it by delivery_id.\n\nHuman feedback:\n\n{feedback}\n\nAttachment bytes remain available only in structuredContent and are not repeated here."
                )
            } else if let Some(uncooked) = package
                .get("uncooked_markdown")
                .and_then(Value::as_str)
                .filter(|uncooked| !uncooked.trim().is_empty())
            {
                format!(
                    "Feedback delivery {delivery_id} is submitted. De-duplicate it by delivery_id. Structured feedback text was unavailable, so this is the uncooked human feedback:\n\n{uncooked}\n\nAttachment bytes remain available only in structuredContent and are not repeated here."
                )
            } else {
                format!(
                    "Feedback delivery {delivery_id} is submitted, but it contains no human-readable feedback text. Inspect structuredContent for package metadata and attachments."
                )
            }
        }
        Some("cancelled") => "The human cancelled this Feedback Request.".to_string(),
        _ => "Feedback delivery is available.".to_string(),
    }
}

fn tool_success(structured: Value, summary: String) -> CallToolResult {
    let mut result = CallToolResult::structured(structured);
    result.content = vec![ContentBlock::text(summary)];
    result
}

fn tool_error(code: &str, message: &str, retryable: bool) -> CallToolResult {
    let mut result = CallToolResult::structured_error(json!({
        "code": code,
        "message": message,
        "retryable": retryable
    }));
    result.content = vec![ContentBlock::text(format!("RambleDesk {code}: {message}"))];
    result
}

#[derive(Clone)]
struct HttpState {
    service: StreamableHttpService<V3SessionToolset, LocalSessionManager>,
    bearer: Arc<str>,
}

async fn handle_http(State(mut state): State<HttpState>, request: Request<Body>) -> Response<Body> {
    let authorized = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == format!("Bearer {}", state.bearer));
    if !authorized {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    match state.service.call(request).await {
        Ok(response) => response.into_response(),
        Err(infallible) => match infallible {},
    }
}

pub(crate) struct SessionToolsetHandle {
    pub(crate) mcp_server: Value,
    pub(crate) digest: String,
    source_link_id: Arc<RwLock<Option<AcpSessionLinkId>>>,
    observations: SessionToolObservationLog,
    cancellation: CancellationToken,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl SessionToolsetHandle {
    pub(crate) async fn start(
        core: Arc<Core>,
        session_id: SessionId,
    ) -> Result<Self, AcpClientError> {
        let cancellation = CancellationToken::new();
        let source_link_id = Arc::new(RwLock::new(None));
        let observations = SessionToolObservationLog::default();
        let toolset = V3SessionToolset::new(
            core,
            session_id,
            source_link_id.clone(),
            observations.clone(),
        );
        let transport = StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_max_request_body_bytes(MAX_BODY_BYTES)
            .with_cancellation_token(cancellation.child_token());
        let service =
            StreamableHttpService::new(move || Ok(toolset.clone()), Default::default(), transport);
        let bearer = uuid::Uuid::now_v7().to_string();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|error| {
                AcpClientError::new(
                    AcpErrorCode::AgentLaunchFailed,
                    format!("could not bind Session Toolset: {error}"),
                    true,
                )
            })?;
        let address = listener.local_addr().map_err(|error| {
            AcpClientError::new(
                AcpErrorCode::AgentLaunchFailed,
                format!("could not inspect Session Toolset listener: {error}"),
                true,
            )
        })?;
        let endpoint = format!("http://{address}/mcp");
        let router = Router::new().fallback(handle_http).with_state(HttpState {
            service,
            bearer: Arc::from(bearer.as_str()),
        });
        let task_cancel = cancellation.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move { task_cancel.cancelled_owned().await })
                .await
        });
        Ok(Self {
            mcp_server: json!({
                "type": "http",
                "name": "rambledesk",
                "url": endpoint,
                "headers": [{"name": "Authorization", "value": format!("Bearer {bearer}")}]
            }),
            digest: toolset_digest(),
            source_link_id,
            observations,
            cancellation,
            task,
        })
    }

    pub(crate) async fn set_source_link(&self, link_id: AcpSessionLinkId) {
        *self.source_link_id.write().await = Some(link_id);
    }

    pub(crate) fn begin_managed_work(
        &self,
        request_id: RequestId,
        required_feedback: Option<RequiredFeedback>,
    ) -> u64 {
        self.observations
            .begin_managed_work(request_id, required_feedback)
    }

    pub(crate) fn end_managed_work(&self, request_id: &RequestId) {
        self.observations.end_managed_work(request_id);
    }

    pub(crate) fn observations_since(&self, generation: u64) -> Vec<SessionToolObservation> {
        self.observations.since(generation)
    }

    pub(crate) async fn shutdown(self) -> Result<(), AcpClientError> {
        self.cancellation.cancel();
        let result = self.task.await.map_err(|error| {
            AcpClientError::new(
                AcpErrorCode::ShutdownFailed,
                format!("Session Toolset task failed: {error}"),
                false,
            )
        })?;
        result.map_err(|error| {
            AcpClientError::new(
                AcpErrorCode::ShutdownFailed,
                format!("Session Toolset server failed: {error}"),
                false,
            )
        })?;
        Ok(())
    }
}

pub(crate) fn toolset_digest() -> String {
    let contract = br#"{"schema_version":3,"tools":["get_feedback","request_feedback"],"transport_contract":"location_independent","managed_ramble_loop":"required_end_of_turn_handoff","terminal_text_projection":"feedback_markdown_with_uncooked_fallback"}"#;
    format!("sha256:{}", hex::encode(Sha256::digest(contract)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rambledesk_core::kernel::{
        AccessMode, AcpSessionLinkObservation, AgentObservation, ArtifactInput, DraftId,
        DraftMutation, FeedbackSubmission, LaunchConfiguration, LaunchSubmission, RambleContent,
        RambleIntent, ResolveFeedbackRequest, SaveDraft, SubmissionId, WorkbenchQuery,
    };
    use rambledesk_storage::v3::{SqliteV3Store, artifact::LocalArtifactStore};
    use reqwest::header::{ACCEPT, AUTHORIZATION};
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn toolset_digest_is_stable_and_canonical() {
        let digest = toolset_digest();
        assert_eq!(digest.len(), 71);
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest, toolset_digest());
    }

    #[test]
    fn submitted_delivery_text_falls_back_to_uncooked_without_copying_attachments() {
        let text = delivery_summary(&json!({
            "delivery_id":"delivery-fallback",
            "resolution":"submitted",
            "package":{
                "feedback_markdown":null,
                "uncooked_markdown":"Raw human direction",
                "artifacts":[{"locator":{"kind":"inline_base64","value":"SECRET_BASE64"}}]
            }
        }));
        assert!(text.contains("uncooked human feedback"), "{text}");
        assert!(text.contains("Raw human direction"), "{text}");
        assert!(!text.contains("SECRET_BASE64"), "{text}");
    }

    #[tokio::test]
    async fn loopback_toolset_calls_only_v3_core_and_returns_short_feedback_handle() {
        let temp = TempDir::new().expect("tempdir");
        let store = Arc::new(
            SqliteV3Store::connect(&temp.path().join("v3.sqlite3"))
                .await
                .expect("v3 store"),
        );
        let artifacts = Arc::new(
            LocalArtifactStore::open(temp.path().join("library"))
                .await
                .expect("artifacts"),
        );
        let core = Arc::new(Core::new(store.clone(), artifacts));
        let launch = core
            .launch(LaunchSubmission {
                submission_id: SubmissionId::new("toolset-launch"),
                submission_digest_assertion: None,
                title: "Toolset Session".to_string(),
                launch_configuration: LaunchConfiguration {
                    agent_profile_id: "fake".to_string(),
                    launch_profile_id: "fake".to_string(),
                    workspace_reference: temp.path().to_string_lossy().to_string(),
                    model: None,
                    reasoning_effort: None,
                    access_mode: AccessMode::WorkspaceWrite,
                    agent_config_json: "{}".to_string(),
                },
                ramble: RambleContent {
                    document_json: "{}".to_string(),
                    body_markdown: "Launch".to_string(),
                    artifacts: Vec::<ArtifactInput>::new(),
                },
            })
            .await
            .expect("launch");
        let link = core
            .record_agent_observation(AgentObservation::AcpSessionLinked(
                AcpSessionLinkObservation {
                    session_id: launch.session_id.clone(),
                    agent_profile_id: "fake".to_string(),
                    launch_profile_id: "fake".to_string(),
                    acp_session_id: "agent-session".to_string(),
                    capabilities_json: "{}".to_string(),
                    session_toolset_digest: toolset_digest(),
                },
            ))
            .await
            .expect("link");
        let handle = SessionToolsetHandle::start(core.clone(), launch.session_id.clone())
            .await
            .expect("toolset");
        handle.set_source_link(link.link_id).await;
        let endpoint = handle.mcp_server["url"].as_str().unwrap().to_string();
        let authorization = handle.mcp_server["headers"][0]["value"]
            .as_str()
            .unwrap()
            .to_string();
        let client = reqwest::Client::new();
        let initialize = client
            .post(&endpoint)
            .header(AUTHORIZATION, &authorization)
            .header(ACCEPT, "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{
                    "protocolVersion":"2025-06-18",
                    "capabilities":{},
                    "clientInfo":{"name":"fake-agent","version":"1"}
                }
            }))
            .send()
            .await
            .expect("initialize response");
        assert_eq!(initialize.status(), reqwest::StatusCode::OK);
        let blocked_request_id = RequestId::new("blocked-request");
        handle.begin_managed_work(
            blocked_request_id.clone(),
            Some(RequiredFeedback {
                request_id: RequestId::new("prior-request"),
                delivery_id: DeliveryId::new("prior-delivery"),
            }),
        );
        let blocked = client
            .post(&endpoint)
            .header(AUTHORIZATION, &authorization)
            .header(ACCEPT, "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc":"2.0",
                "id":2,
                "method":"tools/call",
                "params":{
                    "name":"request_feedback",
                    "arguments":{
                        "title":"Skipped delivery",
                        "instructions":"This request must not be exposed.",
                        "actions":[{"id":"continue","instruction":"Continue."}]
                    }
                }
            }))
            .send()
            .await
            .expect("blocked tool call response");
        let blocked_body = blocked.text().await.expect("blocked tool body");
        assert!(
            blocked_body.contains("FEEDBACK_NOT_CONSUMED"),
            "{blocked_body}"
        );
        assert!(
            core.read_workbench(WorkbenchQuery {
                session_id: Some(launch.session_id.clone()),
            })
            .await
            .expect("workbench")
            .waiting_feedback
            .is_empty()
        );
        handle.end_managed_work(&blocked_request_id);
        let observation_cursor =
            handle.begin_managed_work(RequestId::new("toolset-request-1"), None);
        let call = client
            .post(&endpoint)
            .header(AUTHORIZATION, &authorization)
            .header(ACCEPT, "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{
                    "name":"request_feedback",
                    "arguments":{
                        "title":"Review UI",
                        "instructions":"Judge the full launch experience.",
                        "actions":[{"id":"launch","instruction":"Launch a Session."}]
                    }
                }
            }))
            .send()
            .await
            .expect("tool call response");
        assert_eq!(call.status(), reqwest::StatusCode::OK);
        let body = call.text().await.expect("tool body");
        assert!(body.contains("toolset-request-1"), "{body}");
        assert!(body.contains("End this turn"), "{body}");
        let workbench = core
            .read_workbench(WorkbenchQuery {
                session_id: Some(launch.session_id.clone()),
            })
            .await
            .expect("workbench");
        assert_eq!(workbench.waiting_feedback.len(), 1);
        assert_eq!(
            workbench.waiting_feedback[0].request_id.as_str(),
            "toolset-request-1"
        );
        assert_eq!(
            handle.observations_since(observation_cursor),
            vec![SessionToolObservation::FeedbackRequested {
                request_id: RequestId::new("toolset-request-1")
            }]
        );
        handle.end_managed_work(&RequestId::new("toolset-request-1"));

        let draft = core
            .mutate_draft(DraftMutation::Save(SaveDraft {
                draft_id: DraftId::new("toolset-feedback-draft"),
                intent: RambleIntent::Feedback,
                session_id: Some(launch.session_id.clone()),
                request_id: Some(RequestId::new("toolset-request-1")),
                launch_configuration: None,
                document_json: "{}".to_owned(),
                body_markdown: "Uncooked fallback should not win.".to_owned(),
                expected_revision: 0,
            }))
            .await
            .expect("save feedback draft");
        let attachment_bytes = b"BINARY_ATTACHMENT_MUST_STAY_STRUCTURED".to_vec();
        let attachment_base64 = BASE64.encode(&attachment_bytes);
        core.resolve_feedback(ResolveFeedbackRequest::Submit(FeedbackSubmission {
            submission_id: SubmissionId::new("toolset-feedback-submission"),
            request_id: RequestId::new("toolset-request-1"),
            expected_draft_revision: draft.revision,
            submission_digest_assertion: None,
            document_json: "{}".to_owned(),
            uncooked_markdown: "Uncooked fallback should not win.".to_owned(),
            feedback_markdown:
                "Human task body: keep the Session alive and implement the reviewed change."
                    .to_owned(),
            cooking_model: None,
            artifacts: vec![ArtifactInput {
                display_name: "evidence.bin".to_owned(),
                media_type: "application/octet-stream".to_owned(),
                contents: attachment_bytes,
            }],
        }))
        .await
        .expect("resolve feedback");
        let get = client
            .post(&endpoint)
            .header(AUTHORIZATION, &authorization)
            .header(ACCEPT, "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc":"2.0",
                "id":4,
                "method":"tools/call",
                "params":{
                    "name":"get_feedback",
                    "arguments":{"request_id":"toolset-request-1"}
                }
            }))
            .send()
            .await
            .expect("get_feedback response");
        let get_body = get.text().await.expect("get_feedback body");
        let get_result = mcp_result(&get_body);
        let visible_text = get_result["content"][0]["text"]
            .as_str()
            .expect("human-readable tool content");
        assert!(
            visible_text.contains(
                "Human task body: keep the Session alive and implement the reviewed change."
            ),
            "{visible_text}"
        );
        assert!(!visible_text.contains("/Users/"), "{visible_text}");
        assert!(!visible_text.contains(&attachment_base64), "{visible_text}");
        assert_eq!(
            get_result["structuredContent"]["package"]["artifacts"][0]["locator"]["value"],
            attachment_base64
        );
        handle.shutdown().await.expect("toolset shutdown");
        store.close().await;
    }

    fn mcp_result(body: &str) -> Value {
        let json = body
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap_or(body);
        serde_json::from_str::<Value>(json)
            .expect("MCP JSON response")
            .get("result")
            .cloned()
            .expect("MCP result")
    }
}
