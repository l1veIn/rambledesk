use async_trait::async_trait;

use super::{AgentDriverError, SessionManagement, SessionRecord};

/// Trusted controller identity for a feedback endpoint. It is never deserialized
/// from an Agent tool call; transport authentication selects the whole scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedFeedbackScope {
    pub session_id: String,
    pub host_id: String,
    pub host_session_id: String,
}

impl ManagedFeedbackScope {
    pub fn from_session(session: &SessionRecord) -> Result<Self, AgentDriverError> {
        if !matches!(session.management, SessionManagement::Managed { .. }) {
            return Err(AgentDriverError::new(
                "Feedback endpoints require a managed session",
            ));
        }
        Ok(Self {
            session_id: session.session_id.clone(),
            host_id: session.host_id.clone(),
            host_session_id: session.host_session_id.clone(),
        })
    }
}

/// A capability passed only to the owning Agent instance. No Debug/Serialize:
/// bearer credentials must not appear in diagnostics or application snapshots.
#[derive(Clone)]
pub struct ManagedFeedbackEndpoint {
    pub url: String,
    pub bearer_token: String,
}

#[async_trait]
pub trait ManagedFeedbackProvider: Send + Sync {
    /// Replaces any prior binding for the session with a fresh capability.
    async fn bind(
        &self,
        session: &SessionRecord,
    ) -> Result<ManagedFeedbackEndpoint, AgentDriverError>;
    /// Returns after prior admitted operations finish; no operation may be admitted
    /// through this binding afterwards, including existing MCP transport sessions.
    async fn revoke(&self, session_id: &str) -> Result<(), AgentDriverError>;
}
