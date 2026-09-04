use super::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionPermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionPermission {
    pub request_id: String,
    pub session_id: String,
    pub title: String,
    pub options: Vec<SessionPermissionOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RespondManagedPermissionInput {
    pub session_id: String,
    pub request_id: String,
    pub option_id: Option<String>,
}

impl SessionApplication {
    pub async fn respond_permission(
        &self,
        input: RespondManagedPermissionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        self.require_workable(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        let live = entry.live.lock().await;
        let permission = live
            .permissions
            .iter()
            .find(|permission| permission.request_id == input.request_id)
            .ok_or(SessionError::InvalidInput)?;
        if input.option_id.as_ref().is_some_and(|id| {
            !permission
                .options
                .iter()
                .any(|option| &option.option_id == id)
        }) {
            return Err(SessionError::InvalidInput);
        }
        let connection = live.connection.clone().ok_or(SessionError::NotConnected)?;
        drop(live);
        connection
            .respond_permission(&input.request_id, input.option_id.as_deref())
            .await?;
        let mut live = entry.live.lock().await;
        live.permissions
            .retain(|permission| permission.request_id != input.request_id);
        if live.permissions.is_empty()
            && live.runtime.activity == SessionActivityState::WaitingPermission
        {
            live.runtime.activity = SessionActivityState::Running;
        }
        drop(live);
        self.session_changed(&input.session_id);
        self.get_session(ManagedSessionInput {
            session_id: input.session_id,
        })
        .await
    }

    pub async fn cancel_prompt(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        self.managed_record(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        let events = entry.events.lock().await;
        let turn = events.turn_id.clone();
        let mut live = entry.live.lock().await;
        if live.runtime.activity == SessionActivityState::Idle {
            drop(live);
            drop(events);
            return self.get_session(input).await;
        }
        let connection = live.connection.clone().ok_or(SessionError::NotConnected)?;
        let instance = live.runtime.instance_id.clone();
        live.permissions.clear();
        live.cancelling = true;
        live.runtime.activity = SessionActivityState::Running;
        drop(live);
        drop(events);
        connection.cancel().await?;
        self.session_changed(&input.session_id);
        let app = self.clone();
        let session_id = input.session_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if let (Some(instance), Some(turn)) = (instance, turn) {
                let _ = app.stop_if_current(&session_id, &instance, &turn).await;
            }
        });
        self.get_session(input).await
    }
}
