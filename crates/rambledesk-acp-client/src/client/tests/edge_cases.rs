use super::*;
use rambledesk_core::kernel::{
    AcpSessionLinkObservation, AgentObservation, AgentWorkDisposition, AgentWorkEvidence,
    AgentWorkResult, GetFeedback, GetFeedbackOutcome, SteeringSubmission,
};

#[tokio::test]
async fn setup_failure_stops_the_process_and_session_toolset() {
    let (temp, core, store) = test_core().await;
    let launched = launch(&core, temp.path()).await;
    let state = Arc::new(FakeState::default());
    state.fail_session_setup.store(true, Ordering::Release);
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
    let error = client
        .reconcile(SessionScope {
            session_id: launched.session_id,
        })
        .await
        .expect_err("forced setup failure");
    assert_eq!(error.code, AcpErrorCode::RpcError);
    assert!(state.shutdown.load(Ordering::Acquire));
    let endpoint = state.mcp_server.lock().await.as_ref().unwrap()["url"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(reqwest::Client::new().post(endpoint).send().await.is_err());
    store.close().await;
}

#[tokio::test]
async fn shutdown_retries_active_work_before_returning() {
    let (temp, core, store) = test_core().await;
    let launched = launch(&core, temp.path()).await;
    let state = Arc::new(FakeState::default());
    state.hang_prompts.store(true, Ordering::Release);
    let client = AcpClient::new_with_spawner(
        core.clone(),
        AcpClientConfig {
            profiles: vec![fake_profile()],
            preflight_timeout: Duration::from_secs(2),
            operation_timeout: Duration::from_secs(2),
            shutdown_grace: Duration::from_millis(20),
            event_capacity: 32,
        },
        Arc::new(FakeSpawner { state }),
    )
    .expect("client");
    client
        .reconcile(SessionScope {
            session_id: launched.session_id.clone(),
        })
        .await
        .expect("reconcile");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if client
                .reconcile(SessionScope {
                    session_id: launched.session_id.clone(),
                })
                .await
                .unwrap()
                .state
                == RunState::Running
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("active prompt");
    client.shutdown().await.expect("bounded shutdown");
    let reclaimed = core
        .claim_agent_work(WorkScope {
            session_id: Some(launched.session_id),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("claim immediately after shutdown");
    assert_eq!(reclaimed.items.len(), 1);
    assert_eq!(reclaimed.items[0].work.state, AgentWorkState::Claimed);
    store.close().await;
}

#[tokio::test]
async fn recovery_falls_back_from_resume_to_load_to_new() {
    let (temp, core, store) = test_core().await;
    let launched = launch(&core, temp.path()).await;
    let state = Arc::new(FakeState::default());
    let config = AcpClientConfig {
        profiles: vec![fake_profile()],
        preflight_timeout: Duration::from_secs(2),
        operation_timeout: Duration::from_secs(2),
        shutdown_grace: Duration::from_millis(20),
        event_capacity: 32,
    };
    let first = AcpClient::new_with_spawner(
        core.clone(),
        config.clone(),
        Arc::new(FakeSpawner {
            state: state.clone(),
        }),
    )
    .unwrap();
    assert_eq!(
        first
            .reconcile(SessionScope {
                session_id: launched.session_id.clone(),
            })
            .await
            .unwrap()
            .recovery_method,
        RecoveryMethod::New
    );
    first.shutdown().await.unwrap();

    state.fail_resume.store(true, Ordering::Release);
    state.fail_load.store(true, Ordering::Release);
    let recovered = AcpClient::new_with_spawner(
        core,
        config,
        Arc::new(FakeSpawner {
            state: state.clone(),
        }),
    )
    .unwrap();
    assert_eq!(
        recovered
            .reconcile(SessionScope {
                session_id: launched.session_id,
            })
            .await
            .unwrap()
            .recovery_method,
        RecoveryMethod::New
    );
    recovered.shutdown().await.unwrap();

    let lifecycle = state.lifecycle.lock().await;
    assert_eq!(
        lifecycle
            .iter()
            .map(|(method, _)| method.as_str())
            .collect::<Vec<_>>(),
        vec![
            "session/new",
            "session/resume",
            "session/load",
            "session/new"
        ]
    );
    store.close().await;
}

#[tokio::test]
async fn disconnected_registered_run_is_replaced_and_recovered_instead_of_only_woken() {
    let (temp, core, store) = test_core().await;
    let launched = launch(&core, temp.path()).await;
    let state = Arc::new(FakeState::default());
    state.disconnect_on_prompt.store(true, Ordering::Release);
    let client = AcpClient::new_with_spawner(
        core.clone(),
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
    .unwrap();
    client
        .reconcile(SessionScope {
            session_id: launched.session_id.clone(),
        })
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let pending = core
                .read_session_recovery(launched.session_id.clone())
                .await
                .unwrap()
                .pending_agent_work;
            let snapshot = client.run(&launched.session_id).unwrap().snapshot().await;
            if pending.first().is_some_and(|work| work.attempt_count == 1)
                && snapshot.state == RunState::Disconnected
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("disconnected work recorded once");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        core.read_session_recovery(launched.session_id.clone())
            .await
            .unwrap()
            .pending_agent_work[0]
            .attempt_count,
        1,
        "Retry work must not spin inside the current process"
    );

    let recovered = client
        .reconcile(SessionScope {
            session_id: launched.session_id.clone(),
        })
        .await
        .expect("disconnected Run should enter ACP recovery");
    assert_eq!(recovered.recovery_method, RecoveryMethod::Resume);
    assert_eq!(state.spawns.load(Ordering::Acquire), 2);
    assert_eq!(
        state
            .lifecycle
            .lock()
            .await
            .iter()
            .map(|(method, _)| method.as_str())
            .collect::<Vec<_>>(),
        vec!["session/new", "session/resume"]
    );
    client.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn new_fallback_restores_durable_context_before_feedback_resume() {
    let (temp, core, store) = test_core().await;
    let launched = launch(&core, temp.path()).await;
    core.steer(SteeringSubmission {
        submission_id: SubmissionId::new("steering-recovery"),
        session_id: launched.session_id.clone(),
        submission_digest_assertion: None,
        ramble: RambleContent {
            document_json: "{}".to_string(),
            body_markdown: "Preserve the existing three-column layout".to_string(),
            artifacts: Vec::new(),
        },
    })
    .await
    .unwrap();
    complete_pending_work(&core, &launched.session_id).await;
    core.record_agent_observation(AgentObservation::AcpSessionLinked(
        AcpSessionLinkObservation {
            session_id: launched.session_id.clone(),
            agent_profile_id: "codex".to_string(),
            launch_profile_id: "codex-acp-npx".to_string(),
            acp_session_id: "lost-acp-session".to_string(),
            capabilities_json: "{}".to_string(),
            session_toolset_digest: format!("sha256:{}", "0".repeat(64)),
        },
    ))
    .await
    .unwrap();
    let feedback = core
        .request_feedback(CreateFeedbackRequest {
            request_id: Some(RequestId::new("recovery-feedback")),
            session_id: launched.session_id.clone(),
            source_link_id: None,
            title: "Review the recovered task".to_string(),
            instructions: "Apply the human response after recovery.".to_string(),
            actions: vec![FeedbackAction {
                id: "apply".to_string(),
                instruction: "Apply the selected revision.".to_string(),
            }],
            context_refs: Vec::new(),
            artifacts: Vec::new(),
        })
        .await
        .unwrap();
    let draft = core
        .mutate_draft(DraftMutation::Save(SaveDraft {
            draft_id: DraftId::new("recovery-draft"),
            intent: RambleIntent::Feedback,
            session_id: Some(launched.session_id.clone()),
            request_id: Some(feedback.request_id.clone()),
            launch_configuration: None,
            document_json: "{}".to_string(),
            body_markdown: "Recovered human response".to_string(),
            expected_revision: 0,
        }))
        .await
        .unwrap();
    let resolved = core
        .resolve_feedback(ResolveFeedbackRequest::Submit(FeedbackSubmission {
            submission_id: SubmissionId::new("recovery-feedback-submission"),
            request_id: feedback.request_id,
            expected_draft_revision: draft.revision,
            submission_digest_assertion: None,
            document_json: "{}".to_string(),
            uncooked_markdown: "Recovered raw response".to_string(),
            feedback_markdown: "Recovered structured response".to_string(),
            cooking_model: None,
            artifacts: Vec::new(),
        }))
        .await
        .unwrap();

    let state = Arc::new(FakeState::default());
    state.fail_resume.store(true, Ordering::Release);
    state.fail_load.store(true, Ordering::Release);
    let client = AcpClient::new_with_spawner(
        core.clone(),
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
    .unwrap();
    assert_eq!(
        client
            .reconcile(SessionScope {
                session_id: launched.session_id.clone(),
            })
            .await
            .unwrap()
            .recovery_method,
        RecoveryMethod::New
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let pending = core
                .read_session_recovery(launched.session_id.clone())
                .await
                .unwrap()
                .pending_agent_work;
            if pending.is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Feedback Resume follows Recovery Prompt");
    assert!(matches!(
        core.get_feedback(GetFeedback {
            request_id: resolved.request.request_id.clone()
        })
        .await
        .unwrap(),
        GetFeedbackOutcome::Terminal(ref delivery) if delivery.delivery_id == resolved.delivery_id
    ));
    let prompts = state.prompts.lock().await;
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].starts_with("[RambleDesk Recovery Context]"));
    assert!(prompts[0].contains("Implement the slice"));
    assert!(prompts[0].contains("Preserve the existing three-column layout"));
    assert!(prompts[0].contains("request_id: recovery-feedback"));
    assert!(prompts[0].contains("delivery_id:"));
    assert!(prompts[1].contains("Call get_feedback"));
    drop(prompts);
    assert_eq!(state.feedback_reads.load(Ordering::Acquire), 1);
    client.shutdown().await.unwrap();
    store.close().await;
}

async fn complete_pending_work(core: &Core, session_id: &rambledesk_core::kernel::SessionId) {
    let batch = core
        .claim_agent_work(WorkScope {
            session_id: Some(session_id.clone()),
            limit: 16,
            lease_seconds: 60,
        })
        .await
        .unwrap();
    assert_eq!(batch.items.len(), 2);
    for claimed in batch.items {
        let request_id = RequestId::new(format!("ramble-work-{}", claimed.work.work_id));
        core.request_feedback(CreateFeedbackRequest {
            request_id: Some(request_id.clone()),
            session_id: session_id.clone(),
            source_link_id: None,
            title: "Completed test stage".to_string(),
            instructions: "Continue the managed Ramble loop.".to_string(),
            actions: vec![FeedbackAction {
                id: "continue".to_string(),
                instruction: "Review the completed stage.".to_string(),
            }],
            context_refs: Vec::new(),
            artifacts: Vec::new(),
        })
        .await
        .unwrap();
        core.record_agent_work(AgentWorkResult {
            work_id: claimed.work.work_id,
            claim_token: claimed.claim_token,
            disposition: AgentWorkDisposition::Completed {
                evidence: AgentWorkEvidence::RambleLoopSuspended { request_id },
            },
        })
        .await
        .unwrap();
    }
}
