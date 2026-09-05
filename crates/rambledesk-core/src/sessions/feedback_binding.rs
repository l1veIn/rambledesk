use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, RwLockReadGuard};

use crate::{ActionInput, ContextRef, RequestAttachmentInput, RequestFeedbackInput};

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

/// Shared admission boundary for every transport owned by one Agent instance.
/// Revocation waits for complete admitted operations, including result packaging.
pub struct ManagedFeedbackBinding {
    identity: ManagedFeedbackScope,
    active: RwLock<bool>,
}

impl ManagedFeedbackBinding {
    pub fn new(identity: ManagedFeedbackScope) -> Self {
        Self {
            identity,
            active: RwLock::new(true),
        }
    }

    pub async fn revoke(&self) {
        *self.active.write().await = false;
    }

    pub async fn lease(&self) -> Option<ManagedFeedbackLease<'_>> {
        let active = self.active.read().await;
        if !*active {
            return None;
        }
        Some(ManagedFeedbackLease {
            identity: &self.identity,
            _active: active,
        })
    }
}

/// Keep this lease alive until both the operation and its response are complete.
pub struct ManagedFeedbackLease<'a> {
    identity: &'a ManagedFeedbackScope,
    _active: RwLockReadGuard<'a, bool>,
}

impl ManagedFeedbackLease<'_> {
    pub fn scope(&self) -> &ManagedFeedbackScope {
        self.identity
    }
}

/// Agent-authored request content. Session ownership comes only from the binding.
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ManagedFeedbackRequestInput {
    pub request_id: Option<String>,
    pub title: Option<String>,
    pub what_happened: String,
    pub actions: Vec<ActionInput>,
    #[serde(default)]
    pub context_refs: Vec<ContextRef>,
    #[serde(default)]
    pub attachments: Vec<RequestAttachmentInput>,
    #[serde(default)]
    pub source_hint: Option<String>,
    #[serde(default)]
    pub allow_finish: bool,
    #[serde(default)]
    pub final_summary: Option<String>,
}

impl From<ManagedFeedbackRequestInput> for RequestFeedbackInput {
    fn from(input: ManagedFeedbackRequestInput) -> Self {
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

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ManagedFeedbackRecoverInput {
    #[serde(default)]
    pub request_id: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn revocation_drains_admitted_operations_and_denies_future_operations() {
        let binding = ManagedFeedbackBinding::new(ManagedFeedbackScope {
            session_id: "session".into(),
            host_id: "dsh".into(),
            host_session_id: "session".into(),
        });
        let operation = binding.lease().await.expect("admitted operation");
        assert_eq!(operation.scope().session_id, "session");
        let mut revocation = Box::pin(binding.revoke());
        tokio::select! {
            biased;
            _ = &mut revocation => panic!("revocation completed during admitted operation"),
            _ = std::future::ready(()) => {}
        }
        drop(operation);
        revocation.await;
        assert!(binding.lease().await.is_none());
        binding.revoke().await;
    }
}
