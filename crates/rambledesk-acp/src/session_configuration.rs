//! Agent-confirmed selectors, adapted from Codeg connection.rs/types.rs at
//! 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1. Standard options are typed by the SDK;
//! legacy model catalogs remain an explicitly advertised compatibility surface.
use agent_client_protocol::{Agent, ConnectionTo, UntypedMessage, schema::v1 as acp};
use rambledesk_core::*;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

use crate::AcpError;
#[path = "session_configuration_cache.rs"]
mod cache;
#[path = "session_configuration_extension.rs"]
mod extension;
#[path = "session_configuration_mapping.rs"]
mod mapping;
use cache::Route;
pub(crate) use cache::{ConfigurationCache, InitialConfiguration};
pub(crate) type SharedConfiguration = Arc<Mutex<ConfigurationCache>>;

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
) -> Result<(String, InitialConfiguration), AcpError> {
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
        .map_err(|error| AcpError::protocol(method, error))?;
    let response: OpenResponse = serde_json::from_value(raw.clone())
        .map_err(|_| AcpError::Protocol("session configuration"))?;
    let id = remote
        .map(str::to_owned)
        .or(response.session_id)
        .ok_or(AcpError::Protocol("session identity"))?;
    mapping::identifier(&id)?;
    let standard = mapping::options(response.config_options.as_deref().unwrap_or_default())?;
    let extension = if standard.is_empty() {
        extension::Configuration::parse(&raw)?
    } else {
        None
    };
    Ok((
        id,
        InitialConfiguration {
            standard,
            modes: response.modes.map(mapping::modes).transpose()?,
            models: response.models.map(mapping::models).transpose()?,
            extension,
        },
    ))
}

pub(crate) async fn set(
    sender: &ConnectionTo<Agent>,
    remote: &str,
    cache: &SharedConfiguration,
    change: SessionConfigChange,
) -> Result<(), AgentDriverError> {
    let (route, mode_revision, extension_request, model) = {
        let cache = cache.lock().expect("configuration cache");
        if !cache.state.allows(&change) {
            return Err(AgentDriverError::new(
                "Agent does not advertise this configuration value",
            ));
        }
        let route = cache
            .route(&change.config_id)
            .cloned()
            .ok_or_else(|| AgentDriverError::new("Unknown session configuration option"))?;
        let request = if let Route::Extension(id) = &route {
            Some(
                cache
                    .initial
                    .extension
                    .as_ref()
                    .and_then(|extension| extension.request(remote, id, &change.value))
                    .ok_or_else(|| {
                        AgentDriverError::new("Unsupported configuration extension value")
                    })?,
            )
        } else {
            None
        };
        let model = cache.state.options.iter().any(|option| {
            option.id == change.config_id && option.category.as_deref() == Some("model")
        });
        (route, cache.mode_revision, request, model)
    };
    match route {
        Route::Standard(config_id) => {
            let value = match &change.value {
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
                    config_id,
                    value,
                ))
                .block_task()
                .await
                .map_err(|error| {
                    crate::failure::protocol(AgentFailureStage::Configuration, error, model)
                })?;
            let options = mapping::options(&response.config_options).map_err(|_| {
                AgentDriverError::new("Agent returned invalid configuration options")
            })?;
            let mut cache = cache.lock().expect("configuration cache");
            cache.initial.standard = options;
            cache.refresh();
        }
        Route::Mode | Route::Model => {
            let SessionConfigValue::Select { value } = &change.value else {
                return Err(AgentDriverError::new("Expected a configuration choice"));
            };
            if route == Route::Mode {
                sender
                    .send_request(acp::SetSessionModeRequest::new(
                        remote.to_owned(),
                        value.clone(),
                    ))
                    .block_task()
                    .await
                    .map_err(|error| {
                        crate::failure::protocol(AgentFailureStage::Configuration, error, false)
                    })?;
                let mut cache = cache.lock().expect("configuration cache");
                // Empty ACK confirms the request unless a notification gave a more precise result.
                if cache.mode_revision == mode_revision {
                    cache.apply_mode(value.clone());
                }
                cache.refresh();
            } else {
                let request = UntypedMessage::new(
                    "session/set_model",
                    serde_json::json!({"sessionId":remote,"modelId":value}),
                )
                .map_err(|_| AgentDriverError::new("Unable to encode the model change"))?;
                sender
                    .send_request_to(Agent, request)
                    .block_task()
                    .await
                    .map_err(|error| {
                        crate::failure::protocol(AgentFailureStage::Configuration, error, true)
                    })?;
                let mut cache = cache.lock().expect("configuration cache");
                if let Some(models) = &mut cache.initial.models {
                    models.current = value.clone();
                }
                cache.refresh();
            }
        }
        Route::Extension(id) => {
            let request = extension_request.expect("validated extension request");
            let request = UntypedMessage::new(request.method, request.params)
                .map_err(|_| AgentDriverError::new("Unable to encode the configuration change"))?;
            sender
                .send_request_to(Agent, request)
                .block_task()
                .await
                .map_err(|error| {
                    crate::failure::protocol(AgentFailureStage::Configuration, error, model)
                })?;
            let mut cache = cache.lock().expect("configuration cache");
            if let Some(extension) = &mut cache.initial.extension {
                extension.confirmed(&id, &change.value);
            }
            cache.refresh();
        }
    }
    if !cache
        .lock()
        .expect("configuration cache")
        .state
        .confirms(&change)
    {
        return Err(AgentDriverError::classified(
            AgentFailureStage::Configuration,
            if model {
                AgentFailureReason::Model
            } else {
                AgentFailureReason::Configuration
            },
            "Agent confirmed a different configuration value",
        ));
    }
    Ok(())
}
