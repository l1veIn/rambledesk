use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, RwLock},
};

use rambledesk_acp_client::{
    AcpClient, AcpClientConfig, AcpClientError, AcpErrorCode, LaunchProfile, LaunchProfileRef,
    LiveSessionEvent, ManagedSessionSnapshot, PermissionAnswer, QuestionAction, QuestionAnswer,
    RunState, SessionScope,
};
use rambledesk_core::kernel::{Core, SessionId};
use serde_json::{Map, Value};
use tokio::{sync::broadcast, task::JoinHandle};

use super::{AcpOrchestrationPort, LiveAcpProjection, OrchestrationFuture};
use crate::acp_workbench::model::{
    AcpWorkbenchError, AgentSummary, AttentionItem, LaunchDraftInput, LaunchPreflight,
    PermissionAnswerInput, QuestionAnswerInput,
};

pub(crate) struct AcpClientOrchestrator {
    client: Arc<AcpClient>,
    profiles: HashMap<String, LaunchProfileRef>,
    projection: Arc<ProjectionStore>,
    pumps: Mutex<HashMap<String, JoinHandle<()>>>,
}

impl AcpClientOrchestrator {
    pub(crate) fn new(
        core: Arc<Core>,
        profiles: Vec<LaunchProfile>,
        agents: Vec<AgentSummary>,
    ) -> Result<Self, AcpWorkbenchError> {
        let profile_refs = profiles
            .iter()
            .map(|profile| {
                (
                    profile.profile_ref.agent_profile_id.clone(),
                    profile.profile_ref.clone(),
                )
            })
            .collect();
        let client = AcpClient::new(
            core,
            AcpClientConfig {
                profiles,
                ..AcpClientConfig::default()
            },
        )
        .map_err(map_client_error)?;
        Ok(Self {
            client: Arc::new(client),
            profiles: profile_refs,
            projection: Arc::new(ProjectionStore::new(agents)),
            pumps: Mutex::new(HashMap::new()),
        })
    }

    async fn reconcile_client(&self, session_id: SessionId) -> Result<(), AcpWorkbenchError> {
        let snapshot = self
            .client
            .reconcile(SessionScope {
                session_id: session_id.clone(),
            })
            .await
            .map_err(map_client_error)?;
        self.projection.apply_snapshot(snapshot);

        let needs_pump = {
            let mut pumps = self.pumps.lock().unwrap_or_else(|error| error.into_inner());
            pumps.retain(|_, task| !task.is_finished());
            !pumps.contains_key(session_id.as_str())
        };
        if !needs_pump {
            return Ok(());
        }

        let receiver = self
            .client
            .subscribe(session_id.clone())
            .map_err(map_client_error)?;
        // Refresh after subscribing. This closes the snapshot/subscription gap:
        // duplicate queued events are idempotent, while resolutions remove them.
        let refreshed = self
            .client
            .reconcile(SessionScope {
                session_id: session_id.clone(),
            })
            .await
            .map_err(map_client_error)?;
        self.projection.apply_snapshot(refreshed);

        let task = tokio::spawn(pump_events(
            self.client.clone(),
            self.projection.clone(),
            session_id.clone(),
            receiver,
        ));
        let mut pumps = self.pumps.lock().unwrap_or_else(|error| error.into_inner());
        if pumps.contains_key(session_id.as_str()) {
            task.abort();
        } else {
            pumps.insert(session_id.to_string(), task);
        }
        Ok(())
    }

    async fn probe_agent(&self, agent_id: &str) -> Result<LaunchPreflight, AcpWorkbenchError> {
        let profile_ref = self.profiles.get(agent_id).cloned().ok_or_else(|| {
            AcpWorkbenchError::new(
                "ACP_LAUNCH_PROFILE_NOT_FOUND",
                "the selected Agent has no configured ACP Launch Profile",
                false,
            )
        })?;
        let report = self
            .client
            .preflight(profile_ref)
            .await
            .map_err(map_client_error)?;
        if !report.available {
            return Err(AcpWorkbenchError::new(
                "ACP_AGENT_UNAVAILABLE",
                "the ACP Agent reported that it is unavailable",
                true,
            ));
        }
        let projected = project_preflight(agent_id, &report);
        self.projection.update_agent_options(
            agent_id,
            projected.models.clone(),
            projected.reasoning_efforts.clone(),
        );
        Ok(projected)
    }
}

impl AcpOrchestrationPort for AcpClientOrchestrator {
    fn live_projection(&self) -> LiveAcpProjection {
        self.projection.snapshot()
    }

    fn connect<'a>(&'a self, agent_id: &'a str) -> OrchestrationFuture<'a, LaunchPreflight> {
        Box::pin(async move { self.probe_agent(agent_id).await })
    }

    fn preflight<'a>(
        &'a self,
        input: &'a LaunchDraftInput,
    ) -> OrchestrationFuture<'a, LaunchPreflight> {
        Box::pin(async move { self.probe_agent(&input.agent_id).await })
    }

    fn reconcile<'a>(&'a self, session_id: SessionId) -> OrchestrationFuture<'a, ()> {
        Box::pin(async move { self.reconcile_client(session_id).await })
    }

    fn answer_permission<'a>(
        &'a self,
        input: PermissionAnswerInput,
    ) -> OrchestrationFuture<'a, ()> {
        Box::pin(async move {
            let session_id = self
                .projection
                .permission_session(&input.request_id)
                .ok_or_else(live_request_not_found)?;
            self.client
                .answer_permission(PermissionAnswer {
                    session_id: session_id.clone(),
                    live_request_id: input.request_id.clone(),
                    option_id: input.option_id,
                })
                .await
                .map_err(map_client_error)?;
            self.projection
                .resolve_permission(&session_id, &input.request_id);
            Ok(())
        })
    }

    fn answer_question<'a>(&'a self, input: QuestionAnswerInput) -> OrchestrationFuture<'a, ()> {
        Box::pin(async move {
            let binding = self
                .projection
                .question_binding(&input.request_id)
                .ok_or_else(live_request_not_found)?;
            let session_id = binding.session_id().clone();
            let request_id = input.request_id.clone();
            let answer = binding.answer(input)?;
            self.client
                .answer_question(answer)
                .await
                .map_err(map_client_error)?;
            self.projection.resolve_question(&session_id, &request_id);
            Ok(())
        })
    }

    fn shutdown(&self) -> OrchestrationFuture<'_, ()> {
        Box::pin(async move {
            self.client.shutdown().await.map_err(map_client_error)?;
            let tasks =
                std::mem::take(&mut *self.pumps.lock().unwrap_or_else(|error| error.into_inner()));
            for task in tasks.into_values() {
                task.abort();
            }
            self.projection.clear_runs();
            Ok(())
        })
    }
}

async fn pump_events(
    client: Arc<AcpClient>,
    projection: Arc<ProjectionStore>,
    session_id: SessionId,
    mut receiver: broadcast::Receiver<LiveSessionEvent>,
) {
    loop {
        match receiver.recv().await {
            Ok(event) => projection.apply_event(event),
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(%session_id, skipped, "ACP live event pump lagged; refreshing snapshot");
                match client
                    .reconcile(SessionScope {
                        session_id: session_id.clone(),
                    })
                    .await
                {
                    Ok(snapshot) => projection.apply_snapshot(snapshot),
                    Err(error) => {
                        tracing::warn!(%session_id, error = %error, "ACP live snapshot refresh failed");
                        projection.disconnect(&session_id);
                    }
                }
            }
            Err(broadcast::error::RecvError::Closed) => {
                projection.disconnect(&session_id);
                break;
            }
        }
    }
}

struct ProjectionStore {
    state: RwLock<ProjectionState>,
}

struct ProjectionState {
    agents: Vec<AgentSummary>,
    runs: HashMap<String, RunProjection>,
}

struct RunProjection {
    state: RunState,
    permissions: HashMap<String, ProjectedPermission>,
    questions: HashMap<String, ProjectedQuestion>,
}

struct ProjectedPermission {
    item: AttentionItem,
    queue_position: usize,
}

struct ProjectedQuestion {
    item: AttentionItem,
    binding: QuestionBinding,
    queue_position: usize,
}

#[derive(Clone)]
enum QuestionBinding {
    Select {
        session_id: SessionId,
        live_request_id: String,
        field_id: String,
        multiple: bool,
        choices: HashMap<String, Value>,
    },
    Unsupported {
        session_id: SessionId,
        live_request_id: String,
        reason: String,
    },
}

impl QuestionBinding {
    fn session_id(&self) -> &SessionId {
        match self {
            Self::Select { session_id, .. } | Self::Unsupported { session_id, .. } => session_id,
        }
    }

    fn answer(&self, input: QuestionAnswerInput) -> Result<QuestionAnswer, AcpWorkbenchError> {
        let (session_id, live_request_id) = match self {
            Self::Select {
                session_id,
                live_request_id,
                ..
            }
            | Self::Unsupported {
                session_id,
                live_request_id,
                ..
            } => (session_id.clone(), live_request_id.clone()),
        };
        if input.skipped {
            return Ok(QuestionAnswer {
                session_id,
                live_request_id,
                action: QuestionAction::Decline,
                content: None,
            });
        }
        match self {
            Self::Unsupported { reason, .. } => Err(AcpWorkbenchError::new(
                "UNSUPPORTED_ASK_QUESTION_SHAPE",
                reason,
                false,
            )),
            Self::Select {
                field_id,
                multiple,
                choices,
                ..
            } => {
                if (!multiple && input.choice_ids.len() != 1)
                    || (*multiple && input.choice_ids.is_empty())
                {
                    return Err(AcpWorkbenchError::new(
                        "INVALID_ACP_QUESTION_ANSWER",
                        if *multiple {
                            "the multi-select Ask Question requires at least one choice"
                        } else {
                            "the single-select Ask Question requires exactly one choice"
                        },
                        false,
                    ));
                }
                let mut selected = Vec::with_capacity(input.choice_ids.len());
                let mut unique = HashSet::new();
                for choice_id in &input.choice_ids {
                    if !unique.insert(choice_id) {
                        return Err(AcpWorkbenchError::new(
                            "INVALID_ACP_QUESTION_ANSWER",
                            "Ask Question choices must be unique",
                            false,
                        ));
                    }
                    selected.push(choices.get(choice_id).cloned().ok_or_else(|| {
                        AcpWorkbenchError::new(
                            "INVALID_ACP_QUESTION_ANSWER",
                            "the selected Ask Question choice was not offered by the Agent",
                            false,
                        )
                    })?);
                }
                let value = if *multiple {
                    Value::Array(selected)
                } else {
                    selected.pop().expect("single-select length was checked")
                };
                let mut content = Map::new();
                content.insert(field_id.clone(), value);
                Ok(QuestionAnswer {
                    session_id,
                    live_request_id,
                    action: QuestionAction::Accept,
                    content: Some(Value::Object(content)),
                })
            }
        }
    }
}

impl ProjectionStore {
    fn new(agents: Vec<AgentSummary>) -> Self {
        Self {
            state: RwLock::new(ProjectionState {
                agents,
                runs: HashMap::new(),
            }),
        }
    }

    fn snapshot(&self) -> LiveAcpProjection {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let mut running_session_ids = state
            .runs
            .iter()
            .filter(|(_, run)| is_connected(run.state))
            .map(|(session_id, _)| session_id.clone())
            .collect::<Vec<_>>();
        running_session_ids.sort();
        let mut ordered = Vec::new();
        for (session_id, run) in &state.runs {
            ordered.extend(run.permissions.values().map(|item| {
                (
                    session_id.clone(),
                    item.queue_position,
                    0_u8,
                    item.item.clone(),
                )
            }));
            ordered.extend(run.questions.values().map(|item| {
                (
                    session_id.clone(),
                    item.queue_position,
                    1_u8,
                    item.item.clone(),
                )
            }));
        }
        ordered.sort_by(|left, right| (&left.0, left.1, left.2).cmp(&(&right.0, right.1, right.2)));
        LiveAcpProjection {
            running_session_ids,
            attention_items: ordered.into_iter().map(|(_, _, _, item)| item).collect(),
            agents: state.agents.clone(),
        }
    }

    fn apply_snapshot(&self, snapshot: ManagedSessionSnapshot) {
        let created_at = now_rfc3339();
        let permissions = snapshot
            .permissions
            .iter()
            .map(|request| {
                (
                    request.live_request_id.clone(),
                    project_permission(request, &created_at),
                )
            })
            .collect();
        let questions = snapshot
            .questions
            .iter()
            .map(|question| {
                (
                    question.live_request_id.clone(),
                    project_question(question, &created_at),
                )
            })
            .collect();
        self.state
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .runs
            .insert(
                snapshot.session_id.to_string(),
                RunProjection {
                    state: snapshot.state,
                    permissions,
                    questions,
                },
            );
    }

    fn apply_event(&self, event: LiveSessionEvent) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        match event {
            LiveSessionEvent::StateChanged {
                session_id,
                state: run_state,
            } => {
                state
                    .runs
                    .entry(session_id.to_string())
                    .or_insert_with(empty_run)
                    .state = run_state;
            }
            LiveSessionEvent::SessionUpdate { .. } => {
                // Transcript and raw session updates are not Desktop history.
                // A future live-config DTO may project config_option_update here.
            }
            LiveSessionEvent::PermissionQueued { request } => {
                let session = state
                    .runs
                    .entry(request.session_id.to_string())
                    .or_insert_with(empty_run);
                session.state = RunState::WaitingForPermission;
                session.permissions.insert(
                    request.live_request_id.clone(),
                    project_permission(&request, &now_rfc3339()),
                );
            }
            LiveSessionEvent::PermissionResolved {
                session_id,
                live_request_id,
            } => {
                if let Some(session) = state.runs.get_mut(session_id.as_str()) {
                    session.permissions.remove(&live_request_id);
                }
            }
            LiveSessionEvent::QuestionQueued { question } => {
                let session = state
                    .runs
                    .entry(question.session_id.to_string())
                    .or_insert_with(empty_run);
                session.state = RunState::WaitingForQuestion;
                session.questions.insert(
                    question.live_request_id.clone(),
                    project_question(&question, &now_rfc3339()),
                );
            }
            LiveSessionEvent::QuestionResolved {
                session_id,
                live_request_id,
            } => {
                if let Some(session) = state.runs.get_mut(session_id.as_str()) {
                    session.questions.remove(&live_request_id);
                }
            }
            LiveSessionEvent::Disconnected { session_id, .. } => {
                disconnect_run(&mut state, &session_id);
            }
        }
    }

    fn permission_session(&self, request_id: &str) -> Option<SessionId> {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .runs
            .iter()
            .find(|(_, run)| run.permissions.contains_key(request_id))
            .map(|(session_id, _)| SessionId::new(session_id))
    }

    fn question_binding(&self, request_id: &str) -> Option<QuestionBinding> {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .runs
            .values()
            .find_map(|run| run.questions.get(request_id))
            .map(|question| question.binding.clone())
    }

    fn resolve_permission(&self, session_id: &SessionId, request_id: &str) {
        if let Some(run) = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .runs
            .get_mut(session_id.as_str())
        {
            run.permissions.remove(request_id);
        }
    }

    fn resolve_question(&self, session_id: &SessionId, request_id: &str) {
        if let Some(run) = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .runs
            .get_mut(session_id.as_str())
        {
            run.questions.remove(request_id);
        }
    }

    fn disconnect(&self, session_id: &SessionId) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        disconnect_run(&mut state, session_id);
    }

    fn clear_runs(&self) {
        self.state
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .runs
            .clear();
    }

    fn update_agent_options(
        &self,
        agent_id: &str,
        models: Vec<String>,
        reasoning_efforts: Vec<String>,
    ) {
        if let Some(agent) = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .agents
            .iter_mut()
            .find(|agent| agent.id == agent_id)
        {
            agent.models = models;
            agent.reasoning_efforts = reasoning_efforts;
        }
    }
}

fn empty_run() -> RunProjection {
    RunProjection {
        state: RunState::Ready,
        permissions: HashMap::new(),
        questions: HashMap::new(),
    }
}

fn disconnect_run(state: &mut ProjectionState, session_id: &SessionId) {
    let run = state
        .runs
        .entry(session_id.to_string())
        .or_insert_with(empty_run);
    run.state = RunState::Disconnected;
    run.permissions.clear();
    run.questions.clear();
}

fn is_connected(state: RunState) -> bool {
    !matches!(state, RunState::Stopped | RunState::Disconnected)
}

mod mapping;
#[cfg(test)]
mod tests;

use mapping::{now_rfc3339, project_permission, project_preflight, project_question};

fn live_request_not_found() -> AcpWorkbenchError {
    AcpWorkbenchError::new(
        "ACP_LIVE_REQUEST_NOT_FOUND",
        "the live ACP request is no longer waiting",
        true,
    )
}

fn map_client_error(error: AcpClientError) -> AcpWorkbenchError {
    let code = match error.code {
        AcpErrorCode::InvalidArgument => "ACP_INVALID_ARGUMENT",
        AcpErrorCode::LaunchProfileNotFound => "ACP_LAUNCH_PROFILE_NOT_FOUND",
        AcpErrorCode::SessionNotFound => "ACP_SESSION_NOT_FOUND",
        AcpErrorCode::SessionNotManaged => "ACP_SESSION_NOT_MANAGED",
        AcpErrorCode::RunDisconnected => "ACP_RUN_DISCONNECTED",
        AcpErrorCode::LiveRequestNotFound => "ACP_LIVE_REQUEST_NOT_FOUND",
        AcpErrorCode::LiveRequestNotCurrent => "ACP_LIVE_REQUEST_NOT_CURRENT",
        AcpErrorCode::InvalidLiveAnswer => "ACP_INVALID_LIVE_ANSWER",
        AcpErrorCode::UnsupportedCapability => "ACP_UNSUPPORTED_CAPABILITY",
        AcpErrorCode::SessionToolsetUnsupported => "ACP_SESSION_TOOLSET_UNSUPPORTED",
        AcpErrorCode::AuthenticationRequired => "ACP_AUTHENTICATION_REQUIRED",
        AcpErrorCode::UnsupportedAccessMode => "ACP_UNSUPPORTED_ACCESS_MODE",
        AcpErrorCode::AgentLaunchFailed => "ACP_AGENT_LAUNCH_FAILED",
        AcpErrorCode::ProtocolViolation => "ACP_PROTOCOL_VIOLATION",
        AcpErrorCode::RpcError => "ACP_RPC_ERROR",
        AcpErrorCode::OperationTimedOut => "ACP_OPERATION_TIMED_OUT",
        AcpErrorCode::CoreFailure => "ACP_CORE_FAILURE",
        AcpErrorCode::ShutdownFailed => "ACP_SHUTDOWN_FAILED",
        _ => "ACP_CLIENT_ERROR",
    };
    AcpWorkbenchError::new(code, error.message, error.retryable)
}
