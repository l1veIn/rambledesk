mod configuration;
mod prompt_contract;
mod recovery;
mod runtime;

#[cfg(test)]
mod tests;

use std::{collections::HashMap, path::Path, sync::Arc};

use rambledesk_core::kernel::{
    AcpSessionLinkObservation, AgentObservation, Core, SessionId, SessionKind,
};
use serde_json::{Value, json};

use self::configuration::{
    apply_launch_configuration, apply_process_launch_configuration, project_config_options,
};
use self::recovery::build_recovery_prompt;
use self::runtime::ManagedRun;
use crate::launch_schema::project_launch_schema;
use crate::{
    AcpClientConfig, AcpClientError, AcpErrorCode, CancelOutcome, CapabilitySnapshot,
    LaunchProfile, LaunchProfileRef, LiveAnswerOutcome, LiveSessionEventReceiver,
    ManagedSessionSnapshot, PermissionAnswer, PreflightReport, QuestionAnswer, RecoveryMethod,
    SessionScope, ShutdownOutcome,
    process::{AgentSpawner, CommandAgentSpawner},
    rpc::{InboundMessage, RpcPeer},
    toolset::SessionToolsetHandle,
};

pub struct AcpClient {
    core: Arc<Core>,
    config: AcpClientConfig,
    profiles: HashMap<(String, String), LaunchProfile>,
    runs: std::sync::RwLock<HashMap<SessionId, Arc<ManagedRun>>>,
    spawner: Arc<dyn AgentSpawner>,
}

impl AcpClient {
    pub fn new(core: Arc<Core>, config: AcpClientConfig) -> Result<Self, AcpClientError> {
        Self::new_with_spawner(core, config, Arc::new(CommandAgentSpawner))
    }

    pub(crate) fn new_with_spawner(
        core: Arc<Core>,
        config: AcpClientConfig,
        spawner: Arc<dyn AgentSpawner>,
    ) -> Result<Self, AcpClientError> {
        if config.event_capacity == 0 {
            return Err(AcpClientError::invalid("event_capacity must be positive"));
        }
        let mut profiles = HashMap::new();
        for profile in &config.profiles {
            validate_profile(profile)?;
            let key = (
                profile.profile_ref.agent_profile_id.clone(),
                profile.profile_ref.launch_profile_id.clone(),
            );
            if profiles.insert(key, profile.clone()).is_some() {
                return Err(AcpClientError::invalid("duplicate Launch Profile identity"));
            }
        }
        Ok(Self {
            core,
            config,
            profiles,
            runs: std::sync::RwLock::new(HashMap::new()),
            spawner,
        })
    }

    pub async fn preflight(
        &self,
        profile_ref: LaunchProfileRef,
        workspace: &Path,
    ) -> Result<PreflightReport, AcpClientError> {
        let profile = self.profile(&profile_ref)?;
        let spawned = self.spawner.spawn(profile).await?;
        let (rpc, mut inbound) = RpcPeer::start(spawned);
        let inbound_rpc = rpc.clone();
        let inbound_task = tokio::spawn(async move {
            while let Some(message) = inbound.recv().await {
                if let InboundMessage::Request { id, .. } = message {
                    let _ = inbound_rpc
                        .respond_error(id, -32601, "Method not available during preflight")
                        .await;
                }
            }
        });
        let result = async {
            let (capabilities, agent_version) =
                initialize(&rpc, self.config.preflight_timeout).await?;
            let response = rpc
                .request(
                    "session/new",
                    json!({"cwd": workspace.to_string_lossy(), "mcpServers": []}),
                    Some(self.config.preflight_timeout),
                )
                .await?;
            let raw_config_options = config_options(&response);
            let (config_options, schema_digest) =
                project_launch_schema(profile, &raw_config_options);
            let mut warnings = Vec::new();
            if capabilities.close_session
                && let Some(session_id) = response.get("sessionId").and_then(Value::as_str)
                && rpc
                    .request(
                        "session/close",
                        json!({"sessionId": session_id}),
                        Some(self.config.operation_timeout),
                    )
                    .await
                    .is_err()
            {
                warnings.push(
                    "the temporary capability-probe Session did not confirm closure".to_owned(),
                );
            }
            Ok(PreflightReport {
                profile_ref,
                available: true,
                agent_version,
                capabilities,
                config_options,
                schema_digest,
                warnings,
            })
        }
        .await;
        let _ = rpc.shutdown(self.config.shutdown_grace).await;
        inbound_task.abort();
        result
    }

    pub async fn reconcile(
        &self,
        scope: SessionScope,
    ) -> Result<ManagedSessionSnapshot, AcpClientError> {
        let existing_run = {
            self.runs
                .read()
                .expect("run registry lock poisoned")
                .get(&scope.session_id)
                .cloned()
        };
        if let Some(run) = existing_run {
            if !run.connection_closed() {
                run.trigger_reconcile();
                return Ok(run.snapshot().await);
            }
            let removed = {
                let mut runs = self.runs.write().expect("run registry lock poisoned");
                if runs
                    .get(&scope.session_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &run))
                {
                    runs.remove(&scope.session_id);
                    true
                } else {
                    false
                }
            };
            if removed {
                run.shutdown().await?;
            } else {
                let current = self.run(&scope.session_id)?;
                current.trigger_reconcile();
                return Ok(current.snapshot().await);
            }
        }
        let recovery = self
            .core
            .read_session_recovery(scope.session_id.clone())
            .await
            .map_err(core_error)?;
        if recovery.session.kind != SessionKind::Managed {
            return Err(AcpClientError::new(
                AcpErrorCode::SessionNotManaged,
                "ACP reconcile requires a Managed Session",
                false,
            ));
        }
        let launch = recovery
            .session
            .launch_configuration
            .as_ref()
            .ok_or_else(|| {
                AcpClientError::new(
                    AcpErrorCode::CoreFailure,
                    "Managed Session omitted Launch Configuration",
                    false,
                )
            })?;
        let cwd = Path::new(&launch.workspace_reference);
        if !cwd.is_absolute() {
            return Err(AcpClientError::invalid(
                "Workspace Reference must be an absolute path",
            ));
        }
        let profile_ref = LaunchProfileRef {
            agent_profile_id: launch.agent_profile_id.clone(),
            launch_profile_id: launch.launch_profile_id.clone(),
        };
        let mut profile = self.profile(&profile_ref)?.clone();
        apply_process_launch_configuration(&mut profile, launch)?;
        let spawned = self.spawner.spawn(&profile).await?;
        let (rpc, inbound) = RpcPeer::start(spawned);
        let (capabilities, _) = match initialize(&rpc, self.config.operation_timeout).await {
            Ok(initialized) => initialized,
            Err(error) => {
                return Err(
                    cleanup_failed_setup(error, &rpc, None, self.config.shutdown_grace).await,
                );
            }
        };
        if profile.session_toolset == crate::SessionToolsetPolicy::Unsupported
            || !capabilities.mcp_http
        {
            let _ = rpc.shutdown(self.config.shutdown_grace).await;
            return Err(AcpClientError::new(
                AcpErrorCode::SessionToolsetUnsupported,
                "this ACP Agent cannot receive the RambleDesk Session Toolset required for structured Feedback Requests",
                false,
            ));
        }
        let toolset = match SessionToolsetHandle::start(self.core.clone(), scope.session_id.clone())
            .await
        {
            Ok(toolset) => toolset,
            Err(error) => {
                return Err(
                    cleanup_failed_setup(error, &rpc, None, self.config.shutdown_grace).await,
                );
            }
        };
        let mcp_servers = vec![toolset.mcp_server.clone()];
        let existing_id = recovery
            .current_acp_link
            .as_ref()
            .map(|link| link.acp_session_id.clone());
        let established = establish_session(
            &rpc,
            cwd,
            existing_id.as_deref(),
            &capabilities,
            &mcp_servers,
            self.config.operation_timeout,
        )
        .await;
        let (recovery_method, acp_session_id, mut config_options) = match established {
            Ok(established) => established,
            Err(error) => {
                return Err(cleanup_failed_setup(
                    error,
                    &rpc,
                    Some(toolset),
                    self.config.shutdown_grace,
                )
                .await);
            }
        };
        if recovery_method == RecoveryMethod::New {
            let configured = apply_launch_configuration(
                &rpc,
                &acp_session_id,
                &profile,
                launch,
                config_options,
                self.config.operation_timeout,
            )
            .await;
            config_options = match configured {
                Ok(configured) => configured,
                Err(error) => {
                    return Err(cleanup_failed_setup(
                        error,
                        &rpc,
                        Some(toolset),
                        self.config.shutdown_grace,
                    )
                    .await);
                }
            };
        }
        let capability_json = match serde_json::to_string(&capabilities) {
            Ok(serialized) => serialized,
            Err(error) => {
                return Err(cleanup_failed_setup(
                    AcpClientError::protocol(format!(
                        "could not serialize capability snapshot: {error}"
                    )),
                    &rpc,
                    Some(toolset),
                    self.config.shutdown_grace,
                )
                .await);
            }
        };
        let observed = self
            .core
            .record_agent_observation(AgentObservation::AcpSessionLinked(
                AcpSessionLinkObservation {
                    session_id: scope.session_id.clone(),
                    agent_profile_id: profile_ref.agent_profile_id,
                    launch_profile_id: profile_ref.launch_profile_id,
                    acp_session_id: acp_session_id.clone(),
                    capabilities_json: capability_json,
                    session_toolset_digest: toolset.digest.clone(),
                },
            ))
            .await;
        let link = match observed {
            Ok(link) => link,
            Err(error) => {
                return Err(cleanup_failed_setup(
                    core_error(error),
                    &rpc,
                    Some(toolset),
                    self.config.shutdown_grace,
                )
                .await);
            }
        };
        toolset.set_source_link(link.link_id).await;
        let recovery_prompt = (recovery_method == RecoveryMethod::New && existing_id.is_some())
            .then(|| build_recovery_prompt(&recovery));
        let run = Arc::new(ManagedRun::new(
            scope.session_id.clone(),
            acp_session_id,
            recovery_method,
            capabilities,
            config_options,
            recovery_prompt,
            rpc,
            toolset,
            self.config.event_capacity,
            self.config.shutdown_grace,
        ));
        self.runs
            .write()
            .expect("run registry lock poisoned")
            .insert(scope.session_id.clone(), run.clone());
        run.start_inbound(inbound);
        run.start_work_worker(self.core.clone());
        run.trigger_reconcile();
        Ok(run.snapshot().await)
    }

    pub async fn answer_permission(
        &self,
        answer: PermissionAnswer,
    ) -> Result<LiveAnswerOutcome, AcpClientError> {
        self.run(&answer.session_id)?
            .answer_permission(answer)
            .await
    }

    pub async fn answer_question(
        &self,
        answer: QuestionAnswer,
    ) -> Result<LiveAnswerOutcome, AcpClientError> {
        self.run(&answer.session_id)?.answer_question(answer).await
    }

    pub async fn cancel_turn(
        &self,
        session_id: SessionId,
    ) -> Result<CancelOutcome, AcpClientError> {
        self.run(&session_id)?.cancel_turn().await
    }

    pub fn subscribe(
        &self,
        session_id: SessionId,
    ) -> Result<LiveSessionEventReceiver, AcpClientError> {
        Ok(self.run(&session_id)?.subscribe())
    }

    pub async fn shutdown(&self) -> Result<ShutdownOutcome, AcpClientError> {
        let runs = std::mem::take(&mut *self.runs.write().expect("run registry lock poisoned"));
        let mut forced = 0;
        for run in runs.values() {
            forced += usize::from(run.shutdown().await?);
        }
        Ok(ShutdownOutcome {
            runs_stopped: runs.len(),
            forced_process_trees: forced,
        })
    }

    fn profile(&self, profile_ref: &LaunchProfileRef) -> Result<&LaunchProfile, AcpClientError> {
        self.profiles
            .get(&(
                profile_ref.agent_profile_id.clone(),
                profile_ref.launch_profile_id.clone(),
            ))
            .ok_or_else(|| {
                AcpClientError::new(
                    AcpErrorCode::LaunchProfileNotFound,
                    "Launch Profile is not configured",
                    false,
                )
            })
    }

    fn run(&self, session_id: &SessionId) -> Result<Arc<ManagedRun>, AcpClientError> {
        self.runs
            .read()
            .expect("run registry lock poisoned")
            .get(session_id)
            .cloned()
            .ok_or_else(|| {
                AcpClientError::new(
                    AcpErrorCode::SessionNotFound,
                    "Managed Agent Run is not active",
                    true,
                )
            })
    }
}

fn validate_profile(profile: &LaunchProfile) -> Result<(), AcpClientError> {
    if profile.profile_ref.agent_profile_id.trim().is_empty()
        || profile.profile_ref.launch_profile_id.trim().is_empty()
        || profile.command.as_os_str().is_empty()
    {
        return Err(AcpClientError::invalid(
            "Launch Profile identity and command must not be empty",
        ));
    }
    Ok(())
}

async fn initialize(
    rpc: &RpcPeer,
    timeout: std::time::Duration,
) -> Result<(CapabilitySnapshot, Option<String>), AcpClientError> {
    let response = rpc
        .request(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": {
                    "elicitation": {"form": {}},
                    "session": {"configOptions": {"boolean": {}}}
                },
                "clientInfo": {
                    "name": "rambledesk-acp-client",
                    "title": "RambleDesk",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
            Some(timeout),
        )
        .await?;
    let protocol_version = response
        .get("protocolVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| AcpClientError::protocol("initialize omitted protocolVersion"))?;
    if protocol_version != 1 {
        return Err(AcpClientError::new(
            AcpErrorCode::UnsupportedCapability,
            format!("Agent selected unsupported ACP version {protocol_version}"),
            false,
        ));
    }
    let raw = response
        .get("agentCapabilities")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let session = raw.get("sessionCapabilities").unwrap_or(&Value::Null);
    let mcp = raw.get("mcpCapabilities").unwrap_or(&Value::Null);
    let capabilities = CapabilitySnapshot {
        protocol_version: 1,
        load_session: raw
            .get("loadSession")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        resume_session: session.get("resume").is_some_and(|value| !value.is_null()),
        close_session: session.get("close").is_some_and(|value| !value.is_null()),
        mcp_http: mcp.get("http").and_then(Value::as_bool).unwrap_or(false),
        elicitation_form: true,
        raw_agent_capabilities: raw,
    };
    let agent_version = response.get("agentInfo").and_then(|info| {
        let name = info.get("name")?.as_str()?;
        let version = info.get("version").and_then(Value::as_str).unwrap_or("");
        Some(format!("{name} {version}").trim().to_string())
    });
    Ok((capabilities, agent_version))
}

async fn establish_session(
    rpc: &RpcPeer,
    cwd: &Path,
    existing_id: Option<&str>,
    capabilities: &CapabilitySnapshot,
    mcp_servers: &[Value],
    timeout: std::time::Duration,
) -> Result<(RecoveryMethod, String, Vec<Value>), AcpClientError> {
    let cwd = cwd.to_string_lossy();
    if let Some(session_id) = existing_id {
        if capabilities.resume_session {
            let response = rpc
                .request(
                    "session/resume",
                    json!({"sessionId": session_id, "cwd": cwd, "mcpServers": mcp_servers}),
                    Some(timeout),
                )
                .await;
            match response {
                Ok(response) => {
                    return Ok((
                        RecoveryMethod::Resume,
                        session_id.to_string(),
                        config_options(&response),
                    ));
                }
                Err(error) => {
                    tracing::warn!(
                        acp_session_id = session_id,
                        error = %error,
                        "session/resume failed; trying the next ACP recovery method"
                    );
                }
            }
        }
        if capabilities.load_session {
            let response = rpc
                .request(
                    "session/load",
                    json!({"sessionId": session_id, "cwd": cwd, "mcpServers": mcp_servers}),
                    Some(timeout),
                )
                .await;
            match response {
                Ok(response) => {
                    return Ok((
                        RecoveryMethod::Load,
                        session_id.to_string(),
                        config_options(&response),
                    ));
                }
                Err(error) => {
                    tracing::warn!(
                        acp_session_id = session_id,
                        error = %error,
                        "session/load failed; creating a replacement ACP Session"
                    );
                }
            }
        }
    }
    let response = rpc
        .request(
            "session/new",
            json!({"cwd": cwd, "mcpServers": mcp_servers}),
            Some(timeout),
        )
        .await?;
    let session_id = response
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AcpClientError::protocol("session/new omitted sessionId"))?;
    Ok((
        RecoveryMethod::New,
        session_id.to_string(),
        config_options(&response),
    ))
}

fn config_options(response: &Value) -> Vec<Value> {
    let mut options = response
        .get("configOptions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !has_option(&options, "model", "model")
        && let Some(model) = legacy_model_option(response)
    {
        options.push(model);
    }
    if !has_option(&options, "mode", "mode")
        && let Some(mode) = legacy_mode_option(response)
    {
        options.push(mode);
    }
    project_config_options(options)
}

fn has_option(options: &[Value], id: &str, category: &str) -> bool {
    options.iter().any(|option| {
        option.get("id").and_then(Value::as_str) == Some(id)
            || option.get("category").and_then(Value::as_str) == Some(category)
    })
}

fn legacy_model_option(response: &Value) -> Option<Value> {
    let models = response.get("models")?;
    let available = models.get("availableModels")?.as_array()?;
    let choices = available
        .iter()
        .filter_map(|model| {
            let value = model.get("modelId").and_then(Value::as_str)?;
            Some(json!({
                "value": value,
                "name": model.get("name").and_then(Value::as_str).unwrap_or(value),
                "description": model.get("description").and_then(Value::as_str)
            }))
        })
        .collect::<Vec<_>>();
    (!choices.is_empty()).then(|| {
        json!({
            "id": "model",
            "category": "model",
            "type": "select",
            "currentValue": models.get("currentModelId").and_then(Value::as_str),
            "options": choices,
            "_rambledeskMutation": "set_model"
        })
    })
}

fn legacy_mode_option(response: &Value) -> Option<Value> {
    let modes = response.get("modes")?;
    let available = modes.get("availableModes")?.as_array()?;
    let choices = available
        .iter()
        .filter_map(|mode| {
            let value = mode.get("id").and_then(Value::as_str)?;
            Some(json!({
                "value": value,
                "name": mode.get("name").and_then(Value::as_str).unwrap_or(value),
                "description": mode.get("description").and_then(Value::as_str)
            }))
        })
        .collect::<Vec<_>>();
    (!choices.is_empty()).then(|| {
        json!({
            "id": "mode",
            "category": "mode",
            "type": "select",
            "currentValue": modes.get("currentModeId").and_then(Value::as_str),
            "options": choices,
            "_rambledeskMutation": "set_mode"
        })
    })
}

async fn cleanup_failed_setup(
    original: AcpClientError,
    rpc: &RpcPeer,
    toolset: Option<SessionToolsetHandle>,
    shutdown_grace: std::time::Duration,
) -> AcpClientError {
    if let Some(toolset) = toolset
        && let Err(error) = toolset.shutdown().await
    {
        tracing::warn!(error = %error, "failed to stop Session Toolset after setup failure");
    }
    if let Err(error) = rpc.shutdown(shutdown_grace).await {
        tracing::warn!(error = %error, "failed to stop ACP process after setup failure");
    }
    original
}

fn core_error(error: rambledesk_core::kernel::CoreError) -> AcpClientError {
    let code = match error.code() {
        rambledesk_core::kernel::CoreErrorCode::SessionNotFound => AcpErrorCode::SessionNotFound,
        rambledesk_core::kernel::CoreErrorCode::SessionNotManaged => {
            AcpErrorCode::SessionNotManaged
        }
        _ => AcpErrorCode::CoreFailure,
    };
    AcpClientError::new(code, error.message(), error.retryable())
}
