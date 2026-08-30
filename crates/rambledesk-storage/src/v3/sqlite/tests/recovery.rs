use super::*;

#[tokio::test]
async fn session_recovery_survives_close_and_reopen() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let launch = seed_launch(&store).await;
    let link = AcpSessionLinkSnapshot {
        link_id: AcpSessionLinkId::new("recovery-link"),
        session_id: launch.session_id.clone(),
        agent_profile_id: "codex".to_owned(),
        launch_profile_id: "local".to_owned(),
        acp_session_id: "acp-recovery".to_owned(),
        capabilities_json: "opaque-capabilities".to_owned(),
        session_toolset_digest: digest('1'),
        is_current: true,
        created_at: NOW.to_owned(),
        last_used_at: LATER.to_owned(),
    };
    store
        .apply(FactMutation::AgentObservation(Box::new(
            AgentObservationCommit {
                observation: AgentObservation::AcpSessionLinked(AcpSessionLinkObservation {
                    session_id: launch.session_id.clone(),
                    agent_profile_id: link.agent_profile_id.clone(),
                    launch_profile_id: link.launch_profile_id.clone(),
                    acp_session_id: link.acp_session_id.clone(),
                    capabilities_json: link.capabilities_json.clone(),
                    session_toolset_digest: link.session_toolset_digest.clone(),
                }),
                link: link.clone(),
            },
        )))
        .await
        .expect("link checkpoint");

    let steering = submission(
        "steering-recovery",
        launch.session_id.as_str(),
        RambleIntent::Steering,
        None,
        digest('2'),
    );
    let steering_work = AgentWorkRecord {
        work_id: AgentWorkId::new("work-steering-recovery"),
        session_id: launch.session_id.clone(),
        kind: AgentWorkKind::SteeringPrompt,
        source_id: steering.submission_id.to_string(),
        payload_digest: digest('3'),
        payload: AgentWorkPayload::Steering {
            submission_id: steering.submission_id.clone(),
            prompt_markdown: steering.body_markdown.clone(),
        },
        state: AgentWorkState::Pending,
        attempt_count: 0,
        last_error_code: None,
        last_error_at: None,
        created_at: LATER.to_owned(),
        completed_at: None,
    };
    store
        .apply(FactMutation::Steering(Box::new(SteeringCommit {
            outcome: SteeringOutcome {
                session_id: launch.session_id.clone(),
                submission_id: steering.submission_id.clone(),
                submission_digest: steering.submission_digest.clone(),
                agent_work_id: steering_work.work_id.clone(),
                agent_work_state: AgentWorkState::Pending,
            },
            submission: steering.clone(),
            work: steering_work.clone(),
        })))
        .await
        .expect("steering");

    let request = waiting_request("recovery-request", launch.session_id.as_str());
    store
        .apply(FactMutation::FeedbackRequest(Box::new(
            FeedbackRequestCommit {
                request: request.clone(),
            },
        )))
        .await
        .expect("feedback request");
    let resolution = cancel_commit(request, "recovery");
    let delivery_id = resolution.delivery.delivery_id.clone();
    let feedback_work_id = resolution.work.work_id.clone();
    store
        .apply(FactMutation::FeedbackResolution(Box::new(resolution)))
        .await
        .expect("feedback resolution");
    store.close().await;

    let reopened = open(&temp).await;
    let snapshot = match reopened
        .query(FactQuery::SessionRecovery(launch.session_id.clone()))
        .await
        .expect("session recovery")
    {
        FactQueryOutcome::SessionRecovery(value) => value,
        _ => panic!("wrong recovery outcome"),
    };
    assert_eq!(snapshot.session.session_id, launch.session_id);
    assert_eq!(snapshot.current_acp_link, Some(link));
    assert_eq!(
        snapshot
            .launch_submission
            .as_ref()
            .map(|value| value.submission_id.as_str()),
        Some("launch-1")
    );
    assert_eq!(snapshot.steering_submissions, vec![steering]);
    assert_eq!(
        snapshot.pending_feedback[0].delivery.delivery_id,
        delivery_id
    );
    assert_eq!(
        snapshot.pending_feedback[0].request.request_id,
        snapshot.pending_feedback[0].delivery.request_id
    );
    let pending_ids = snapshot
        .pending_agent_work
        .iter()
        .map(|value| value.work_id.clone())
        .collect::<Vec<_>>();
    assert!(pending_ids.contains(&steering_work.work_id));
    assert!(pending_ids.contains(&feedback_work_id));
}

#[tokio::test]
async fn recovery_queries_never_mix_resolution_and_work_commits() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let launch = seed_launch(&store).await;
    for index in 0..12 {
        let request = waiting_request(
            &format!("snapshot-request-{index}"),
            launch.session_id.as_str(),
        );
        store
            .apply(FactMutation::FeedbackRequest(Box::new(
                FeedbackRequestCommit {
                    request: request.clone(),
                },
            )))
            .await
            .expect("request");
        let writer = store.clone();
        let reader = store.clone();
        let session_id = launch.session_id.clone();
        let (write_result, read_result) = tokio::join!(
            writer.apply(FactMutation::FeedbackResolution(Box::new(cancel_commit(
                request,
                &format!("snapshot-{index}"),
            )))),
            reader.query(FactQuery::SessionRecovery(session_id)),
        );
        write_result.expect("atomic resolution");
        let snapshot = match read_result.expect("consistent recovery") {
            FactQueryOutcome::SessionRecovery(value) => value,
            _ => panic!("wrong recovery outcome"),
        };
        let resume_work = snapshot
            .pending_agent_work
            .iter()
            .filter(|value| value.kind == AgentWorkKind::FeedbackResume)
            .count();
        assert_eq!(snapshot.pending_feedback.len(), resume_work);
        assert!(snapshot.pending_feedback.iter().all(|value| {
            value.request.request_id == value.delivery.request_id
                && value.request.session_id == value.delivery.session_id
        }));
    }
}
