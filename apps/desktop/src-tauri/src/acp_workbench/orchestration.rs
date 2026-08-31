use std::{future::Future, pin::Pin};

use rambledesk_core::kernel::SessionId;

use super::model::{
    AcpWorkbenchError, AgentSummary, AttentionItem, LaunchPreflight, LaunchPreflightInput,
    PermissionAnswerInput, QuestionAnswerInput, SessionTimeline,
};

mod real;

pub(super) use real::AcpClientOrchestrator;

pub(super) type OrchestrationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, AcpWorkbenchError>> + Send + 'a>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct LiveAcpProjection {
    pub running_session_ids: Vec<String>,
    pub attention_items: Vec<AttentionItem>,
    pub agents: Vec<AgentSummary>,
    pub timelines: Vec<SessionTimeline>,
}

/// Desktop orchestration seam for `rambledesk-acp-client`.
///
/// The real Adapter owns process trees, ACP request correlation and live-event
/// accumulation. This port never stores transcript data and never owns durable
/// RambleDesk facts.
pub(super) trait AcpOrchestrationPort: Send + Sync {
    fn live_projection(&self) -> LiveAcpProjection;

    /// Starts the configured ACP Server, completes the ACP handshake, reads
    /// its capabilities, and shuts the probe down again. This intentionally
    /// does not require a user-selected workspace.
    fn connect<'a>(&'a self, agent_id: &'a str) -> OrchestrationFuture<'a, LaunchPreflight>;

    fn preflight<'a>(
        &'a self,
        input: &'a LaunchPreflightInput,
    ) -> OrchestrationFuture<'a, LaunchPreflight>;

    fn reconcile<'a>(&'a self, session_id: SessionId) -> OrchestrationFuture<'a, ()>;

    fn answer_permission<'a>(&'a self, input: PermissionAnswerInput)
    -> OrchestrationFuture<'a, ()>;

    fn answer_question<'a>(&'a self, input: QuestionAnswerInput) -> OrchestrationFuture<'a, ()>;

    fn shutdown(&self) -> OrchestrationFuture<'_, ()>;
}

#[cfg(test)]
pub(super) struct UnavailableAcpOrchestrator;

#[cfg(test)]
impl UnavailableAcpOrchestrator {
    fn unavailable<T>() -> Result<T, AcpWorkbenchError> {
        Err(AcpWorkbenchError::new(
            "ACP_CLIENT_UNAVAILABLE",
            "ACP Client is not available in this Desktop build",
            true,
        ))
    }
}

#[cfg(test)]
impl AcpOrchestrationPort for UnavailableAcpOrchestrator {
    fn live_projection(&self) -> LiveAcpProjection {
        LiveAcpProjection::default()
    }

    fn connect<'a>(&'a self, _agent_id: &'a str) -> OrchestrationFuture<'a, LaunchPreflight> {
        Box::pin(async { Self::unavailable() })
    }

    fn preflight<'a>(
        &'a self,
        _input: &'a LaunchPreflightInput,
    ) -> OrchestrationFuture<'a, LaunchPreflight> {
        Box::pin(async { Self::unavailable() })
    }

    fn reconcile<'a>(&'a self, _session_id: SessionId) -> OrchestrationFuture<'a, ()> {
        Box::pin(async { Self::unavailable() })
    }

    fn answer_permission<'a>(
        &'a self,
        _input: PermissionAnswerInput,
    ) -> OrchestrationFuture<'a, ()> {
        Box::pin(async { Self::unavailable() })
    }

    fn answer_question<'a>(&'a self, _input: QuestionAnswerInput) -> OrchestrationFuture<'a, ()> {
        Box::pin(async { Self::unavailable() })
    }

    fn shutdown(&self) -> OrchestrationFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
