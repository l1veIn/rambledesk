use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::*;

/// Request-side execution and continuation state without loading the Agent
/// conversation, launch configuration, environment, or transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ManagedFeedbackStatus {
    pub session_id: String,
    pub deleting: bool,
    pub connection: SessionConnectionState,
    pub activity: SessionActivityState,
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
        // Observe only an existing in-memory owner. A read must not create a
        // runtime entry, recover a session, or invoke any Agent configuration.
        let entry = self.entries.lock().await.get(&input.session_id).cloned();
        let (connection, activity) = match entry {
            Some(entry) => {
                let live = entry.live.lock().await;
                if live.runtime.connection == SessionConnectionState::Connected
                    && live
                        .connection
                        .as_ref()
                        .is_some_and(|connection| connection.is_closed())
                {
                    // Transport closure can precede the runtime worker's next
                    // reconciliation. Report it without performing cleanup.
                    (
                        SessionConnectionState::Disconnected,
                        SessionActivityState::Idle,
                    )
                } else {
                    (live.runtime.connection, live.runtime.activity)
                }
            }
            None => (SessionConnectionState::Stopped, SessionActivityState::Idle),
        };
        Ok(ManagedFeedbackStatus {
            session_id: input.session_id,
            deleting,
            connection,
            activity,
            deliveries,
        })
    }
}
