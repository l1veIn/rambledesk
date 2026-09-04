use super::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListManagedSessionActivityInput {
    pub session_id: String,
    #[ts(type = "number")]
    pub before_sequence: u64,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ManagedSessionActivityPage {
    pub activities: Vec<SessionActivity>,
    pub has_more: bool,
}

impl SessionApplication {
    pub async fn list_activity_history(
        &self,
        input: ListManagedSessionActivityInput,
    ) -> Result<ManagedSessionActivityPage, SessionError> {
        self.managed_record(&input.session_id).await?;
        let limit = input.limit.unwrap_or(100);
        if limit == 0 || limit > 500 || input.before_sequence == 0 {
            return Err(SessionError::InvalidInput);
        }
        let mut activities = self
            .activities
            .list_session_activity_before(&input.session_id, input.before_sequence, limit + 1)
            .await?;
        let has_more = activities.len() > limit as usize;
        if has_more {
            activities.remove(0);
        }
        Ok(ManagedSessionActivityPage {
            activities,
            has_more,
        })
    }
}
