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
        let connection = AcpConnection::connect(&options, Arc::new(|_| {}))
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
        Ok(StartedAgentSession {
            remote_session_id: info.remote_session_id,
            capabilities: AgentSessionCapabilities {
                load_session: info.load_session,
                resume_session: info.resume_session,
                http_mcp: info.http_mcp,
            },
            connection: Arc::new(ManagedConnection {
                owned: Mutex::new(Some(connection)),
                shutdown: tokio::sync::Mutex::new(()),
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
