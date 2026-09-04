//! Agent-confirmed selectors, adapted from Codeg connection.rs/types.rs at
//! 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1. Standard options are typed by the SDK;
//! legacy model catalogs remain an explicitly advertised compatibility surface.
use agent_client_protocol::{Agent, ConnectionTo, UntypedMessage, schema::v1 as acp};
use rambledesk_core::*;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

use crate::AcpError;
#[path = "session_configuration_mapping.rs"]
mod mapping;
pub(crate) type SharedConfiguration = Arc<Mutex<ConfigurationCache>>;

#[derive(Default)]
pub(crate) struct ConfigurationCache {
    pub state: SessionConfiguration,
    remote: Option<String>,
    early_remote: Option<String>,
    early_options: Option<Vec<SessionConfigOption>>,
    early_mode: Option<String>,
    mode_revision: u64,
}

impl ConfigurationCache {
    pub fn opened(
        &mut self,
        remote: &str,
        mut initial: SessionConfiguration,
    ) -> Result<(), AcpError> {
        if self
            .early_remote
            .as_deref()
            .is_some_and(|early| early != remote)
        {
            return Err(AcpError::Protocol("configuration attribution"));
        }
        if let Some(options) = self.early_options.take() {
            initial.options = options;
        }
        if let Some(mode) = self.early_mode.take() {
            apply_mode(&mut initial, mode);
        }
        self.state = initial;
        self.remote = Some(remote.into());
        Ok(())
    }

    pub fn observe(&mut self, notification: &acp::SessionNotification) -> Result<(), AcpError> {
        let (options, mode) = match &notification.update {
            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                (Some(mapping::options(&update.config_options)?), None)
            }
            acp::SessionUpdate::CurrentModeUpdate(update) => (
                None,
                Some(mapping::identifier(&update.current_mode_id.to_string())?),
            ),
            _ => return Ok(()),
        };
        let remote = notification.session_id.to_string();
        if self
            .remote
            .as_ref()
            .or(self.early_remote.as_ref())
            .is_some_and(|expected| expected != &remote)
        {
            return Err(AcpError::Protocol("configuration attribution"));
        }
        if self.remote.is_none() {
            self.early_remote = Some(remote);
            if options.is_some() {
                self.early_options = options;
            }
            if mode.is_some() {
                self.early_mode = mode;
            }
        } else {
            if let Some(options) = options {
                self.state.options = options;
            }
            if let Some(mode) = mode {
                apply_mode(&mut self.state, mode);
                self.mode_revision = self.mode_revision.wrapping_add(1);
            }
        }
        Ok(())
    }
}

fn apply_mode(state: &mut SessionConfiguration, id: String) {
    let modes = state.modes.get_or_insert_with(|| SessionModeCatalog {
        current_mode_id: id.clone(),
        available_modes: vec![],
    });
    modes.current_mode_id = id;
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenResponse {
    session_id: Option<String>,
    modes: Option<acp::SessionModeState>,
    config_options: Option<Vec<acp::SessionConfigOption>>,
    models: Option<mapping::LegacyModels>,
}

pub(crate) async fn open(
    sender: &ConnectionTo<Agent>,
    initialized: &acp::InitializeResponse,
    launch: &crate::AcpLaunch,
    remote: Option<&str>,
) -> Result<(String, SessionConfiguration), AcpError> {
    let (method, request) = match remote {
        Some(remote)
            if initialized
                .agent_capabilities
                .session_capabilities
                .resume
                .is_some() =>
        {
            (
                "session/resume",
                serde_json::to_value(
                    acp::ResumeSessionRequest::new(remote.to_owned(), launch.cwd.clone())
                        .mcp_servers(launch.mcp_servers.clone()),
                ),
            )
        }
        Some(remote) if initialized.agent_capabilities.load_session => (
            "session/load",
            serde_json::to_value(
                acp::LoadSessionRequest::new(remote.to_owned(), launch.cwd.clone())
                    .mcp_servers(launch.mcp_servers.clone()),
            ),
        ),
        Some(_) => return Err(AcpError::CannotLoad),
        None => (
            "session/new",
            serde_json::to_value(
                acp::NewSessionRequest::new(launch.cwd.clone())
                    .mcp_servers(launch.mcp_servers.clone()),
            ),
        ),
    };
    let request = UntypedMessage::new(method, request.map_err(|_| AcpError::Protocol(method))?)
        .map_err(|_| AcpError::Protocol(method))?;
    let raw = sender
        .send_request_to(Agent, request)
        .block_task()
        .await
        .map_err(|_| AcpError::Protocol(method))?;
    let response: OpenResponse =
        serde_json::from_value(raw).map_err(|_| AcpError::Protocol("session configuration"))?;
    let id = remote
        .map(str::to_owned)
        .or(response.session_id)
        .ok_or(AcpError::Protocol("session identity"))?;
    mapping::identifier(&id)?;
    let state = SessionConfiguration {
        options: mapping::options(response.config_options.as_deref().unwrap_or_default())?,
        modes: response.modes.map(mapping::modes).transpose()?,
        models: response.models.map(mapping::models).transpose()?,
    };
    Ok((id, state))
}

pub(crate) async fn set(
    sender: &ConnectionTo<Agent>,
    remote: &str,
    cache: &SharedConfiguration,
    change: SessionConfigChange,
) -> Result<(), AgentDriverError> {
    let mode_revision = {
        let cache = cache.lock().expect("configuration cache");
        if !cache.state.allows(&change) {
            return Err(AgentDriverError::new(
                "Agent does not advertise this configuration value",
            ));
        }
        cache.mode_revision
    };
    match &change {
        SessionConfigChange::Option { config_id, value } => {
            let value = match value {
                SessionConfigValue::Select { value } => {
                    acp::SessionConfigOptionValue::value_id(value.clone())
                }
                SessionConfigValue::Boolean { value } => {
                    acp::SessionConfigOptionValue::boolean(*value)
                }
            };
            let response = sender
                .send_request(acp::SetSessionConfigOptionRequest::new(
                    remote.to_owned(),
                    config_id.clone(),
                    value,
                ))
                .block_task()
                .await
                .map_err(|_| AgentDriverError::new("Agent rejected the configuration change"))?;
            let options = mapping::options(&response.config_options).map_err(|_| {
                AgentDriverError::new("Agent returned invalid configuration options")
            })?;
            cache.lock().expect("configuration cache").state.options = options;
        }
        SessionConfigChange::Mode { mode_id } => {
            sender
                .send_request(acp::SetSessionModeRequest::new(
                    remote.to_owned(),
                    mode_id.clone(),
                ))
                .block_task()
                .await
                .map_err(|_| AgentDriverError::new("Agent rejected the mode change"))?;
            let mut cache = cache.lock().expect("configuration cache");
            // Empty set_mode acknowledgement confirms the requested value unless
            // an explicit current_mode_update has supplied a more precise result.
            if cache.mode_revision == mode_revision {
                apply_mode(&mut cache.state, mode_id.clone());
            }
        }
        SessionConfigChange::Model { model_id } => {
            let request = UntypedMessage::new(
                "session/set_model",
                serde_json::json!({"sessionId":remote,"modelId":model_id}),
            )
            .map_err(|_| AgentDriverError::new("Unable to encode the model change"))?;
            sender
                .send_request_to(Agent, request)
                .block_task()
                .await
                .map_err(|_| AgentDriverError::new("Agent rejected the model change"))?;
            if let Some(models) = &mut cache.lock().expect("configuration cache").state.models {
                models.current_model_id = model_id.clone();
            }
        }
    }
    if !cache
        .lock()
        .expect("configuration cache")
        .state
        .confirms(&change)
    {
        return Err(AgentDriverError::new(
            "Agent confirmed a different configuration value",
        ));
    }
    Ok(())
}
