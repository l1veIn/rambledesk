use agent_client_protocol::{
    Agent, ByteStreams, Client, ConnectionTo,
    schema::{ProtocolVersion, v1::*},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::{process::Child, sync::oneshot, task::JoinHandle};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[derive(Clone, Deserialize)]
pub struct AcpLaunch {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub cwd: PathBuf,
    #[serde(default)]
    pub mcp_servers: Vec<McpServer>,
}

#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    #[error("Invalid ACP launch: {0}")]
    InvalidLaunch(String),
    #[error("ACP process I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("ACP {0} failed; check the agent configuration and credentials")]
    Protocol(&'static str),
    #[error("ACP connection closed")]
    Closed,
    #[error("ACP {0} timed out")]
    Timeout(&'static str),
    #[error("Agent does not support loading the original session")]
    CannotLoad,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcpSessionInfo {
    pub remote_session_id: String,
    pub agent_name: Option<String>,
    pub agent_version: Option<String>,
    pub load_session: bool,
    pub resume_session: bool,
    pub http_mcp: bool,
}

/// Protocol notifications are kept inside the ACP package boundary. Application
/// integration maps them to its own activity types, never serializes SDK errors.
#[derive(Debug, Clone)]
pub enum AcpEvent {
    Update(Box<SessionNotification>),
    PermissionDeclined,
}

pub struct AcpConnection {
    connection: ConnectionTo<Agent>,
    child: Child,
    task: JoinHandle<Result<(), agent_client_protocol::Error>>,
    stop: Option<oneshot::Sender<()>>,
    stderr: JoinHandle<()>,
    initialized: InitializeResponse,
    remote_session_id: std::sync::Mutex<Option<String>>,
}

impl AcpConnection {
    pub async fn connect(
        launch: &AcpLaunch,
        observer: Arc<dyn Fn(AcpEvent) + Send + Sync>,
    ) -> Result<Self, AcpError> {
        let mut child =
            crate::process::spawn(&launch.command, &launch.args, &launch.env, &launch.cwd)?;
        let stdin = child.stdin.take().ok_or(AcpError::Closed)?;
        let stdout = child.stdout.take().ok_or(AcpError::Closed)?;
        let stderr = tokio::spawn(crate::process::drain_stderr(
            child.stderr.take().ok_or(AcpError::Closed)?,
        ));
        let (ready_tx, ready_rx) = oneshot::channel();
        let (stop, stopped) = oneshot::channel();
        let notification_observer = observer.clone();
        let task = tokio::spawn(async move {
            Client
                .builder()
                .name("rambledesk")
                .on_receive_notification(
                    async move |notification: SessionNotification, _| {
                        notification_observer(AcpEvent::Update(Box::new(notification)));
                        Ok(())
                    },
                    agent_client_protocol::on_receive_notification!(),
                )
                .on_receive_request(
                    async move |_: RequestPermissionRequest, responder, _| {
                        // Smoke probes never approve operations implicitly. The managed
                        // runtime installs an explicit permission queue in a later slice.
                        observer(AcpEvent::PermissionDeclined);
                        responder.respond(RequestPermissionResponse::new(
                            RequestPermissionOutcome::Cancelled,
                        ))
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .connect_with(
                    ByteStreams::new(stdin.compat_write(), stdout.compat()),
                    async move |cx| {
                        let result = cx
                            .send_request(InitializeRequest::new(ProtocolVersion::V1).client_info(
                                Implementation::new("rambledesk", env!("CARGO_PKG_VERSION")),
                            ))
                            .block_task()
                            .await;
                        match result {
                            Ok(initialized)
                                if initialized.protocol_version == ProtocolVersion::V1 =>
                            {
                                let _ = ready_tx.send(Ok((cx, initialized)));
                            }
                            Ok(_) => {
                                let _ =
                                    ready_tx.send(Err(AcpError::Protocol("protocol negotiation")));
                                return Ok(());
                            }
                            Err(error) => {
                                let _ = ready_tx.send(Err(AcpError::Protocol("initialize")));
                                return Err(error);
                            }
                        }
                        let _ = stopped.await;
                        Ok(())
                    },
                )
                .await
        });
        let result = tokio::time::timeout(Duration::from_secs(30), ready_rx).await;
        match result {
            Ok(Ok(Ok((connection, initialized)))) => Ok(Self {
                connection,
                child,
                task,
                stop: Some(stop),
                stderr,
                initialized,
                remote_session_id: std::sync::Mutex::new(None),
            }),
            other => {
                let _ = stop.send(());
                task.abort();
                let _ = task.await;
                let _ = child.kill().await;
                let _ = child.wait().await;
                stderr.abort();
                match other {
                    Err(_) => Err(AcpError::Timeout("initialize")),
                    Ok(Ok(Err(error))) => Err(error),
                    _ => Err(AcpError::Closed),
                }
            }
        }
    }

    pub fn process_id(&self) -> Option<u32> {
        self.child.id()
    }
    pub fn is_closed(&self) -> bool {
        self.task.is_finished()
    }

    pub async fn open_session(
        &self,
        launch: &AcpLaunch,
        remote: Option<&str>,
    ) -> Result<AcpSessionInfo, AcpError> {
        let remote_session_id = if let Some(remote) = remote {
            if self
                .initialized
                .agent_capabilities
                .session_capabilities
                .resume
                .is_some()
            {
                self.connection
                    .send_request(
                        ResumeSessionRequest::new(SessionId::new(remote), launch.cwd.clone())
                            .mcp_servers(launch.mcp_servers.clone()),
                    )
                    .block_task()
                    .await
                    .map_err(|_| AcpError::Protocol("session/resume"))?;
            } else if self.initialized.agent_capabilities.load_session {
                self.connection
                    .send_request(
                        LoadSessionRequest::new(SessionId::new(remote), launch.cwd.clone())
                            .mcp_servers(launch.mcp_servers.clone()),
                    )
                    .block_task()
                    .await
                    .map_err(|_| AcpError::Protocol("session/load"))?;
            } else {
                return Err(AcpError::CannotLoad);
            }
            remote.to_owned()
        } else {
            self.connection
                .send_request(
                    NewSessionRequest::new(launch.cwd.clone())
                        .mcp_servers(launch.mcp_servers.clone()),
                )
                .block_task()
                .await
                .map_err(|_| AcpError::Protocol("session/new"))?
                .session_id
                .to_string()
        };
        *self.remote_session_id.lock().expect("remote session lock") =
            Some(remote_session_id.clone());
        Ok(AcpSessionInfo {
            remote_session_id,
            agent_name: self
                .initialized
                .agent_info
                .as_ref()
                .map(|info| info.name.clone()),
            agent_version: self
                .initialized
                .agent_info
                .as_ref()
                .map(|info| info.version.clone()),
            load_session: self.initialized.agent_capabilities.load_session,
            resume_session: self
                .initialized
                .agent_capabilities
                .session_capabilities
                .resume
                .is_some(),
            http_mcp: self.initialized.agent_capabilities.mcp_capabilities.http,
        })
    }

    pub async fn prompt(&self, remote: &str, text: &str) -> Result<String, AcpError> {
        let response = self
            .connection
            .send_request(PromptRequest::new(
                SessionId::new(remote),
                vec![ContentBlock::Text(TextContent::new(text))],
            ))
            .block_task()
            .await
            .map_err(|_| AcpError::Protocol("session/prompt"))?;
        Ok(format!("{:?}", response.stop_reason))
    }

    pub fn cancel(&self, remote: &str) -> Result<(), AcpError> {
        self.connection
            .send_notification(CancelNotification::new(SessionId::new(remote)))
            .map_err(|_| AcpError::Closed)
    }

    pub async fn shutdown(mut self) -> Result<(), AcpError> {
        let remote = self
            .remote_session_id
            .lock()
            .expect("remote session lock")
            .clone();
        let mut close_result = Ok(());
        if self
            .initialized
            .agent_capabilities
            .session_capabilities
            .close
            .is_some()
            && !self.is_closed()
            && let Some(remote) = remote
        {
            // EOF alone is insufficient for agents which flush history on close.
            // Failure still cleans our resources, but is reported to the caller.
            close_result = match tokio::time::timeout(
                Duration::from_secs(10),
                self.connection
                    .send_request(CloseSessionRequest::new(SessionId::new(remote)))
                    .block_task(),
            )
            .await
            {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(_)) => Err(AcpError::Protocol("session/close")),
                Err(_) => Err(AcpError::Timeout("session/close")),
            };
        }
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if tokio::time::timeout(Duration::from_secs(2), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
            let _ = (&mut self.task).await;
        }
        let result = crate::process::reap(&mut self.child).await;
        self.stderr.abort();
        result.and(close_result)
    }
}

impl Drop for AcpConnection {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
        self.stderr.abort();
        // Child::kill_on_drop is a last resort. Explicit shutdown also reaps it.
    }
}
