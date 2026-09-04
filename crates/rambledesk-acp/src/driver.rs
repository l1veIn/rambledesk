use crate::{AcpConnection, AcpLaunch};
use async_trait::async_trait;
use rambledesk_core::{
    AgentConfig, AgentDriverError, AgentSessionCapabilities, AgentSessionConnection,
    AgentSessionDriver, AgentSessionLaunch, SessionManagement, StartedAgentSession,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
pub struct AcpSessionDriver;

struct ManagedConnection {
    owned: Mutex<Option<AcpConnection>>,
    shutdown: tokio::sync::Mutex<()>,
    sender: agent_client_protocol::ConnectionTo<agent_client_protocol::Agent>,
    remote: String,
    permissions: Arc<crate::permissions::PermissionQueue>,
}

#[async_trait]
impl AgentSessionDriver for AcpSessionDriver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        let SessionManagement::Managed {
            cwd,
            remote_session_id,
            ..
        } = &launch.session.management
        else {
            return Err(AgentDriverError::new("ACP requires a managed session"));
        };
        let options = options(&launch.config, cwd.into());
        let observer = Arc::new(crate::observer::ManagedObserver {
            sink: launch.observer,
            remote: Mutex::new(None),
            local_session_id: launch.session.session_id.clone(),
        });
        let connection = AcpConnection::connect_observed(&options, observer.clone())
            .await
            .map_err(safe_error)?;
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
        Ok(StartedAgentSession {
            remote_session_id: info.remote_session_id.clone(),
            capabilities: AgentSessionCapabilities {
                load_session: info.load_session,
                resume_session: info.resume_session,
                http_mcp: info.http_mcp,
            },
            connection: Arc::new(ManagedConnection {
                owned: Mutex::new(Some(connection)),
                shutdown: tokio::sync::Mutex::new(()),
                sender,
                remote: info.remote_session_id,
                permissions,
            }),
        })
    }

    async fn check(
        &self,
        config: &AgentConfig,
    ) -> Result<AgentSessionCapabilities, AgentDriverError> {
        let cwd = std::env::current_dir()
            .map_err(|_| AgentDriverError::new("Cannot determine the runtime working directory"))?;
        let connection = AcpConnection::connect(&options(config, cwd), Arc::new(|_| {}))
            .await
            .map_err(safe_error)?;
        let capabilities = connection.capabilities();
        connection.shutdown().await.map_err(safe_error)?;
        Ok(capabilities)
    }
}

#[async_trait]
impl AgentSessionConnection for ManagedConnection {
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
        use agent_client_protocol::schema::v1::{
            ContentBlock, PromptRequest, SessionId, TextContent,
        };
        let result = self
            .sender
            .send_request(PromptRequest::new(
                SessionId::new(self.remote.clone()),
                vec![ContentBlock::Text(TextContent::new(text))],
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

fn options(config: &AgentConfig, cwd: std::path::PathBuf) -> AcpLaunch {
    AcpLaunch {
        command: config.command.clone(),
        args: config.args.clone(),
        env: config.env.clone(),
        cwd,
        mcp_servers: vec![],
    }
}
fn safe_error(error: crate::AcpError) -> AgentDriverError {
    AgentDriverError::new(error.to_string())
}
