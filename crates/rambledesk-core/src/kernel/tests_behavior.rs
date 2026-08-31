use super::{
    AgentWorkDisposition, AgentWorkEvidence, AgentWorkResult, AgentWorkState,
    CancelFeedbackRequest, CoreErrorCode, DeliveryState, DraftMutation, FeedbackRequestStatus,
    GetFeedback, GetFeedbackOutcome, RambleIntent, ResolveFeedbackRequest, SaveDraft, WorkScope,
    WorkbenchQuery,
    test_fixtures::{
        create_request, feedback_submission, harness, launch_input, launch_session,
        save_feedback_draft,
    },
};

#[tokio::test]
async fn launch_is_idempotent_and_conflicts_on_changed_content() {
    let (core, facts) = harness();
    let input = launch_input("launch-stable", "Build it.");
    let first = core.launch(input.clone()).await.expect("first launch");
    let replay = core.launch(input).await.expect("launch replay");
    assert_eq!(replay, first);
    facts.inspect(|state| {
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(state.submissions.len(), 1);
        assert_eq!(state.packages.len(), 1);
        assert_eq!(state.work.len(), 1);
    });
    let changed = launch_input("launch-stable", "Build something else.");
    let error = core.launch(changed).await.expect_err("must conflict");
    assert_eq!(error.code(), CoreErrorCode::IdempotencyConflict);
}

#[tokio::test]
async fn feedback_submission_commits_offline_facts_atomically() {
    let (core, facts) = harness();
    let launch = launch_session(&core).await;
    let request = create_request(&core, launch.session_id.clone(), "request-1").await;
    let draft = save_feedback_draft(&core, launch.session_id, request.request_id.clone()).await;
    let outcome = core
        .resolve_feedback(ResolveFeedbackRequest::Submit(feedback_submission(
            request.request_id.clone(),
            draft.revision,
        )))
        .await
        .expect("submit offline");
    assert_eq!(outcome.request.status, FeedbackRequestStatus::Submitted);
    assert_eq!(outcome.delivery_state, DeliveryState::Pending);
    assert!(outcome.package_id.is_some());
    assert!(outcome.package_content_digest.is_some());
    assert!(outcome.package_manifest_digest.is_some());
    facts.inspect(|state| {
        assert_eq!(state.deliveries.len(), 1);
        assert_eq!(state.resolution_outcomes.len(), 1);
        assert_eq!(
            state
                .work
                .get(&outcome.agent_work_id)
                .map(|value| value.state),
            Some(AgentWorkState::Pending)
        );
    });
    let workbench = core
        .read_workbench(WorkbenchQuery {
            session_id: Some(outcome.request.session_id),
        })
        .await
        .expect("workbench after submission");
    assert!(workbench.drafts.is_empty());
}

#[tokio::test]
async fn cancellation_creates_delivery_and_work_but_no_package() {
    let (core, facts) = harness();
    let launch = launch_session(&core).await;
    let request = create_request(&core, launch.session_id, "request-cancel").await;
    let packages_before = facts.inspect(|state| state.packages.len());
    let submissions_before = facts.inspect(|state| state.submissions.len());
    let outcome = core
        .resolve_feedback(ResolveFeedbackRequest::Cancel(CancelFeedbackRequest {
            request_id: request.request_id,
            reason: "The build changed.".to_owned(),
        }))
        .await
        .expect("cancel");
    assert_eq!(outcome.request.status, FeedbackRequestStatus::Cancelled);
    assert!(outcome.package_id.is_none());
    facts.inspect(|state| {
        assert_eq!(state.packages.len(), packages_before);
        assert_eq!(state.submissions.len(), submissions_before);
        assert_eq!(state.deliveries.len(), 1);
    });
}

#[tokio::test]
async fn draft_cas_does_not_change_waiting_request() {
    let (core, _) = harness();
    let launch = launch_session(&core).await;
    let request = create_request(&core, launch.session_id.clone(), "request-draft").await;
    let draft =
        save_feedback_draft(&core, launch.session_id.clone(), request.request_id.clone()).await;
    assert_eq!(draft.revision, 1);
    let error = core
        .mutate_draft(DraftMutation::Save(SaveDraft {
            draft_id: draft.draft_id,
            intent: RambleIntent::Feedback,
            session_id: Some(launch.session_id),
            request_id: Some(request.request_id.clone()),
            launch_configuration: None,
            document_json: r#"{"type":"doc"}"#.to_owned(),
            body_markdown: "Stale write".to_owned(),
            expected_revision: 0,
        }))
        .await
        .expect_err("stale revision");
    assert_eq!(error.code(), CoreErrorCode::DraftConflict);
    assert!(matches!(
        core.get_feedback(GetFeedback {
            request_id: request.request_id
        })
        .await
        .expect("get waiting"),
        GetFeedbackOutcome::Waiting { .. }
    ));
}

#[tokio::test]
async fn terminal_feedback_reads_a_stable_delivery_envelope() {
    let (core, _) = harness();
    let launch = launch_session(&core).await;
    let request = create_request(&core, launch.session_id.clone(), "request-envelope").await;
    let draft = save_feedback_draft(&core, launch.session_id, request.request_id.clone()).await;
    let outcome = core
        .resolve_feedback(ResolveFeedbackRequest::Submit(feedback_submission(
            request.request_id.clone(),
            draft.revision,
        )))
        .await
        .expect("submit");
    let first = core
        .get_feedback(GetFeedback {
            request_id: request.request_id.clone(),
        })
        .await
        .expect("first read");
    let second = core
        .get_feedback(GetFeedback {
            request_id: request.request_id,
        })
        .await
        .expect("second read");
    assert_eq!(first, second);
    let GetFeedbackOutcome::Terminal(envelope) = first else {
        panic!("terminal envelope expected")
    };
    assert_eq!(envelope.delivery_id, outcome.delivery_id);
    assert_eq!(envelope.package_id, outcome.package_id);
    assert_eq!(envelope.artifacts.len(), 2);
}

#[tokio::test]
async fn claiming_work_does_not_complete_it() {
    let (core, _) = harness();
    let launch = launch_session(&core).await;
    let batch = core
        .claim_agent_work(WorkScope {
            session_id: Some(launch.session_id.clone()),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("claim");
    assert_eq!(batch.items.len(), 1);
    assert_eq!(batch.items[0].work.state, AgentWorkState::Claimed);
    let snapshot = core
        .read_workbench(WorkbenchQuery {
            session_id: Some(launch.session_id),
        })
        .await
        .expect("workbench");
    assert_eq!(snapshot.pending_agent_work.len(), 1);
    assert_eq!(
        snapshot.pending_agent_work[0].state,
        AgentWorkState::Claimed
    );
    let checkpoint = create_request(
        &core,
        batch.items[0].work.session_id.clone(),
        "workbench-completion-checkpoint",
    )
    .await;
    let completed = core
        .record_agent_work(AgentWorkResult {
            work_id: batch.items[0].work.work_id.clone(),
            claim_token: batch.items[0].claim_token.clone(),
            disposition: AgentWorkDisposition::Completed {
                evidence: AgentWorkEvidence::RambleLoopSuspended {
                    request_id: checkpoint.request_id,
                },
            },
        })
        .await
        .expect("complete");
    assert_eq!(completed.state, AgentWorkState::Completed);
}
