use crate::{AcpConnection, AcpLaunch};
use async_trait::async_trait;
use rambledesk_core::{
    AgentConfig, AgentDriverError, AgentFailureReason, AgentFailureStage, AgentSessionCapabilities,
    AgentSessionConnection, AgentSessionDriver, AgentSessionLaunch, SessionManagement,
    StartedAgentSession,
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
}
impl AcpSessionDriver {
    pub fn with_feedback_companion(path: impl Into<PathBuf>) -> ConfiguredAcpSessionDriver {
        ConfiguredAcpSessionDriver {
            companion: path.into(),
        }
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
    feedback_workflow: Option<crate::feedback_workflow::FeedbackWorkflow>,
    cancelled: std::sync::atomic::AtomicBool,
    prompt_dispatch: Mutex<()>,
    observer: Arc<dyn rambledesk_core::AgentSessionObserver>,
}

#[async_trait]
impl AgentSessionDriver for AcpSessionDriver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        start(launch, None).await
    }
    async fn check(
        &self,
        config: &AgentConfig,
    ) -> Result<AgentSessionCapabilities, AgentDriverError> {
        check(config, None).await
    }
    async fn check_connection(
        &self,
        config: &AgentConfig,
    ) -> rambledesk_core::AgentConnectionCheck {
        crate::connection_check::check(config, None).await
    }
}
#[async_trait]
impl AgentSessionDriver for ConfiguredAcpSessionDriver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        start(launch, Some(&self.companion)).await
    }
    async fn check(
        &self,
        config: &AgentConfig,
    ) -> Result<AgentSessionCapabilities, AgentDriverError> {
        check(config, Some(&self.companion)).await
    }
    async fn check_connection(
        &self,
        config: &AgentConfig,
    ) -> rambledesk_core::AgentConnectionCheck {
        crate::connection_check::check(config, Some(&self.companion)).await
    }
}

async fn start(
    launch: AgentSessionLaunch,
    companion: Option<&Path>,
) -> Result<StartedAgentSession, AgentDriverError> {
    let mut trace = rambledesk_core::agent_operation_trace::AgentOperationTrace::new(
        "acp.start_session",
        Some(&launch.session.session_id),
    );
    let result = start_inner(launch, companion, &mut trace).await;
    trace.result(result, |_| "driver")
}

async fn start_inner(
    launch: AgentSessionLaunch,
    companion: Option<&Path>,
    trace: &mut rambledesk_core::agent_operation_trace::AgentOperationTrace,
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
    crate::agents::apply_managed_pi_defaults(&launch.config, &mut options.env).await;
    let feedback_workflow = launch
        .feedback
        .map(|endpoint| crate::feedback_workflow::inject(&mut options, endpoint, companion))
        .transpose()
        .map_err(|error| {
            AgentDriverError::classified(
                AgentFailureStage::Launch,
                AgentFailureReason::Configuration,
                error.message,
            )
        })?;
    let feedback_transport = feedback_workflow
        .as_ref()
        .map(|_| rambledesk_core::FeedbackTransport::Command);
    let observer = Arc::new(crate::observer::ManagedObserver {
        sink: launch.observer.clone(),
        remote: Mutex::new(None),
        local_session_id: launch.session.session_id.clone(),
    });
    let connection = AcpConnection::connect_observed(&options, observer.clone(), Some(trace))
        .await
        .map_err(|error| crate::failure::transport(error, AgentFailureStage::Initialize))?;
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        connection.open_session(&options, remote_session_id.as_deref()),
    )
    .await;
    let info = match result {
        Ok(Ok(info)) => info,
        other => {
            if other.is_err() {
                trace.finish("timed_out", "timeout");
            }
            let _ = connection.shutdown().await;
            return Err(match other {
                Ok(Err(error)) => crate::failure::transport(error, AgentFailureStage::Session),
                _ => AgentDriverError::classified(
                    AgentFailureStage::Session,
                    AgentFailureReason::Connection,
                    "ACP session creation or recovery timed out",
                ),
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
            feedback_workflow,
            cancelled: std::sync::atomic::AtomicBool::new(false),
            prompt_dispatch: Mutex::new(()),
            observer: launch.observer,
        }),
    })
}

async fn check(
    config: &AgentConfig,
    companion: Option<&Path>,
) -> Result<AgentSessionCapabilities, AgentDriverError> {
    let trace = rambledesk_core::agent_operation_trace::AgentOperationTrace::new(
        "acp.check",
        Some(&config.id),
    );
    let result = check_inner(config, companion, &trace).await;
    trace.result(result, |_| "driver")
}

async fn check_inner(
    config: &AgentConfig,
    companion: Option<&Path>,
    trace: &rambledesk_core::agent_operation_trace::AgentOperationTrace,
) -> Result<AgentSessionCapabilities, AgentDriverError> {
    let cwd = std::env::current_dir()
        .map_err(|_| AgentDriverError::new("Cannot determine the runtime working directory"))?;
    let mut options = options(config, cwd);
    crate::agents::apply_managed_pi_defaults(config, &mut options.env).await;
    let connection = AcpConnection::connect_observed(
        &options,
        Arc::new(crate::observer::CallbackObserver(Arc::new(|_| {}))),
        Some(trace),
    )
    .await
    .map_err(safe_error)?;
    let mut capabilities = connection.capabilities();
    let selected = companion
        .map(crate::feedback_transport::validate_companion)
        .transpose();
    connection.shutdown().await.map_err(safe_error)?;
    capabilities.feedback_transport =
        selected?.map(|_| rambledesk_core::FeedbackTransport::Command);
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
            .map_err(|error| {
                if error.failure.is_some() {
                    error
                } else {
                    AgentDriverError::classified(
                        AgentFailureStage::Configuration,
                        AgentFailureReason::Configuration,
                        error.message,
                    )
                }
            })
    }
    async fn cancel(&self) -> Result<(), AgentDriverError> {
        let _dispatch = self.prompt_dispatch.lock().expect("prompt dispatch");
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.permissions.cancel_all();
        self.sender
            .send_notification(agent_client_protocol::schema::v1::CancelNotification::new(
                self.remote.clone(),
            ))
            .map_err(|_| AgentDriverError::new("ACP cancellation failed"))
    }
    async fn respond_interaction(
        &self,
        request_id: &str,
        response: rambledesk_core::SessionInteractionResponse,
    ) -> Result<(), AgentDriverError> {
        self.permissions
            .respond_interaction(request_id, response)
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
        {
            let _dispatch = self.prompt_dispatch.lock().expect("prompt dispatch");
            self.cancelled
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if let Some(workflow) = &self.feedback_workflow {
            workflow.channel.close().await;
        }
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
        mut blocks: Vec<agent_client_protocol::schema::v1::ContentBlock>,
    ) -> Result<String, AgentDriverError> {
        use agent_client_protocol::schema::v1::{ContentBlock, TextContent};
        self.cancelled
            .store(false, std::sync::atomic::Ordering::SeqCst);
        // ACP has no standard system-prompt field. Runtime context accompanies
        // the actual prompt, never creates a turn, title, or user history row.
        if let Some(workflow) = &self.feedback_workflow {
            workflow.channel.begin_turn();
            blocks.insert(
                0,
                ContentBlock::Text(TextContent::new(crate::feedback_workflow::INSTRUCTIONS)),
            );
        }
        let mut result = self.send_blocks(blocks, false).await?;
        if let Some(workflow) = &self.feedback_workflow
            && result == "EndTurn"
            && !self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
            && !workflow.channel.receipt().handed_off
        {
            if !workflow.channel.receipt().attempted {
                self.observer
                    .observe(rambledesk_core::AgentSessionEvent::Activity {
                        kind: rambledesk_core::SessionActivityKind::Status,
                        text:
                            "Ramble handoff missing; asking the Agent to hand off its result once"
                                .into(),
                        tool_call_id: None,
                        append: false,
                    })
                    .await?;
                if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                    return Ok("Cancelled".into());
                }
                result = self
                    .send_blocks(
                        vec![ContentBlock::Text(TextContent::new(
                            crate::feedback_workflow::HANDOFF_REMINDER,
                        ))],
                        true,
                    )
                    .await?;
            }
            if result == "EndTurn"
                && !self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
                && !workflow.channel.receipt().handed_off
            {
                return Err(AgentDriverError::classified(
                    AgentFailureStage::Prompt,
                    AgentFailureReason::Unknown,
                    "The Agent ended without a Ramble handoff. Its answer remains in this conversation. Check the feedback command result and recover any existing request before retrying.",
                ));
            }
        }
        Ok(result)
    }

    async fn send_blocks(
        &self,
        blocks: Vec<agent_client_protocol::schema::v1::ContentBlock>,
        handoff_retry: bool,
    ) -> Result<String, AgentDriverError> {
        use agent_client_protocol::schema::v1::{PromptRequest, SessionId};
        // Queueing a retry and cancelling share a synchronous gate: cancellation
        // cannot slip between the last flag check and the protocol send.
        let pending = {
            let _dispatch = self.prompt_dispatch.lock().expect("prompt dispatch");
            if handoff_retry && self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                return Ok("Cancelled".into());
            }
            self.sender
                .send_request(PromptRequest::new(
                    SessionId::new(self.remote.clone()),
                    blocks,
                ))
                .block_task()
        };
        let result = pending.await;
        self.permissions.cancel_all();
        result
            .map(|response| format!("{:?}", response.stop_reason))
            .map_err(|error| crate::failure::protocol(AgentFailureStage::Prompt, error, false))
    }
}

pub(crate) fn options(config: &AgentConfig, cwd: std::path::PathBuf) -> AcpLaunch {
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
