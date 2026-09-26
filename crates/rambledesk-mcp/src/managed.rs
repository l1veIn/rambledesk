use std::sync::Arc;

use rambledesk_core::{ManagedFeedbackRecoverInput, ManagedFeedbackRequestInput};

pub use rambledesk_core::ManagedFeedbackBinding as ManagedMcpScope;

use super::*;
use crate::result::{managed_feedback_tool_result, structured_error_result};

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
        name = "list_workbenches",
        description = "Browse available capabilities without guessing search terms. Call with {} to see all four: ramble (free feedback), questions (one-by-one choices/custom answers), single_choice (choose one option), document_review (annotate an immutable script or speech). Returns purpose and result summaries; describe_workbench loads a selected schema. Optional offset/limit paginate."
    )]
    async fn list_workbenches(
        &self,
        Parameters(input): Parameters<rambledesk_core::ListWorkbenchesInput>,
    ) -> CallToolResult {
        let Some(_lease) = self.scope.lease().await else {
            return revoked();
        };
        crate::result::discovery_result(rambledesk_core::list_workbenches(&input))
    }

    #[tool(
        name = "describe_workbench",
        description = "Read one workbench type/version: input schema for workbench.data, example and result contract. Then call request_feedback with workbench and shared request materials; omit legacy actions when workbench is present."
    )]
    async fn describe_workbench(
        &self,
        Parameters(input): Parameters<rambledesk_core::DescribeWorkbenchInput>,
    ) -> CallToolResult {
        let Some(_lease) = self.scope.lease().await else {
            return revoked();
        };
        crate::result::discovery_result(rambledesk_core::describe_workbench(&input))
    }

    #[tool(
        name = "request_feedback",
        description = "Use list_workbenches and describe_workbench to choose a type; pass workbench with its data, or omit it for legacy Ramble actions. Create a durable feedback request for this managed session and return immediately. Session identity is fixed by RambleDesk; do not supply host or session IDs. Keep what_happened within 200 characters: it is the summary the human scans, and longer text is rejected; put the full explanation in a Markdown attachment. Attach existing local files using attachments[].path. After creating the request, END THE CURRENT TURN. RambleDesk automatically continues this same Agent session after human feedback. Do not poll, wait on another tool, or ask for external confirmation. Reuse request_id for identical retries; a transport disconnect does not require a new feedback request. allow_finish is only for a final approval with final_summary, not for substantive feedback."
    )]
    async fn request_feedback(
        &self,
        Parameters(input): Parameters<ManagedFeedbackRequestInput>,
    ) -> CallToolResult {
        let Some(lease) = self.scope.lease().await else {
            return revoked();
        };
        managed_feedback_tool_result(
            &self.application,
            self.application
                .request_managed_feedback(lease.scope(), input.into())
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
        let Some(lease) = self.scope.lease().await else {
            return revoked();
        };
        managed_feedback_tool_result(
            &self.application,
            self.application
                .get_managed_feedback(lease.scope(), input)
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
        Parameters(input): Parameters<ManagedFeedbackRecoverInput>,
    ) -> CallToolResult {
        let Some(lease) = self.scope.lease().await else {
            return revoked();
        };
        managed_feedback_tool_result(
            &self.application,
            self.application
                .recover_managed_feedback(lease.scope(), input.request_id)
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
            .with_instructions("This feedback endpoint belongs to one RambleDesk managed session. Use list_workbenches and describe_workbench to discover a type and its schema; create with request_feedback, read with get_feedback, recover with recover_feedback. Typed requests carry workbench: {type, version, data} and omit legacy actions. Structured results are in feedback_package.manifest.workbench.result. After request_feedback, end the current Agent turn immediately. RambleDesk will automatically continue this same Agent context when human feedback is ready; then read the original request_id. Do not poll, use a blocking wait tool, ask for external confirmation, or create replacement requests after reconnects. Session identity is controller-owned. Cancellation and approval belong to the human in RambleDesk.")
    }
}
