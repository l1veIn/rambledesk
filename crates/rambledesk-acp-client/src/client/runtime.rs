use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use rambledesk_core::kernel::{
    AgentWorkDisposition, AgentWorkEvidence, AgentWorkPayload, AgentWorkResult, Core, SessionId,
    WorkScope,
};
use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify, broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::{
    AcpClientError, AcpErrorCode, AskQuestion, CancelOutcome, CapabilitySnapshot,
    LiveAnswerOutcome, LiveSessionEvent, LiveSessionEventReceiver, ManagedSessionSnapshot,
    PermissionAnswer, PermissionOption, PermissionRequest, QuestionAnswer, RecoveryMethod,
    RunState,
    elicitation::{self, ElicitationPlan},
    rpc::{InboundMessage, RpcPeer},
    toolset::SessionToolsetHandle,
};

struct PendingPermission {
    request: PermissionRequest,
    wire_id: Value,
    kind: PermissionResponderKind,
}

enum PermissionResponderKind {
    Acp,
    Elicitation { persist_in_content: bool },
}

struct PendingQuestion {
    question: AskQuestion,
    wire_id: Value,
    schema: Value,
}

struct LiveState {
    state: RunState,
    permissions: VecDeque<PendingPermission>,
    questions: VecDeque<PendingQuestion>,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
            state: RunState::Ready,
            permissions: VecDeque::new(),
            questions: VecDeque::new(),
        }
    }
}

pub(super) struct ManagedRun {
    session_id: SessionId,
    acp_session_id: String,
    recovery_method: RecoveryMethod,
    capabilities: CapabilitySnapshot,
    config_options: Vec<Value>,
    recovery_prompt: Mutex<Option<String>>,
    rpc: Arc<RpcPeer>,
    toolset: Mutex<Option<SessionToolsetHandle>>,
    live: Mutex<LiveState>,
    events: broadcast::Sender<LiveSessionEvent>,
    shutdown_grace: std::time::Duration,
    consumed_deliveries: Mutex<HashSet<String>>,
    work_worker_started: AtomicBool,
    work_wake: Notify,
    work_stop: CancellationToken,
    work_task: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl ManagedRun {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        session_id: SessionId,
        acp_session_id: String,
        recovery_method: RecoveryMethod,
        capabilities: CapabilitySnapshot,
        config_options: Vec<Value>,
        recovery_prompt: Option<String>,
        rpc: Arc<RpcPeer>,
        toolset: SessionToolsetHandle,
        event_capacity: usize,
        shutdown_grace: std::time::Duration,
    ) -> Self {
        let (events, _) = broadcast::channel(event_capacity);
        Self {
            session_id,
            acp_session_id,
            recovery_method,
            capabilities,
            config_options,
            recovery_prompt: Mutex::new(recovery_prompt),
            rpc,
            toolset: Mutex::new(Some(toolset)),
            live: Mutex::new(LiveState::default()),
            events,
            shutdown_grace,
            consumed_deliveries: Mutex::new(HashSet::new()),
            work_worker_started: AtomicBool::new(false),
            work_wake: Notify::new(),
            work_stop: CancellationToken::new(),
            work_task: std::sync::Mutex::new(None),
        }
    }

    pub(super) fn subscribe(&self) -> LiveSessionEventReceiver {
        self.events.subscribe()
    }

    pub(super) fn connection_closed(&self) -> bool {
        self.rpc.is_closed()
    }

    pub(super) fn start_inbound(self: &Arc<Self>, mut inbound: mpsc::Receiver<InboundMessage>) {
        let run = self.clone();
        tokio::spawn(async move {
            while let Some(message) = inbound.recv().await {
                match message {
                    InboundMessage::Request { id, method, params } => {
                        run.handle_request(id, &method, params).await;
                    }
                    InboundMessage::Notification { method, params } => {
                        run.handle_notification(&method, params).await;
                    }
                    InboundMessage::Disconnected { reason } => {
                        let mut live = run.live.lock().await;
                        live.state = RunState::Disconnected;
                        live.permissions.clear();
                        live.questions.clear();
                        drop(live);
                        let _ = run.events.send(LiveSessionEvent::Disconnected {
                            session_id: run.session_id.clone(),
                            reason,
                        });
                    }
                }
            }
        });
    }

    async fn handle_request(&self, id: Value, method: &str, params: Value) {
        match method {
            "session/request_permission" => {
                if let Err(error) = self.enqueue_permission(id.clone(), params).await {
                    let _ = self.rpc.respond_error(id, -32602, &error.message).await;
                }
            }
            "elicitation/create" => {
                if let Err(error) = self.enqueue_elicitation(id.clone(), params).await {
                    let _ = self.rpc.respond_error(id, -32602, &error.message).await;
                }
            }
            _ => {
                let _ = self.rpc.respond_error(id, -32601, "Method not found").await;
            }
        }
    }

    async fn handle_notification(&self, method: &str, params: Value) {
        if method != "session/update" {
            return;
        }
        if params.get("sessionId").and_then(Value::as_str) != Some(self.acp_session_id.as_str()) {
            return;
        }
        let update = params.get("update").cloned().unwrap_or(Value::Null);
        observe_feedback_consumption(&update, &self.consumed_deliveries).await;
        let _ = self.events.send(LiveSessionEvent::SessionUpdate {
            session_id: self.session_id.clone(),
            update,
        });
    }

    async fn enqueue_permission(
        &self,
        wire_id: Value,
        params: Value,
    ) -> Result<(), AcpClientError> {
        self.ensure_wire_session(&params)?;
        let raw_options = params
            .get("options")
            .and_then(Value::as_array)
            .ok_or_else(|| AcpClientError::protocol("permission omitted options"))?;
        if raw_options.is_empty() || raw_options.len() > 16 {
            return Err(AcpClientError::protocol(
                "permission must provide 1..=16 options",
            ));
        }
        let options = raw_options
            .iter()
            .map(|value| {
                Ok(PermissionOption {
                    option_id: required_string(value, "optionId")?,
                    name: required_string(value, "name")?,
                    kind: required_string(value, "kind")?,
                })
            })
            .collect::<Result<Vec<_>, AcpClientError>>()?;
        let mut live = self.live.lock().await;
        let live_request_id = uuid::Uuid::now_v7().to_string();
        let request = PermissionRequest {
            live_request_id,
            session_id: self.session_id.clone(),
            tool_call: params.get("toolCall").cloned().unwrap_or(Value::Null),
            request_meta: params.get("_meta").cloned().unwrap_or(Value::Null),
            options,
            queue_position: live.permissions.len(),
        };
        live.permissions.push_back(PendingPermission {
            request: request.clone(),
            wire_id,
            kind: PermissionResponderKind::Acp,
        });
        live.state = RunState::WaitingForPermission;
        let _ = self
            .events
            .send(LiveSessionEvent::PermissionQueued { request });
        Ok(())
    }

    async fn enqueue_elicitation(
        &self,
        wire_id: Value,
        params: Value,
    ) -> Result<(), AcpClientError> {
        self.ensure_wire_session(&params)?;
        let request_meta = params.get("_meta").cloned().unwrap_or(Value::Null);
        let mut live = self.live.lock().await;
        let id = uuid::Uuid::now_v7().to_string();
        let plan = elicitation::classify(
            &params,
            self.session_id.clone(),
            id.clone(),
            live.questions.len(),
        )?;
        match plan {
            ElicitationPlan::Decline => {
                drop(live);
                self.rpc
                    .respond_result(wire_id, json!({"action": "decline"}))
                    .await?;
            }
            ElicitationPlan::Question { question, schema } => {
                live.questions.push_back(PendingQuestion {
                    question: question.clone(),
                    wire_id,
                    schema,
                });
                live.state = RunState::WaitingForQuestion;
                let _ = self
                    .events
                    .send(LiveSessionEvent::QuestionQueued { question });
            }
            ElicitationPlan::Approval {
                message,
                tool_call_id,
                options,
                persist_in_content,
            } => {
                let request = PermissionRequest {
                    live_request_id: id,
                    session_id: self.session_id.clone(),
                    tool_call: json!({
                        "toolCallId": tool_call_id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
                        "title": message,
                        "kind": "execute",
                        "status": "pending"
                    }),
                    request_meta,
                    options,
                    queue_position: live.permissions.len(),
                };
                live.permissions.push_back(PendingPermission {
                    request: request.clone(),
                    wire_id,
                    kind: PermissionResponderKind::Elicitation { persist_in_content },
                });
                live.state = RunState::WaitingForPermission;
                let _ = self
                    .events
                    .send(LiveSessionEvent::PermissionQueued { request });
            }
        }
        Ok(())
    }

    pub(super) async fn answer_permission(
        &self,
        answer: PermissionAnswer,
    ) -> Result<LiveAnswerOutcome, AcpClientError> {
        let mut live = self.live.lock().await;
        let Some(front) = live.permissions.front() else {
            return Err(live_not_found());
        };
        if front.request.live_request_id != answer.live_request_id {
            return Err(live_not_current());
        }
        if !front
            .request
            .options
            .iter()
            .any(|option| option.option_id == answer.option_id)
        {
            return Err(AcpClientError::new(
                AcpErrorCode::InvalidLiveAnswer,
                "permission option was not provided by the Agent",
                false,
            ));
        }
        let pending = live.permissions.pop_front().expect("front exists");
        reindex_permissions(&mut live.permissions);
        let remaining = live.permissions.len();
        live.state = projected_state(&live);
        drop(live);
        let result = match pending.kind {
            PermissionResponderKind::Acp => json!({
                "outcome": {"outcome": "selected", "optionId": answer.option_id}
            }),
            PermissionResponderKind::Elicitation { persist_in_content } => {
                elicitation::approval_response(&answer.option_id, persist_in_content)
            }
        };
        self.rpc.respond_result(pending.wire_id, result).await?;
        let _ = self.events.send(LiveSessionEvent::PermissionResolved {
            session_id: self.session_id.clone(),
            live_request_id: answer.live_request_id.clone(),
        });
        Ok(LiveAnswerOutcome {
            live_request_id: answer.live_request_id,
            accepted: true,
            remaining,
        })
    }

    pub(super) async fn answer_question(
        &self,
        answer: QuestionAnswer,
    ) -> Result<LiveAnswerOutcome, AcpClientError> {
        let mut live = self.live.lock().await;
        let Some(front) = live.questions.front() else {
            return Err(live_not_found());
        };
        if front.question.live_request_id != answer.live_request_id {
            return Err(live_not_current());
        }
        let response =
            elicitation::question_response(&front.schema, answer.action, answer.content.clone())?;
        let pending = live.questions.pop_front().expect("front exists");
        reindex_questions(&mut live.questions);
        let remaining = live.questions.len();
        live.state = projected_state(&live);
        drop(live);
        self.rpc.respond_result(pending.wire_id, response).await?;
        let _ = self.events.send(LiveSessionEvent::QuestionResolved {
            session_id: self.session_id.clone(),
            live_request_id: answer.live_request_id.clone(),
        });
        Ok(LiveAnswerOutcome {
            live_request_id: answer.live_request_id,
            accepted: true,
            remaining,
        })
    }

    pub(super) async fn cancel_turn(&self) -> Result<CancelOutcome, AcpClientError> {
        let cancelled = self.cancel_live_requests().await;
        self.rpc
            .notify("session/cancel", json!({"sessionId": self.acp_session_id}))
            .await?;
        Ok(CancelOutcome {
            session_id: self.session_id.clone(),
            notification_sent: true,
            live_requests_cancelled: cancelled,
        })
    }

    async fn cancel_live_requests(&self) -> usize {
        let (permissions, questions) = {
            let mut live = self.live.lock().await;
            live.state = RunState::Ready;
            (
                std::mem::take(&mut live.permissions),
                std::mem::take(&mut live.questions),
            )
        };
        let count = permissions.len() + questions.len();
        for pending in permissions {
            let result = match pending.kind {
                PermissionResponderKind::Acp => json!({"outcome": {"outcome": "cancelled"}}),
                PermissionResponderKind::Elicitation { .. } => json!({"action": "cancel"}),
            };
            let _ = self.rpc.respond_result(pending.wire_id, result).await;
        }
        for pending in questions {
            let _ = self
                .rpc
                .respond_result(pending.wire_id, json!({"action": "cancel"}))
                .await;
        }
        count
    }

    pub(super) async fn snapshot(&self) -> ManagedSessionSnapshot {
        let live = self.live.lock().await;
        ManagedSessionSnapshot {
            session_id: self.session_id.clone(),
            acp_session_id: self.acp_session_id.clone(),
            recovery_method: self.recovery_method,
            capabilities: self.capabilities.clone(),
            config_options: self.config_options.clone(),
            state: live.state,
            permissions: live
                .permissions
                .iter()
                .map(|pending| pending.request.clone())
                .collect(),
            questions: live
                .questions
                .iter()
                .map(|pending| pending.question.clone())
                .collect(),
        }
    }

    pub(super) fn start_work_worker(self: &Arc<Self>, core: Arc<Core>) {
        if self.work_worker_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let run = self.clone();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = run.work_stop.cancelled() => return,
                    () = run.work_wake.notified() => {
                        if !run.reconcile_recovery_prompt().await {
                            continue;
                        }
                        while run.reconcile_once(&core).await {
                            if run.work_stop.is_cancelled() {
                                break;
                            }
                        }
                    },
                }
            }
        });
        *self.work_task.lock().expect("work task lock poisoned") = Some(task);
    }

    pub(super) fn trigger_reconcile(&self) {
        self.work_wake.notify_one();
    }

    async fn reconcile_recovery_prompt(&self) -> bool {
        let prompt = self.recovery_prompt.lock().await.clone();
        let Some(prompt) = prompt else {
            return true;
        };
        self.set_state(RunState::Running).await;
        let request = self.rpc.request(
            "session/prompt",
            json!({
                "sessionId": self.acp_session_id,
                "prompt": [{"type": "text", "text": prompt}]
            }),
            None,
        );
        tokio::pin!(request);
        let result = tokio::select! {
            () = self.work_stop.cancelled() => Err(AcpClientError::disconnected(
                "ACP Client shut down while its Recovery Prompt was active",
            )),
            result = &mut request => result,
        };
        let completed = result.is_ok_and(|response| {
            response.get("stopReason").and_then(Value::as_str) != Some("cancelled")
        });
        let _ = self.cancel_live_requests().await;
        if completed {
            self.recovery_prompt.lock().await.take();
        }
        self.set_idle_state().await;
        completed
    }

    async fn reconcile_once(&self, core: &Core) -> bool {
        let batch = match core
            .claim_agent_work(WorkScope {
                session_id: Some(self.session_id.clone()),
                limit: 1,
                lease_seconds: 3_600,
            })
            .await
        {
            Ok(batch) => batch,
            Err(error) => {
                tracing::error!(session_id = %self.session_id, error = %error, "could not claim Agent work");
                return false;
            }
        };
        let had_work = !batch.items.is_empty();
        let mut retry = false;
        for claimed in batch.items {
            self.set_state(RunState::Running).await;
            let prompt = prompt_for_work(&claimed.work);
            let prompt = self.rpc.request(
                "session/prompt",
                json!({
                    "sessionId": self.acp_session_id,
                    "prompt": [{"type": "text", "text": prompt}]
                }),
                None,
            );
            tokio::pin!(prompt);
            let result = tokio::select! {
                () = self.work_stop.cancelled() => Err(AcpClientError::disconnected(
                    "ACP Client shut down while Agent work was active",
                )),
                result = &mut prompt => result,
            };
            let disposition = match result {
                Ok(response) => completion_evidence(&claimed.work.payload, &response, self).await,
                Err(error) => AgentWorkDisposition::Retry {
                    error_code: format!("ACP_{:?}", error.code),
                },
            };
            retry = matches!(&disposition, AgentWorkDisposition::Retry { .. });
            let _ = self.cancel_live_requests().await;
            if let Err(error) = core
                .record_agent_work(AgentWorkResult {
                    work_id: claimed.work.work_id,
                    claim_token: claimed.claim_token,
                    disposition,
                })
                .await
            {
                tracing::error!(session_id = %self.session_id, error = %error, "could not record Agent work");
                retry = true;
            }
        }
        self.set_idle_state().await;
        had_work && !retry
    }

    async fn set_idle_state(&self) {
        let state = if self.rpc.is_closed() {
            RunState::Disconnected
        } else {
            RunState::Ready
        };
        self.set_state(state).await;
    }

    async fn set_state(&self, state: RunState) {
        self.live.lock().await.state = state;
        let _ = self.events.send(LiveSessionEvent::StateChanged {
            session_id: self.session_id.clone(),
            state,
        });
    }

    fn ensure_wire_session(&self, params: &Value) -> Result<(), AcpClientError> {
        if params.get("sessionId").and_then(Value::as_str) == Some(self.acp_session_id.as_str()) {
            Ok(())
        } else {
            Err(AcpClientError::protocol(
                "live request belongs to another ACP Session",
            ))
        }
    }

    pub(super) async fn shutdown(&self) -> Result<bool, AcpClientError> {
        self.work_stop.cancel();
        let _ = self.cancel_live_requests().await;
        if self.capabilities.close_session {
            let _ = self
                .rpc
                .request(
                    "session/close",
                    json!({"sessionId": self.acp_session_id}),
                    Some(self.shutdown_grace),
                )
                .await;
        }
        let forced = self.rpc.shutdown(self.shutdown_grace).await?;
        let work_task = self
            .work_task
            .lock()
            .expect("work task lock poisoned")
            .take();
        if let Some(task) = work_task {
            task.await.map_err(|error| {
                AcpClientError::new(
                    AcpErrorCode::ShutdownFailed,
                    format!("Agent work worker failed during shutdown: {error}"),
                    false,
                )
            })?;
        }
        if let Some(toolset) = self.toolset.lock().await.take() {
            toolset.shutdown().await?;
        }
        Ok(forced)
    }
}

fn required_string(value: &Value, field: &str) -> Result<String, AcpClientError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AcpClientError::protocol(format!("live request omitted {field}")))
}

fn reindex_permissions(queue: &mut VecDeque<PendingPermission>) {
    for (position, pending) in queue.iter_mut().enumerate() {
        pending.request.queue_position = position;
    }
}

fn reindex_questions(queue: &mut VecDeque<PendingQuestion>) {
    for (position, pending) in queue.iter_mut().enumerate() {
        pending.question.queue_position = position;
    }
}

fn projected_state(live: &LiveState) -> RunState {
    if !live.permissions.is_empty() {
        RunState::WaitingForPermission
    } else if !live.questions.is_empty() {
        RunState::WaitingForQuestion
    } else {
        RunState::Running
    }
}

fn live_not_found() -> AcpClientError {
    AcpClientError::new(
        AcpErrorCode::LiveRequestNotFound,
        "live request is no longer pending",
        false,
    )
}

fn live_not_current() -> AcpClientError {
    AcpClientError::new(
        AcpErrorCode::LiveRequestNotCurrent,
        "live requests must be answered in FIFO order",
        false,
    )
}

fn prompt_for_work(work: &rambledesk_core::kernel::AgentWorkRecord) -> String {
    let marker = format!("[RambleDesk work_id: {}]", work.work_id);
    match &work.payload {
        AgentWorkPayload::Launch {
            prompt_markdown, ..
        }
        | AgentWorkPayload::Steering {
            prompt_markdown, ..
        } => format!("{marker}\n\n{prompt_markdown}"),
        AgentWorkPayload::FeedbackResume {
            delivery_id,
            request_id,
        } => format!(
            "{marker}\n\nRambleDesk has resolved Feedback Request {request_id}. Call get_feedback with request_id {request_id} now. Consume the returned envelope and de-duplicate it by delivery_id {delivery_id}."
        ),
    }
}

async fn completion_evidence(
    payload: &AgentWorkPayload,
    response: &Value,
    run: &ManagedRun,
) -> AgentWorkDisposition {
    if response.get("stopReason").and_then(Value::as_str) == Some("cancelled") {
        return AgentWorkDisposition::Retry {
            error_code: "ACP_TURN_CANCELLED".to_string(),
        };
    }
    match payload {
        AgentWorkPayload::FeedbackResume { delivery_id, .. } => {
            if run
                .consumed_deliveries
                .lock()
                .await
                .remove(delivery_id.as_str())
            {
                AgentWorkDisposition::Completed {
                    evidence: AgentWorkEvidence::FeedbackConsumedAndTurnCompleted {
                        delivery_id: delivery_id.clone(),
                    },
                }
            } else {
                AgentWorkDisposition::Retry {
                    error_code: "FEEDBACK_NOT_CONSUMED".to_string(),
                }
            }
        }
        _ => AgentWorkDisposition::Completed {
            evidence: AgentWorkEvidence::PromptTurnCompleted,
        },
    }
}

async fn observe_feedback_consumption(update: &Value, consumed: &Mutex<HashSet<String>>) {
    let update_type = update.get("sessionUpdate").and_then(Value::as_str);
    if !matches!(update_type, Some("tool_call") | Some("tool_call_update")) {
        return;
    }
    let status = update.get("status").and_then(Value::as_str);
    if status != Some("completed") {
        return;
    }
    let tool_name = update
        .get("title")
        .and_then(Value::as_str)
        .or_else(|| update.get("name").and_then(Value::as_str));
    if !tool_name.is_some_and(|name| name.ends_with("get_feedback")) {
        return;
    }
    if let Some(delivery_id) = update
        .get("rawOutput")
        .and_then(|value| value.get("delivery_id"))
        .and_then(Value::as_str)
    {
        consumed.lock().await.insert(delivery_id.to_string());
    }
}
