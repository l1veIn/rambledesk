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
        let turn = entry.events.lock().await.turn_id.clone();
        let mut live = entry.live.lock().await;
        if live.runtime.activity == SessionActivityState::Idle {
            drop(live);
            return self.get_session(input).await;
        }
        let connection = live.connection.clone().ok_or(SessionError::NotConnected)?;
        let instance = live.runtime.instance_id.clone();
        live.permissions.clear();
        live.cancelling = true;
        live.runtime.activity = SessionActivityState::Running;
        drop(live);
        connection.cancel().await?;
        self.session_changed(&input.session_id);
        let app = self.clone();
        let session_id = input.session_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let same_turn = entry.events.lock().await.turn_id == turn;
            let live = entry.live.lock().await;
            let still_running = same_turn
                && live.runtime.instance_id == instance
                && live.runtime.activity != SessionActivityState::Idle;
            drop(live);
            if still_running {
                let _ = app
                    .stop_session(ManagedSessionInput {
                        session_id: session_id.clone(),
                    })
                    .await;
                entry.live.lock().await.runtime.last_error =
                    Some("Agent did not finish cancellation; its instance was stopped".into());
                app.session_changed(&session_id);
            }
        });
        self.get_session(input).await
    }
}
