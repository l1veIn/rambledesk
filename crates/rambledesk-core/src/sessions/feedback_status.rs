use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::*;

/// Request-side continuation state without loading the Agent conversation,
/// launch configuration, environment, or transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ManagedFeedbackStatus {
    pub session_id: String,
    pub deleting: bool,
    pub deliveries: Vec<FeedbackDelivery>,
}

impl SessionApplication {
    pub async fn get_feedback_status(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedFeedbackStatus, SessionError> {
        self.managed_record(&input.session_id).await?;
        let deleting = match &self.deletions {
            Some(repository) => {
                repository
                    .is_managed_session_deleting(&input.session_id)
                    .await?
            }
            None => false,
        };
        let deliveries = match &self.deliveries {
            Some(repository) => {
                repository
                    .list_session_deliveries(&input.session_id)
                    .await?
            }
            None => vec![],
        };
        Ok(ManagedFeedbackStatus {
            session_id: input.session_id,
            deleting,
            deliveries,
        })
    }
}
