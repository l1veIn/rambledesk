use async_trait::async_trait;
use thiserror::Error;

use super::{
    AgentWorkBatch, AgentWorkRecordOutcome, FactMutation, FactMutationOutcome, FactQuery,
    FactQueryOutcome, StoredBlob, StoredWorkResult, WorkClaim,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FactStoreError {
    #[error("stable id conflicts with existing content")]
    IdempotencyConflict,
    #[error("session was not found")]
    SessionNotFound,
    #[error("operation requires a managed session")]
    SessionNotManaged,
    #[error("session has pending Feedback or Agent work")]
    SessionHasPendingActivity,
    #[error("ACP session link was not found for the session")]
    AcpSessionLinkNotFound,
    #[error("feedback request was not found")]
    RequestNotFound,
    #[error("feedback request is terminal")]
    RequestTerminal,
    #[error("draft revision conflicts")]
    DraftConflict,
    #[error("agent work was not found")]
    WorkNotFound,
    #[error("agent work claim conflicts")]
    WorkClaimConflict,
    #[error("stored facts are corrupt")]
    CorruptData,
    #[error("fact storage failed")]
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ArtifactStoreError {
    #[error("artifact was not found")]
    NotFound,
    #[error("artifact digest does not match")]
    DigestMismatch,
    #[error("artifact storage failed")]
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PutArtifact {
    pub contents: Vec<u8>,
    /// Canonical form: `sha256:` plus 64 lowercase hexadecimal digits.
    pub expected_sha256: String,
}

#[async_trait]
pub trait FactStore: Send + Sync {
    async fn apply(&self, mutation: FactMutation) -> Result<FactMutationOutcome, FactStoreError>;

    async fn query(&self, query: FactQuery) -> Result<FactQueryOutcome, FactStoreError>;

    async fn claim_work(&self, claim: WorkClaim) -> Result<AgentWorkBatch, FactStoreError>;

    async fn record_work(
        &self,
        result: StoredWorkResult,
    ) -> Result<AgentWorkRecordOutcome, FactStoreError>;
}

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    async fn put(&self, artifact: PutArtifact) -> Result<StoredBlob, ArtifactStoreError>;

    async fn open_verified(
        &self,
        storage_key: &str,
        expected_sha256: &str,
    ) -> Result<Vec<u8>, ArtifactStoreError>;
}
