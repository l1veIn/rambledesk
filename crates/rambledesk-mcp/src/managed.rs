use std::sync::Arc;

use rambledesk_core::{ActionInput, ContextRef, ManagedFeedbackScope, RequestAttachmentInput};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::{RwLock, RwLockReadGuard};

use super::*;
use crate::result::{managed_feedback_tool_result, structured_error_result};

/// Shared by every transport handler created under one binding. A read lease
/// covers each complete application operation; revoke waits for those operations.
pub struct ManagedMcpScope {
    identity: ManagedFeedbackScope,
    active: RwLock<bool>,
}

impl ManagedMcpScope {
    pub fn new(identity: ManagedFeedbackScope) -> Self {
        Self {
            identity,
            active: RwLock::new(true),
        }
    }
    pub async fn revoke(&self) {
        *self.active.write().await = false;
    }
    async fn lease(&self) -> Option<RwLockReadGuard<'_, bool>> {
        let lease = self.active.read().await;
        if *lease { Some(lease) } else { None }
    }
}

#[derive(Deserialize, JsonSchema)]
struct ManagedRequestInput {
    request_id: Option<String>,
    title: Option<String>,
    what_happened: String,
    actions: Vec<ActionInput>,
    #[serde(default)]
    context_refs: Vec<ContextRef>,
    #[serde(default)]
    attachments: Vec<RequestAttachmentInput>,
    #[serde(default)]
    source_hint: Option<String>,
    #[serde(default)]
    allow_finish: bool,
    #[serde(default)]
    final_summary: Option<String>,
}

impl From<ManagedRequestInput> for RequestFeedbackInput {
    fn from(input: ManagedRequestInput) -> Self {
        Self {
            request_id: input.request_id,
            host_id: None,
            host_session_id: String::new(),
            title: input.title,
            what_happened: input.what_happened,
            actions: input.actions,
            context_refs: input.context_refs,
            attachments: input.attachments,
            source_hint: input.source_hint,
            allow_finish: input.allow_finish,
            final_summary: input.final_summary,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
struct ManagedRecoverInput {
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Clone)]
pub struct ManagedRambleDeskMcp {
    tool_router: ToolRouter<Self>,
    application: FeedbackApplication,
    scope: Arc<ManagedMcpScope>,
}

impl ManagedRambleDeskMcp {
    pub fn new(application: FeedbackApplication, scope: Arc<ManagedMcpScope>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            application,
            scope,
        }
    }
}

fn revoked() -> CallToolResult {
    structured_error_result(
        "SCOPE_REVOKED",
        "This managed feedback binding has been revoked",
        false,
    )
}

#[tool_router]
impl ManagedRambleDeskMcp {
    #[tool(
        name = "request_feedback",
        description = "Create a durable feedback request for this managed session and return immediately. Session identity is fixed by RambleDesk; do not supply host or session IDs. Attach existing local files using attachments[].path. After creating the request, END THE CURRENT TURN. RambleDesk automatically continues this same Agent session after human feedback. Do not poll, wait on another tool, or ask for external confirmation. Reuse request_id for identical retries; a transport disconnect does not require a new feedback request. allow_finish is only for a final approval with final_summary, not for substantive feedback."
    )]
    async fn request_feedback(
        &self,
        Parameters(input): Parameters<ManagedRequestInput>,
    ) -> CallToolResult {
        let Some(_lease) = self.scope.lease().await else {
            return revoked();
        };
        managed_feedback_tool_result(
            &self.application,
            self.application
                .request_managed_feedback(&self.scope.identity, input.into())
                .await,
            false,
        )
        .await
    }

    #[tool(
        name = "get_feedback",
        description = "Read a feedback request belonging to this managed session. Use the original request_id when RambleDesk continues this Agent session or after a transport reconnect. Completed replies include the feedback package. If it is still waiting, end the current turn and let RambleDesk continue it; do not poll or open an external confirmation UI."
    )]
    async fn get_feedback(
        &self,
        Parameters(input): Parameters<GetFeedbackInput>,
    ) -> CallToolResult {
        let Some(_lease) = self.scope.lease().await else {
            return revoked();
        };
        managed_feedback_tool_result(
            &self.application,
            self.application
                .get_managed_feedback(&self.scope.identity, input)
                .await,
            true,
        )
        .await
    }

    #[tool(
        name = "recover_feedback",
        description = "Recover a durable request within this managed session without creating another request. Prefer the original request_id. Without it, recovery succeeds only when exactly one request belongs to the session; multiple matches require request_id. If waiting, end the turn and await RambleDesk continuation."
    )]
    async fn recover_feedback(
        &self,
        Parameters(input): Parameters<ManagedRecoverInput>,
    ) -> CallToolResult {
        let Some(_lease) = self.scope.lease().await else {
            return revoked();
        };
        managed_feedback_tool_result(
            &self.application,
            self.application
                .recover_managed_feedback(&self.scope.identity, input.request_id)
                .await,
            true,
        )
        .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ManagedRambleDeskMcp {
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let Some(_lease) = self.scope.lease().await else {
            return Err(ErrorData::invalid_request(
                "Managed feedback binding was revoked",
                None,
            ));
        };
        Ok(ListToolsResult {
            result_type: Some(ResultType::COMPLETE),
            tools: self.tool_router.list_all(),
            meta: None,
            next_cursor: None,
            ttl_ms: Some(0),
            cache_scope: Some(CacheScope::Private),
        })
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rambledesk-managed", env!("CARGO_PKG_VERSION")))
            .with_instructions("This feedback endpoint belongs to one RambleDesk managed session. Use request_feedback, get_feedback and recover_feedback only. After request_feedback, end the current Agent turn immediately. RambleDesk will automatically continue this same Agent context when human feedback is ready; then read the original request_id. Do not poll, use a blocking wait tool, ask for external confirmation, or create replacement requests after reconnects. Session identity is controller-owned. Cancellation and approval belong to the human in RambleDesk.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn revocation_drains_admitted_operations_and_denies_future_operations() {
        let scope = Arc::new(ManagedMcpScope::new(ManagedFeedbackScope {
            session_id: "session".into(),
            host_id: "dsh".into(),
            host_session_id: "session".into(),
        }));
        let operation = scope.lease().await.expect("admitted operation");
        let mut revocation = Box::pin(scope.revoke());
        tokio::select! {
            biased;
            _ = &mut revocation => panic!("revocation completed during admitted operation"),
            _ = std::future::ready(()) => {}
        }
        drop(operation);
        revocation.await;
        assert!(scope.lease().await.is_none());
        scope.revoke().await;
    }
}
