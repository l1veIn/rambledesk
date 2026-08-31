use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rambledesk_core::kernel::{
    AccessMode, AgentWorkState, ArtifactInput, Core, CreateFeedbackRequest, DraftId, DraftMutation,
    FeedbackAction, FeedbackSubmission, LaunchConfiguration, LaunchSubmission, RambleContent,
    RambleIntent, RequestId, ResolveFeedbackRequest, SaveDraft, SubmissionId, WorkScope,
};
use rambledesk_storage::v3::{SqliteV3Store, artifact::LocalArtifactStore};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream},
    sync::Mutex,
};

use super::*;
use crate::{
    QuestionAction, RunState,
    process::{AgentSpawner, ProcessControl, SpawnedAgent},
};

#[derive(Default)]
struct FakeState {
    lifecycle: Mutex<Vec<(String, Vec<Value>)>>,
    answers: Mutex<Vec<Value>>,
    prompts: Mutex<Vec<String>>,
    config_options: Mutex<Vec<Value>>,
    config_selections: Mutex<Vec<(String, Value)>>,
    mcp_server: Mutex<Option<Value>>,
    feedback_reads: AtomicUsize,
    close_calls: AtomicUsize,
    hang_prompts: AtomicBool,
    fail_session_setup: AtomicBool,
    fail_resume: AtomicBool,
    fail_load: AtomicBool,
    disconnect_on_prompt: AtomicBool,
    spawns: AtomicUsize,
    shutdown: AtomicBool,
}

mod edge_cases;

struct FakeSpawner {
    state: Arc<FakeState>,
}

#[async_trait]
impl AgentSpawner for FakeSpawner {
    async fn spawn(&self, _profile: &LaunchProfile) -> Result<SpawnedAgent, AcpClientError> {
        self.state.spawns.fetch_add(1, Ordering::AcqRel);
        let (client, agent) = tokio::io::duplex(1024 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client);
        let state = self.state.clone();
        tokio::spawn(async move { fake_agent(agent, state).await });
        Ok(SpawnedAgent {
            reader: Box::pin(client_reader),
            writer: Box::pin(client_writer),
            control: Arc::new(FakeControl {
                state: self.state.clone(),
            }),
        })
    }
}

struct FakeControl {
    state: Arc<FakeState>,
}

#[async_trait]
impl ProcessControl for FakeControl {
    async fn shutdown(&self, _grace: Duration) -> Result<bool, AcpClientError> {
        self.state.shutdown.store(true, Ordering::Release);
        Ok(false)
    }
}

async fn fake_agent(stream: DuplexStream, state: Arc<FakeState>) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();
    let mut prompt_id = None;
    let mut live_answers = 0;
    while let Ok(Some(line)) = lines.next_line().await {
        let message: Value = serde_json::from_str(&line).expect("client frame");
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            let id = message.get("id").cloned();
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            match method {
                "initialize" => {
                    write_frame(
                        &mut writer,
                        json!({
                            "jsonrpc":"2.0", "id":id,
                            "result": {
                                "protocolVersion": 1,
                                    "agentCapabilities": {
                                        "loadSession": true,
                                        "sessionCapabilities": {"resume": {}, "close": {}},
                                        "mcpCapabilities": {"http": true}
                                },
                                "agentInfo": {"name":"fake-acp", "version":"1"}
                            }
                        }),
                    )
                    .await;
                }
                "session/new" | "session/resume" | "session/load" => {
                    let servers = params
                        .get("mcpServers")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    state
                        .lifecycle
                        .lock()
                        .await
                        .push((method.to_string(), servers.clone()));
                    if let Some(server) = servers.first() {
                        *state.mcp_server.lock().await = Some(server.clone());
                    }
                    if state.fail_session_setup.load(Ordering::Acquire) {
                        write_frame(
                            &mut writer,
                            json!({
                                "jsonrpc":"2.0", "id":id,
                                "error":{"code":-32000,"message":"forced session setup failure"}
                            }),
                        )
                        .await;
                        continue;
                    }
                    if (method == "session/resume" && state.fail_resume.load(Ordering::Acquire))
                        || (method == "session/load" && state.fail_load.load(Ordering::Acquire))
                    {
                        write_frame(
                            &mut writer,
                            json!({
                                "jsonrpc":"2.0", "id":id,
                                "error":{"code":-32000,"message":"forced recovery failure"}
                            }),
                        )
                        .await;
                        continue;
                    }
                    let config_options = {
                        let mut options = state.config_options.lock().await;
                        if method == "session/new" || options.is_empty() {
                            *options = fake_config_options();
                        }
                        options.clone()
                    };
                    let result = json!({
                        "configOptions": config_options,
                        "sessionId": (method == "session/new").then_some("fake-session")
                    });
                    write_frame(
                        &mut writer,
                        json!({"jsonrpc":"2.0", "id":id, "result":result}),
                    )
                    .await;
                }
                "session/set_config_option" => {
                    let config_id = params["configId"].as_str().unwrap().to_string();
                    let value = params["value"].clone();
                    state
                        .config_selections
                        .lock()
                        .await
                        .push((config_id.clone(), value.clone()));
                    let config_options = {
                        let mut options = state.config_options.lock().await;
                        let option = options
                            .iter_mut()
                            .find(|option| option["id"] == config_id)
                            .expect("known config option");
                        option["currentValue"] = value;
                        options.clone()
                    };
                    write_frame(
                        &mut writer,
                        json!({"jsonrpc":"2.0", "id":id, "result":{"configOptions":config_options}}),
                    )
                    .await;
                }
                "session/prompt" => {
                    if state.hang_prompts.load(Ordering::Acquire) {
                        continue;
                    }
                    let prompt = params["prompt"][0]["text"].as_str().unwrap_or("");
                    state.prompts.lock().await.push(prompt.to_string());
                    if state.disconnect_on_prompt.swap(false, Ordering::AcqRel) {
                        return;
                    }
                    if prompt.starts_with("[RambleDesk Recovery Context]") {
                        write_frame(
                            &mut writer,
                            json!({"jsonrpc":"2.0", "id":id, "result":{"stopReason":"end_turn"}}),
                        )
                        .await;
                        continue;
                    }
                    if prompt.contains("Call get_feedback") {
                        let request_id = between(prompt, "request_id ", " now.");
                        let delivery_id = between(prompt, "delivery_id ", ".");
                        call_get_feedback(&state, request_id, delivery_id).await;
                        write_frame(
                            &mut writer,
                            json!({
                                "jsonrpc":"2.0",
                                "method":"session/update",
                                "params":{
                                    "sessionId":"fake-session",
                                    "update":{
                                        "sessionUpdate":"tool_call",
                                        "status":"completed",
                                        "title":"rambledesk/get_feedback",
                                        "rawOutput":{"delivery_id":delivery_id}
                                    }
                                }
                            }),
                        )
                        .await;
                        write_frame(
                            &mut writer,
                            json!({"jsonrpc":"2.0", "id":id, "result":{"stopReason":"end_turn"}}),
                        )
                        .await;
                        continue;
                    }
                    prompt_id = id;
                    for (request_id, title) in [(900, "Run tests"), (901, "Write files")] {
                        write_frame(
                            &mut writer,
                            json!({
                                "jsonrpc":"2.0", "id":request_id,
                                "method":"session/request_permission",
                                    "params": {
                                        "sessionId":"fake-session",
                                        "_meta":{"permission":{"title":title,"description":"Needs human approval"}},
                                        "toolCall":{"toolCallId":format!("call-{request_id}"),"title":title,"kind":"execute"},
                                    "options":[
                                        {"optionId":"allow-once","name":"Allow once","kind":"allow_once"},
                                        {"optionId":"reject-once","name":"Reject","kind":"reject_once"}
                                    ]
                                }
                            }),
                        )
                        .await;
                    }
                    write_frame(
                        &mut writer,
                        json!({
                            "jsonrpc":"2.0", "id":902,
                            "method":"elicitation/create",
                            "params": {
                                "sessionId":"fake-session",
                                "mode":"form",
                                "message":"Choose a strategy",
                                "requestedSchema": {
                                    "type":"object",
                                    "properties":{"strategy":{"type":"string","enum":["safe","fast"]}},
                                    "required":["strategy"]
                                }
                            }
                        }),
                    )
                    .await;
                    write_frame(
                        &mut writer,
                        json!({
                            "jsonrpc":"2.0", "id":903,
                            "method":"elicitation/create",
                            "params": {
                                "sessionId":"fake-session",
                                "mode":"form",
                                "message":"Explain and choose",
                                "requestedSchema": {
                                    "type":"object",
                                    "properties":{
                                        "details":{"type":"string"},
                                        "strategy":{"type":"string","enum":["safe","fast"]}
                                    }
                                }
                            }
                        }),
                    )
                    .await;
                }
                "session/cancel" => {}
                "session/close" => {
                    state.close_calls.fetch_add(1, Ordering::AcqRel);
                    write_frame(&mut writer, json!({"jsonrpc":"2.0", "id":id, "result":{}})).await;
                }
                _ => {
                    if let Some(id) = id {
                        write_frame(&mut writer, json!({"jsonrpc":"2.0", "id":id, "result":{}}))
                            .await;
                    }
                }
            }
        } else if message.get("id").is_some() {
            state.answers.lock().await.push(message.clone());
            if message["id"] == 903 {
                assert_eq!(message["result"], json!({"action":"decline"}));
                continue;
            }
            live_answers += 1;
            if live_answers == 3
                && let Some(id) = prompt_id.take()
            {
                write_frame(
                    &mut writer,
                    json!({"jsonrpc":"2.0", "id":id, "result":{"stopReason":"end_turn"}}),
                )
                .await;
            }
        }
    }
}

async fn write_frame(writer: &mut tokio::io::WriteHalf<DuplexStream>, value: Value) {
    writer
        .write_all(format!("{}\n", serde_json::to_string(&value).unwrap()).as_bytes())
        .await
        .expect("write fake frame");
    writer.flush().await.expect("flush fake frame");
}

fn fake_config_options() -> Vec<Value> {
    vec![
        json!({
            "id":"model", "name":"Model", "category":"model", "type":"select",
            "currentValue":"fake", "options":[
                {"value":"fake","name":"Fake"}, {"value":"fake-next","name":"Fake next"}
            ]
        }),
        json!({
            "id":"reasoning_effort", "name":"Reasoning", "category":"thought_level", "type":"select",
            "currentValue":"medium", "options":[
                {"value":"medium","name":"Medium"}, {"value":"high","name":"High"}
            ]
        }),
        json!({
            "id":"mode", "name":"Mode", "category":"mode", "type":"select",
            "currentValue":"agent", "options":[
                {"value":"read-only","name":"Ask for approval","_meta":{"kind":"standard"}},
                {"value":"agent","name":"Approve for me","_meta":{"kind":"auto_review"}},
                {"value":"agent-full-access","name":"Full access","_meta":{"kind":"full_access"}}
            ]
        }),
    ]
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let tail = text.split_once(start).expect("prompt start marker").1;
    tail.split_once(end).expect("prompt end marker").0
}

async fn call_get_feedback(state: &FakeState, request_id: &str, delivery_id: &str) {
    let server = state
        .mcp_server
        .lock()
        .await
        .clone()
        .expect("injected Session Toolset");
    let endpoint = server["url"].as_str().expect("toolset endpoint");
    let authorization = server["headers"][0]["value"]
        .as_str()
        .expect("authorization");
    let client = reqwest::Client::new();
    for request in [
        json!({
            "jsonrpc":"2.0", "id":1, "method":"initialize",
            "params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"fake-acp","version":"1"}}
        }),
        json!({
            "jsonrpc":"2.0", "id":2, "method":"tools/call",
            "params":{"name":"get_feedback","arguments":{"request_id":request_id}}
        }),
    ] {
        let response = client
            .post(endpoint)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .header(
                reqwest::header::ACCEPT,
                "application/json, text/event-stream",
            )
            .json(&request)
            .send()
            .await
            .expect("toolset response");
        let body = response.text().await.expect("toolset body");
        if request["id"] == 2 {
            assert!(body.contains(request_id), "{body}");
            assert!(body.contains(delivery_id), "{body}");
        }
    }
    state.feedback_reads.fetch_add(1, Ordering::AcqRel);
}

async fn test_core() -> (TempDir, Arc<Core>, Arc<SqliteV3Store>) {
    let temp = TempDir::new().expect("tempdir");
    let store = Arc::new(
        SqliteV3Store::connect(&temp.path().join("v3.sqlite3"))
            .await
            .expect("v3 store"),
    );
    let artifacts = Arc::new(
        LocalArtifactStore::open(temp.path().join("library"))
            .await
            .expect("artifact store"),
    );
    let core = Arc::new(Core::new(store.clone(), artifacts));
    (temp, core, store)
}

fn fake_profile() -> LaunchProfile {
    LaunchProfile {
        profile_ref: LaunchProfileRef {
            agent_profile_id: "codex".to_string(),
            launch_profile_id: "codex-acp-npx".to_string(),
        },
        command: "fake-acp".into(),
        args: Vec::new(),
        env: BTreeMap::new(),
        configuration: LaunchProfile::codex_npx().configuration,
        session_toolset: crate::SessionToolsetPolicy::Required,
    }
}

async fn launch(core: &Core, workspace: &Path) -> rambledesk_core::kernel::LaunchOutcome {
    core.launch(LaunchSubmission {
        submission_id: SubmissionId::new("launch-1"),
        submission_digest_assertion: None,
        title: "Fake Session".to_string(),
        launch_configuration: LaunchConfiguration {
            agent_profile_id: "codex".to_string(),
            launch_profile_id: "codex-acp-npx".to_string(),
            workspace_reference: workspace.to_string_lossy().to_string(),
            model: Some("fake-next".to_string()),
            reasoning_effort: Some("high".to_string()),
            access_mode: AccessMode::WorkspaceWrite,
            agent_config_json: "{}".to_string(),
        },
        ramble: RambleContent {
            document_json: "{}".to_string(),
            body_markdown: "Implement the slice".to_string(),
            artifacts: Vec::<ArtifactInput>::new(),
        },
    })
    .await
    .expect("launch")
}

#[tokio::test]
async fn preflight_reports_the_negotiated_agent_contract() {
    let (_temp, core, store) = test_core().await;
    let state = Arc::new(FakeState::default());
    let client = AcpClient::new_with_spawner(
        core,
        AcpClientConfig {
            profiles: vec![fake_profile()],
            preflight_timeout: Duration::from_secs(2),
            operation_timeout: Duration::from_secs(2),
            shutdown_grace: Duration::from_millis(20),
            event_capacity: 32,
        },
        Arc::new(FakeSpawner {
            state: state.clone(),
        }),
    )
    .expect("client");
    let report = client
        .preflight(fake_profile().profile_ref)
        .await
        .expect("preflight");
    assert!(report.available);
    assert_eq!(report.agent_version.as_deref(), Some("fake-acp 1"));
    assert!(report.capabilities.resume_session);
    assert!(report.capabilities.mcp_http);
    assert_eq!(report.config_options.len(), 3);
    assert_eq!(
        report.supported_access_modes,
        vec![AccessMode::WorkspaceWrite, AccessMode::Yolo]
    );
    assert_eq!(state.close_calls.load(Ordering::Acquire), 1);
    assert!(state.shutdown.load(Ordering::Acquire));
    store.close().await;
}

#[test]
fn legacy_acp_models_and_modes_are_projected_as_config_options() {
    let options = config_options(&json!({
        "models": {
            "currentModelId": "agent/default",
            "availableModels": [
                {"modelId": "agent/default", "name": "Default"},
                {"modelId": "agent/fast", "name": "Fast", "description": "Lower latency"}
            ]
        },
        "modes": {
            "currentModeId": "plan",
            "availableModes": [
                {"id": "plan", "name": "Plan"},
                {"id": "agent", "name": "Agent"}
            ]
        }
    }));

    assert_eq!(options.len(), 2);
    assert_eq!(options[0]["id"], "model");
    assert_eq!(options[0]["currentValue"], "agent/default");
    assert_eq!(options[0]["_rambledeskMutation"], "set_model");
    assert_eq!(options[0]["options"][1]["value"], "agent/fast");
    assert_eq!(options[1]["id"], "mode");
    assert_eq!(options[1]["currentValue"], "plan");
    assert_eq!(options[1]["_rambledeskMutation"], "set_mode");
    assert_eq!(options[1]["options"][1]["value"], "agent");
}

#[test]
fn explicit_config_options_take_precedence_over_legacy_fields() {
    let options = config_options(&json!({
        "configOptions": [{
            "id": "model",
            "category": "model",
            "type": "select",
            "currentValue": "modern",
            "options": [{"value": "modern", "name": "Modern"}]
        }],
        "models": {
            "currentModelId": "legacy",
            "availableModels": [{"modelId": "legacy", "name": "Legacy"}]
        }
    }));

    assert_eq!(options.len(), 1);
    assert_eq!(options[0]["currentValue"], "modern");
    assert!(options[0].get("_rambledeskMutation").is_none());
}

#[tokio::test]
async fn interface_reconciles_fifo_permission_question_and_reinjects_toolset_on_resume() {
    let (temp, core, store) = test_core().await;
    let launched = launch(&core, temp.path()).await;
    assert_eq!(launched.agent_work_state, AgentWorkState::Pending);
    let state = Arc::new(FakeState::default());
    let config = AcpClientConfig {
        profiles: vec![fake_profile()],
        preflight_timeout: Duration::from_secs(2),
        operation_timeout: Duration::from_secs(2),
        shutdown_grace: Duration::from_millis(20),
        event_capacity: 32,
    };
    let client = AcpClient::new_with_spawner(
        core.clone(),
        config.clone(),
        Arc::new(FakeSpawner {
            state: state.clone(),
        }),
    )
    .expect("client");
    let first = client
        .reconcile(SessionScope {
            session_id: launched.session_id.clone(),
        })
        .await
        .expect("first reconcile");
    assert_eq!(first.recovery_method, RecoveryMethod::New);

    let current = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = client
                .reconcile(SessionScope {
                    session_id: launched.session_id.clone(),
                })
                .await
                .unwrap();
            if snapshot.permissions.len() == 2 && snapshot.questions.len() == 1 {
                break snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("live requests");
    let second = current.permissions[1].clone();
    assert_eq!(
        current.permissions[0].request_meta["permission"]["description"],
        "Needs human approval"
    );
    assert_eq!(second.queue_position, 1);
    let error = client
        .answer_permission(PermissionAnswer {
            session_id: launched.session_id.clone(),
            live_request_id: second.live_request_id.clone(),
            option_id: "allow-once".to_string(),
        })
        .await
        .expect_err("FIFO must reject second");
    assert_eq!(error.code, AcpErrorCode::LiveRequestNotCurrent);
    for permission in &current.permissions {
        client
            .answer_permission(PermissionAnswer {
                session_id: launched.session_id.clone(),
                live_request_id: permission.live_request_id.clone(),
                option_id: "allow-once".to_string(),
            })
            .await
            .expect("answer permission");
    }
    client
        .answer_question(QuestionAnswer {
            session_id: launched.session_id.clone(),
            live_request_id: current.questions[0].live_request_id.clone(),
            action: QuestionAction::Accept,
            content: Some(json!({"strategy":"safe"})),
        })
        .await
        .expect("answer question");

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let recovery = core
                .read_session_recovery(launched.session_id.clone())
                .await
                .unwrap();
            if recovery.pending_agent_work.is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("work completed");

    let feedback = core
        .request_feedback(CreateFeedbackRequest {
            request_id: Some(RequestId::new("live-feedback")),
            session_id: launched.session_id.clone(),
            source_link_id: None,
            title: "Review live Run".to_string(),
            instructions: "Return feedback without restarting the App.".to_string(),
            actions: vec![FeedbackAction {
                id: "review".to_string(),
                instruction: "Review the result.".to_string(),
            }],
            context_refs: Vec::new(),
            artifacts: Vec::new(),
        })
        .await
        .expect("request feedback");
    let draft = core
        .mutate_draft(DraftMutation::Save(SaveDraft {
            draft_id: DraftId::new("live-draft"),
            intent: RambleIntent::Feedback,
            session_id: Some(launched.session_id.clone()),
            request_id: Some(feedback.request_id.clone()),
            launch_configuration: None,
            document_json: "{}".to_string(),
            body_markdown: "Human feedback".to_string(),
            expected_revision: 0,
        }))
        .await
        .expect("save draft");
    let resolution = core
        .resolve_feedback(ResolveFeedbackRequest::Submit(FeedbackSubmission {
            submission_id: SubmissionId::new("live-feedback-submission"),
            request_id: feedback.request_id,
            expected_draft_revision: draft.revision,
            submission_digest_assertion: None,
            document_json: "{}".to_string(),
            uncooked_markdown: "Raw feedback".to_string(),
            feedback_markdown: "Structured feedback".to_string(),
            cooking_model: None,
            artifacts: Vec::new(),
        }))
        .await
        .expect("resolve feedback");
    client
        .reconcile(SessionScope {
            session_id: launched.session_id.clone(),
        })
        .await
        .expect("wake existing live Run");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let outcome = core
                .get_feedback(rambledesk_core::kernel::GetFeedback {
                    request_id: resolution.request.request_id.clone(),
                })
                .await
                .unwrap();
            if matches!(
                outcome,
                rambledesk_core::kernel::GetFeedbackOutcome::Terminal(ref delivery)
                    if delivery.delivery_id == resolution.delivery_id
            ) && core
                .read_session_recovery(launched.session_id.clone())
                .await
                .unwrap()
                .pending_agent_work
                .is_empty()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("existing Run delivered feedback");
    assert_eq!(state.feedback_reads.load(Ordering::Acquire), 1);
    client.shutdown().await.expect("shutdown first run");

    let second_client = AcpClient::new_with_spawner(
        core.clone(),
        config,
        Arc::new(FakeSpawner {
            state: state.clone(),
        }),
    )
    .expect("second client");
    let resumed = second_client
        .reconcile(SessionScope {
            session_id: launched.session_id.clone(),
        })
        .await
        .expect("resume");
    assert_eq!(resumed.recovery_method, RecoveryMethod::Resume);
    second_client.shutdown().await.expect("shutdown second run");

    let lifecycle = state.lifecycle.lock().await;
    assert_eq!(lifecycle.len(), 2);
    assert_eq!(lifecycle[0].0, "session/new");
    assert_eq!(lifecycle[1].0, "session/resume");
    for (_, servers) in lifecycle.iter() {
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["type"], "http");
        assert_eq!(servers[0]["name"], "rambledesk");
    }
    let selections = state.config_selections.lock().await;
    assert_eq!(selections.len(), 3);
    assert_eq!(selections[0], ("model".to_string(), json!("fake-next")));
    assert_eq!(
        selections[1],
        ("reasoning_effort".to_string(), json!("high"))
    );
    assert_eq!(selections[2], ("mode".to_string(), json!("read-only")));
    assert_eq!(state.answers.lock().await.len(), 4);
    assert_eq!(state.close_calls.load(Ordering::Acquire), 2);
    store.close().await;
}
