use crate::{AcpConnection, AcpLaunch};
use async_trait::async_trait;
use rambledesk_core::{
    AgentConfig, AgentDriverError, AgentSessionCapabilities, AgentSessionConnection,
    AgentSessionDriver, AgentSessionLaunch, SessionManagement, StartedAgentSession,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
pub struct AcpSessionDriver;

#[derive(Clone)]
pub struct ConfiguredAcpSessionDriver {
    companion: PathBuf,
    pi_extension_root: Option<PathBuf>,
}
impl AcpSessionDriver {
    pub fn with_feedback_companion(path: impl Into<PathBuf>) -> ConfiguredAcpSessionDriver {
        ConfiguredAcpSessionDriver {
            companion: path.into(),
            pi_extension_root: None,
        }
    }
}

impl ConfiguredAcpSessionDriver {
    pub fn with_pi_extension_root(mut self, path: impl Into<PathBuf>) -> Self {
        self.pi_extension_root = Some(path.into());
        self
    }
}

struct ManagedConnection {
    owned: Mutex<Option<AcpConnection>>,
    shutdown: tokio::sync::Mutex<()>,
    sender: agent_client_protocol::ConnectionTo<agent_client_protocol::Agent>,
    remote: String,
    permissions: Arc<crate::permissions::PermissionQueue>,
    configuration: crate::session_configuration::SharedConfiguration,
    prompt_capabilities: rambledesk_core::AgentPromptCapabilities,
}

#[async_trait]
impl AgentSessionDriver for AcpSessionDriver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        start(launch, None, None).await
    }
    async fn check(
        &self,
        config: &AgentConfig,
    ) -> Result<AgentSessionCapabilities, AgentDriverError> {
        check(config, None, None).await
    }
}
#[async_trait]
impl AgentSessionDriver for ConfiguredAcpSessionDriver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        start(
            launch,
            Some(&self.companion),
            self.pi_extension_root.as_deref(),
        )
        .await
    }
    async fn check(
        &self,
        config: &AgentConfig,
    ) -> Result<AgentSessionCapabilities, AgentDriverError> {
        check(
            config,
            Some(&self.companion),
            self.pi_extension_root.as_deref(),
        )
        .await
    }
}

async fn start(
    launch: AgentSessionLaunch,
    companion: Option<&Path>,
    pi_extension_root: Option<&Path>,
) -> Result<StartedAgentSession, AgentDriverError> {
    let SessionManagement::Managed {
        cwd,
        remote_session_id,
        ..
    } = &launch.session.management
    else {
        return Err(AgentDriverError::new("ACP requires a managed session"));
    };
    let mut options = options(&launch.config, cwd.into());
    let observer = Arc::new(crate::observer::ManagedObserver {
        sink: launch.observer,
        remote: Mutex::new(None),
        local_session_id: launch.session.session_id.clone(),
    });
    let mut connection = AcpConnection::connect_observed(&options, observer.clone())
        .await
        .map_err(safe_error)?;
    let selected = crate::pi_feedback::select(
        &launch.config,
        connection.capabilities().http_mcp,
        companion,
        pi_extension_root,
    )
    .await;
    let (feedback_transport, pi) = match selected {
        Ok(transport) => transport,
        Err(error) => {
            let _ = connection.shutdown().await;
            return Err(error);
        }
    };
    if let (Some(pi), Some(endpoint)) = (pi, launch.feedback.clone()) {
        // Capability discovery never creates an Agent session. Close that process
        // before spawning the private wrapper environment; cancellation drops the
        // one currently owned process at every await boundary.
        connection.shutdown().await.map_err(safe_error)?;
        pi.inject(&mut options, endpoint).await?;
        connection = AcpConnection::connect_observed(&options, observer.clone())
            .await
            .map_err(safe_error)?;
    } else if let Some(endpoint) = launch.feedback {
        let injected = feedback_transport
            .ok_or_else(|| {
                AgentDriverError::new("No managed feedback transport is available for this Agent")
            })
            .and_then(|transport| {
                crate::feedback_transport::server(transport, endpoint, companion)
            });
        match injected {
            Ok(server) => options.mcp_servers.push(server),
            Err(error) => {
                let _ = connection.shutdown().await;
                return Err(error);
            }
        }
    }
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        connection.open_session(&options, remote_session_id.as_deref()),
    )
    .await;
    let info = match result {
        Ok(Ok(info)) => info,
        other => {
            let _ = connection.shutdown().await;
            return Err(match other {
                Ok(Err(error)) => safe_error(error),
                _ => AgentDriverError::new("ACP session creation or recovery timed out"),
            });
        }
    };
    *observer.remote.lock().expect("remote attribution lock") =
        Some(info.remote_session_id.clone());
    let sender = connection.sender();
    let permissions = connection.permission_queue();
    let configuration = connection.configuration_cache();
    let mut capabilities = connection.capabilities();
    capabilities.feedback_transport = feedback_transport;
    let prompt_capabilities = capabilities.prompt.clone();
    Ok(StartedAgentSession {
        remote_session_id: info.remote_session_id.clone(),
        capabilities,
        connection: Arc::new(ManagedConnection {
            owned: Mutex::new(Some(connection)),
            shutdown: tokio::sync::Mutex::new(()),
            sender,
            remote: info.remote_session_id,
            permissions,
            configuration,
            prompt_capabilities,
        }),
    })
}

async fn check(
    config: &AgentConfig,
    companion: Option<&Path>,
    pi_extension_root: Option<&Path>,
) -> Result<AgentSessionCapabilities, AgentDriverError> {
    let cwd = std::env::current_dir()
        .map_err(|_| AgentDriverError::new("Cannot determine the runtime working directory"))?;
    let connection = AcpConnection::connect(&options(config, cwd), Arc::new(|_| {}))
        .await
        .map_err(safe_error)?;
    let mut capabilities = connection.capabilities();
    let selected =
        crate::pi_feedback::select(config, capabilities.http_mcp, companion, pi_extension_root)
            .await;
    connection.shutdown().await.map_err(safe_error)?;
    capabilities.feedback_transport = selected?.0;
    Ok(capabilities)
}

#[async_trait]
impl AgentSessionConnection for ManagedConnection {
    fn configuration(&self) -> rambledesk_core::SessionConfiguration {
        self.configuration
            .lock()
            .expect("configuration cache")
            .state
            .clone()
    }
    async fn set_configuration(
        &self,
        change: rambledesk_core::SessionConfigChange,
    ) -> Result<(), AgentDriverError> {
        crate::session_configuration::set(&self.sender, &self.remote, &self.configuration, change)
            .await
    }
    async fn cancel(&self) -> Result<(), AgentDriverError> {
        self.permissions.cancel_all();
        self.sender
            .send_notification(agent_client_protocol::schema::v1::CancelNotification::new(
                self.remote.clone(),
            ))
            .map_err(|_| AgentDriverError::new("ACP cancellation failed"))
    }
    async fn respond_permission(
        &self,
        request_id: &str,
        option_id: Option<&str>,
    ) -> Result<(), AgentDriverError> {
        self.permissions
            .respond(request_id, option_id)
            .map_err(safe_error)
    }
    async fn prompt(&self, text: &str) -> Result<String, AgentDriverError> {
        use agent_client_protocol::schema::v1::{ContentBlock, TextContent};
        self.send_prompt_blocks(vec![ContentBlock::Text(TextContent::new(text))])
            .await
    }
    async fn prompt_content(
        &self,
        blocks: &[rambledesk_core::SessionPromptContent],
    ) -> Result<String, AgentDriverError> {
        rambledesk_core::validate_prompt_content(blocks)?;
        if !rambledesk_core::prompt_content_supported(blocks, &self.prompt_capabilities) {
            return Err(AgentDriverError::new(
                "Agent does not support this prompt content",
            ));
        }
        self.send_prompt_blocks(crate::prompt_content::map(blocks))
            .await
    }
    fn is_closed(&self) -> bool {
        self.owned
            .lock()
            .expect("owned ACP instance lock")
            .as_ref()
            .is_none_or(AcpConnection::is_closed)
    }
    async fn stop(&self) -> Result<(), AgentDriverError> {
        let _serial = self.shutdown.lock().await;
        let owned = self.owned.lock().expect("owned ACP instance lock").take();
        if let Some(owned) = owned {
            owned.shutdown().await.map_err(safe_error)?;
        }
        Ok(())
    }
}

impl ManagedConnection {
    async fn send_prompt_blocks(
        &self,
        blocks: Vec<agent_client_protocol::schema::v1::ContentBlock>,
    ) -> Result<String, AgentDriverError> {
        use agent_client_protocol::schema::v1::{PromptRequest, SessionId};
        let result = self
            .sender
            .send_request(PromptRequest::new(
                SessionId::new(self.remote.clone()),
                blocks,
            ))
            .block_task()
            .await;
        self.permissions.cancel_all();
        result
            .map(|response| format!("{:?}", response.stop_reason))
            .map_err(|_| {
                AgentDriverError::new(
                    "ACP prompt failed; reconnect to the original session before continuing",
                )
            })
    }
}

fn options(config: &AgentConfig, cwd: std::path::PathBuf) -> AcpLaunch {
    AcpLaunch {
        command: config.command.clone(),
        args: config.args.clone(),
        env: crate::feedback_transport::public_environment(&config.env),
        cwd,
        mcp_servers: vec![],
    }
}
fn safe_error(error: crate::AcpError) -> AgentDriverError {
    AgentDriverError::new(error.to_string())
}
