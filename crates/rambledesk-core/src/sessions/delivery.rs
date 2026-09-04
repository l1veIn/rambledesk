use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::SessionRepositoryError;
use crate::FeedbackResolution;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum FeedbackDeliveryState {
    Pending,
    Sending,
    Delivered,
    Uncertain,
    Discarded,
}

impl FeedbackDeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sending => "sending",
            Self::Delivered => "delivered",
            Self::Uncertain => "uncertain",
            Self::Discarded => "discarded",
        }
    }
}

impl TryFrom<&str> for FeedbackDeliveryState {
    type Error = SessionRepositoryError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "sending" => Ok(Self::Sending),
            "delivered" => Ok(Self::Delivered),
            "uncertain" => Ok(Self::Uncertain),
            "discarded" => Ok(Self::Discarded),
            _ => Err(SessionRepositoryError::CorruptData),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ResolveDeliveryAction {
    Retry,
    Acknowledge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FeedbackDelivery {
    pub request_id: String,
    pub session_id: String,
    pub resolution: FeedbackResolution,
    pub state: FeedbackDeliveryState,
    pub attempt_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_error: Option<String>,
}

#[async_trait]
pub trait FeedbackDeliveryRepository: Send + Sync {
    async fn list_session_deliveries(
        &self,
        session_id: &str,
    ) -> Result<Vec<FeedbackDelivery>, SessionRepositoryError>;

    async fn list_pending_deliveries(
        &self,
    ) -> Result<Vec<FeedbackDelivery>, SessionRepositoryError>;

    /// Claims a pending record before any send attempt. None means another worker
    /// already claimed it, it is terminal/uncertain, or the record no longer exists.
    async fn claim_delivery(
        &self,
        request_id: &str,
        attempt_id: &str,
        now: &str,
    ) -> Result<Option<FeedbackDelivery>, SessionRepositoryError>;

    /// Requires the current sending attempt. Pending is allowed only with evidence
    /// that nothing was sent; ambiguous failures must use Uncertain. Identical
    /// completion retries are idempotent until a newer attempt is claimed.
    async fn finish_delivery(
        &self,
        request_id: &str,
        attempt_id: &str,
        state: FeedbackDeliveryState,
        last_error: Option<&str>,
        now: &str,
    ) -> Result<FeedbackDelivery, SessionRepositoryError>;

    /// Startup recovery never automatically retries an attempt with unknown outcome.
    async fn recover_interrupted_deliveries(
        &self,
        now: &str,
    ) -> Result<u64, SessionRepositoryError>;

    async fn discard_session_deliveries(
        &self,
        session_id: &str,
        now: &str,
    ) -> Result<u64, SessionRepositoryError>;

    /// Only an explicit user decision may retry or acknowledge an uncertain send.
    async fn resolve_delivery(
        &self,
        request_id: &str,
        session_id: &str,
        action: ResolveDeliveryAction,
        now: &str,
    ) -> Result<FeedbackDelivery, SessionRepositoryError>;
}
