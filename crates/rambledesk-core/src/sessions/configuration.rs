//! Live, Agent-confirmed session options. Launch configuration remains separate.
use super::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionConfiguration {
    pub options: Vec<SessionConfigOption>,
    pub modes: Option<SessionModeCatalog>,
    pub models: Option<SessionModelCatalog>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionConfigOption {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub kind: SessionConfigKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionConfigKind {
    Select {
        current_value: String,
        options: Vec<SessionConfigChoice>,
    },
    Boolean {
        current_value: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionConfigChoice {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionModeCatalog {
    pub current_mode_id: String,
    pub available_modes: Vec<SessionMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionMode {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionModelCatalog {
    pub current_model_id: String,
    pub available_models: Vec<SessionModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionModel {
    pub model_id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionConfigValue {
    Select { value: String },
    Boolean { value: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionConfigChange {
    Option {
        config_id: String,
        value: SessionConfigValue,
    },
    Mode {
        mode_id: String,
    },
    Model {
        model_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SetManagedSessionConfigInput {
    pub session_id: String,
    pub change: SessionConfigChange,
}

impl SessionConfiguration {
    pub fn allows(&self, change: &SessionConfigChange) -> bool {
        match change {
            SessionConfigChange::Option { config_id, value } => self.options.iter().any(|option| {
                option.id == *config_id
                    && match (&option.kind, value) {
                        (
                            SessionConfigKind::Select { options, .. },
                            SessionConfigValue::Select { value },
                        ) => options.iter().any(|option| option.value == *value),
                        (SessionConfigKind::Boolean { .. }, SessionConfigValue::Boolean { .. }) => {
                            true
                        }
                        _ => false,
                    }
            }),
            SessionConfigChange::Mode { mode_id } => self
                .modes
                .as_ref()
                .is_some_and(|modes| modes.available_modes.iter().any(|mode| mode.id == *mode_id)),
            SessionConfigChange::Model { model_id } => self.models.as_ref().is_some_and(|models| {
                models
                    .available_models
                    .iter()
                    .any(|model| model.model_id == *model_id)
            }),
        }
    }

    pub fn confirms(&self, change: &SessionConfigChange) -> bool {
        match change {
            SessionConfigChange::Option { config_id, value } => self.options.iter().any(|option| {
                option.id == *config_id
                    && match (&option.kind, value) {
                        (
                            SessionConfigKind::Select { current_value, .. },
                            SessionConfigValue::Select { value },
                        ) => current_value == value,
                        (
                            SessionConfigKind::Boolean { current_value },
                            SessionConfigValue::Boolean { value },
                        ) => current_value == value,
                        _ => false,
                    }
            }),
            SessionConfigChange::Mode { mode_id } => self
                .modes
                .as_ref()
                .is_some_and(|modes| modes.current_mode_id == *mode_id),
            SessionConfigChange::Model { model_id } => self
                .models
                .as_ref()
                .is_some_and(|models| models.current_model_id == *model_id),
        }
    }
}

impl SessionApplication {
    pub async fn set_session_config(
        &self,
        input: SetManagedSessionConfigInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        self.managed_record(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        let mut interrupted = entry.interrupt.subscribe();
        let lifecycle = entry.lifecycle.lock().await;
        self.require_workable(&input.session_id).await?;
        if self.closing.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SessionError::ShuttingDown);
        }
        let live = entry.live.lock().await;
        if live.runtime.connection != SessionConnectionState::Connected {
            return Err(SessionError::NotConnected);
        }
        if live.runtime.activity != SessionActivityState::Idle {
            return Err(SessionError::Busy);
        }
        let connection = live.connection.clone().ok_or(SessionError::NotConnected)?;
        if connection.is_closed() {
            return Err(SessionError::NotConnected);
        }
        if !connection.configuration().allows(&input.change) {
            return Err(SessionError::InvalidInput);
        }
        drop(live);
        let result = tokio::select! {
            result = tokio::time::timeout(std::time::Duration::from_secs(30), connection.set_configuration(input.change)) => {
                result.map_err(|_| AgentDriverError::new("Agent configuration change timed out"))?.map_err(SessionError::from)
            }
            _ = interrupted.changed() => Err(SessionError::Interrupted),
        };
        drop(lifecycle);
        // Also refresh after a refusal: the Agent may have confirmed a different
        // value or removed an option. The snapshot always reflects its response.
        self.session_changed(&input.session_id);
        result?;
        self.get_session(ManagedSessionInput {
            session_id: input.session_id,
        })
        .await
    }
}
