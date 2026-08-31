use super::{
    AcpSessionLinkId, AcpSessionLinkObservation, AgentObservation, AgentWorkDisposition,
    AgentWorkEvidence, AgentWorkId, AgentWorkKind, AgentWorkResult, AgentWorkState, ArtifactInput,
    CancelFeedbackRequest, CoreErrorCode, CreateFeedbackRequest, DeliveryState, DraftId,
    DraftMutation, FeedbackAction, MAX_ARTIFACT_BYTES, RambleContent, RambleIntent, RequestId,
    ResolveFeedbackRequest, SaveDraft, SessionId, SessionKind, SessionLifecycle, SessionRecord,
    SteeringSubmission, StoredWorkResult, WorkClaim, WorkClaimToken, WorkScope, WorkbenchQuery,
    digest::launch_submission_digest,
    ports::FactStore,
    test_fixtures::{
        create_request, feedback_submission, harness, launch_input, launch_session,
        save_feedback_draft,
    },
};

#[test]
fn feedback_request_interface_rejects_oversized_artifacts_before_storage() {
    let input = CreateFeedbackRequest {
        request_id: Some(RequestId::from("oversized-request")),
        session_id: SessionId::from("oversized-session"),
        source_link_id: None,
        title: "Review".to_owned(),
        instructions: "Review the work.".to_owned(),
        actions: vec![FeedbackAction {
            id: "review".to_owned(),
            instruction: "Review it.".to_owned(),
        }],
        context_refs: Vec::new(),
        artifacts: vec![ArtifactInput {
            display_name: "oversized.bin".to_owned(),
            media_type: "application/octet-stream".to_owned(),
            contents: vec![0; MAX_ARTIFACT_BYTES + 1],
        }],
    };

    assert_eq!(
        super::validate_feedback_request_input(&input)
            .expect_err("oversized Artifact")
            .code(),
        CoreErrorCode::InvalidArgument
    );
}

#[tokio::test]
async fn launch_draft_requires_and_preserves_launch_configuration() {
    let (core, _) = harness();
    let configuration = launch_input("draft-config", "Draft").launch_configuration;
    let draft = core
        .mutate_draft(DraftMutation::Save(SaveDraft {
            draft_id: DraftId::from("launch-draft"),
            intent: RambleIntent::Launch,
            session_id: None,
            request_id: None,
            launch_configuration: Some(configuration.clone()),
            document_json: r#"{"type":"doc"}"#.to_owned(),
            body_markdown: "Launch later".to_owned(),
            expected_revision: 0,
        }))
        .await
        .expect("save launch draft");
    assert_eq!(draft.launch_configuration, Some(configuration));

    let error = core
        .mutate_draft(DraftMutation::Save(SaveDraft {
            draft_id: DraftId::from("invalid-launch-draft"),
            intent: RambleIntent::Launch,
            session_id: None,
            request_id: None,
            launch_configuration: None,
            document_json: r#"{"type":"doc"}"#.to_owned(),
            body_markdown: "Missing config".to_owned(),
            expected_revision: 0,
        }))
        .await
        .expect_err("missing configuration must fail");
    assert_eq!(error.code(), CoreErrorCode::InvalidArgument);
}

#[tokio::test]
async fn draft_identity_is_immutable_while_launch_configuration_is_editable() {
    let (core, _) = harness();
    let mut configuration = launch_input("draft-identity", "Draft").launch_configuration;
    core.mutate_draft(DraftMutation::Save(SaveDraft {
        draft_id: DraftId::from("identity-draft"),
        intent: RambleIntent::Launch,
        session_id: None,
        request_id: None,
        launch_configuration: Some(configuration.clone()),
        document_json: "{}".to_owned(),
        body_markdown: "First".to_owned(),
        expected_revision: 0,
    }))
    .await
    .expect("first draft");
    configuration.model = Some("new-model".to_owned());
    let updated = core
        .mutate_draft(DraftMutation::Save(SaveDraft {
            draft_id: DraftId::from("identity-draft"),
            intent: RambleIntent::Launch,
            session_id: None,
            request_id: None,
            launch_configuration: Some(configuration.clone()),
            document_json: "{}".to_owned(),
            body_markdown: "Second".to_owned(),
            expected_revision: 1,
        }))
        .await
        .expect("edit launch configuration");
    assert_eq!(updated.launch_configuration, Some(configuration));

    let error = core
        .mutate_draft(DraftMutation::Save(SaveDraft {
            draft_id: DraftId::from("identity-draft"),
            intent: RambleIntent::Steering,
            session_id: Some(super::SessionId::from("different-session")),
            request_id: None,
            launch_configuration: None,
            document_json: "{}".to_owned(),
            body_markdown: "Rebound".to_owned(),
            expected_revision: 2,
        }))
        .await
        .expect_err("draft identity cannot change");
    assert_eq!(error.code(), CoreErrorCode::DraftConflict);
}

#[tokio::test]
async fn agent_json_fields_are_opaque_to_core() {
    let (core, _) = harness();
    let mut launch = launch_input("opaque-agent-json", "Launch");
    launch.launch_configuration.agent_config_json = "   ".to_owned();
    launch.submission_digest_assertion = Some(launch_submission_digest(&launch));
    core.launch(launch).await.expect("opaque Agent config");

    let session = launch_session(&core).await;
    core.record_agent_observation(AgentObservation::AcpSessionLinked(
        AcpSessionLinkObservation {
            session_id: session.session_id,
            agent_profile_id: "codex".to_owned(),
            launch_profile_id: "local".to_owned(),
            acp_session_id: "acp-session".to_owned(),
            capabilities_json: "tools".to_owned(),
            session_toolset_digest: format!("sha256:{}", "a".repeat(64)),
        },
    ))
    .await
    .expect("opaque capabilities");
}

#[tokio::test]
async fn acp_session_link_checkpoint_reuses_identity_and_refreshes_state() {
    let (core, facts) = harness();
    let launch = launch_session(&core).await;
    let session_id = launch.session_id;
    let first = core
        .record_agent_observation(AgentObservation::AcpSessionLinked(
            AcpSessionLinkObservation {
                session_id: session_id.clone(),
                agent_profile_id: "codex".to_owned(),
                launch_profile_id: "local".to_owned(),
                acp_session_id: "checkpoint-one".to_owned(),
                capabilities_json: r#"{"version":1}"#.to_owned(),
                session_toolset_digest: format!("sha256:{}", "1".repeat(64)),
            },
        ))
        .await
        .expect("first checkpoint");
    core.record_agent_observation(AgentObservation::AcpSessionLinked(
        AcpSessionLinkObservation {
            session_id: session_id.clone(),
            agent_profile_id: "codex".to_owned(),
            launch_profile_id: "local".to_owned(),
            acp_session_id: "checkpoint-two".to_owned(),
            capabilities_json: "{}".to_owned(),
            session_toolset_digest: format!("sha256:{}", "2".repeat(64)),
        },
    ))
    .await
    .expect("replacement checkpoint");
    let resumed = core
        .record_agent_observation(AgentObservation::AcpSessionLinked(
            AcpSessionLinkObservation {
                session_id,
                agent_profile_id: "codex".to_owned(),
                launch_profile_id: "local".to_owned(),
                acp_session_id: "checkpoint-one".to_owned(),
                capabilities_json: r#"{"version":2}"#.to_owned(),
                session_toolset_digest: format!("sha256:{}", "3".repeat(64)),
            },
        ))
        .await
        .expect("resume checkpoint");

    assert_eq!(resumed.link_id, first.link_id);
    assert_eq!(resumed.created_at, first.created_at);
    assert_eq!(resumed.capabilities_json, r#"{"version":2}"#);
    assert_eq!(
        resumed.session_toolset_digest,
        format!("sha256:{}", "3".repeat(64))
    );
    assert!(resumed.is_current);
    let links = facts.links();
    assert_eq!(links.len(), 2);
    assert_eq!(links.iter().filter(|link| link.is_current).count(), 1);
    assert!(
        links
            .iter()
            .find(|link| link.link_id == resumed.link_id)
            .unwrap()
            .is_current
    );
    let workbench = core
        .read_workbench(super::WorkbenchQuery {
            session_id: Some(resumed.session_id.clone()),
        })
        .await
        .expect("resume workbench");
    assert_eq!(workbench.current_acp_links, vec![resumed]);
}

#[tokio::test]
async fn document_json_is_an_opaque_structured_editor_payload() {
    let (core, _) = harness();
    let mut launch = launch_input("opaque-document", "Launch");
    launch.ramble.document_json = "not json".to_owned();
    launch.submission_digest_assertion = Some(launch_submission_digest(&launch));
    core.launch(launch).await.expect("opaque document payload");
}

#[tokio::test]
async fn feedback_request_preserves_source_link_provenance() {
    let (core, _) = harness();
    let launch = launch_session(&core).await;
    let session_id = launch.session_id;
    let link = core
        .record_agent_observation(AgentObservation::AcpSessionLinked(
            AcpSessionLinkObservation {
                session_id: session_id.clone(),
                agent_profile_id: "codex".to_owned(),
                launch_profile_id: "local".to_owned(),
                acp_session_id: "provenance-acp-session".to_owned(),
                capabilities_json: "{}".to_owned(),
                session_toolset_digest: format!("sha256:{}", "b".repeat(64)),
            },
        ))
        .await
        .expect("record source link");
    let request = core
        .request_feedback(CreateFeedbackRequest {
            request_id: Some(RequestId::from("provenance-request")),
            session_id,
            source_link_id: Some(link.link_id.clone()),
            title: "Review".to_owned(),
            instructions: "Review the work.".to_owned(),
            actions: vec![FeedbackAction {
                id: "review".to_owned(),
                instruction: "Review it.".to_owned(),
            }],
            context_refs: Vec::new(),
            artifacts: Vec::new(),
        })
        .await
        .expect("request feedback");
    assert_eq!(request.source_link_id, Some(link.link_id));
}

#[tokio::test]
async fn feedback_request_rejects_unknown_source_link() {
    let (core, _) = harness();
    let launch = launch_session(&core).await;
    let error = core
        .request_feedback(CreateFeedbackRequest {
            request_id: Some(RequestId::from("unknown-link-request")),
            session_id: launch.session_id,
            source_link_id: Some(AcpSessionLinkId::from("unknown-link")),
            title: "Review".to_owned(),
            instructions: "Review the work.".to_owned(),
            actions: vec![FeedbackAction {
                id: "review".to_owned(),
                instruction: "Review it.".to_owned(),
            }],
            context_refs: Vec::new(),
            artifacts: Vec::new(),
        })
        .await
        .expect_err("unknown source link");
    assert_eq!(error.code(), CoreErrorCode::AcpSessionLinkNotFound);
}

#[tokio::test]
async fn submission_digest_assertion_is_optional_but_checked_when_present() {
    let (core, _) = harness();
    let mut without_assertion = launch_input("computed-by-core", "Launch");
    without_assertion.submission_digest_assertion = None;
    core.launch(without_assertion)
        .await
        .expect("Core computes digest");

    let mut mismatch = launch_input("assertion-mismatch", "Launch");
    mismatch.submission_digest_assertion = Some(format!("sha256:{}", "0".repeat(64)));
    let error = core.launch(mismatch).await.expect_err("mismatch");
    assert_eq!(error.code(), CoreErrorCode::InvalidArgument);
}

#[tokio::test]
async fn semantic_text_rejects_blank_values_before_the_storage_seam() {
    let (core, _) = harness();
    let mut blank_title = launch_input("blank-title", "Launch");
    blank_title.title = " \t ".to_owned();
    assert_eq!(
        core.launch(blank_title)
            .await
            .expect_err("blank title")
            .code(),
        CoreErrorCode::InvalidArgument
    );

    let launch = launch_session(&core).await;
    let blank_request = core
        .request_feedback(CreateFeedbackRequest {
            request_id: Some(RequestId::from("blank-request")),
            session_id: launch.session_id.clone(),
            source_link_id: None,
            title: "Review".to_owned(),
            instructions: "   ".to_owned(),
            actions: vec![FeedbackAction {
                id: "review".to_owned(),
                instruction: "Review".to_owned(),
            }],
            context_refs: Vec::new(),
            artifacts: Vec::new(),
        })
        .await
        .expect_err("blank instructions");
    assert_eq!(blank_request.code(), CoreErrorCode::InvalidArgument);

    let mut blank_artifact = launch_input("blank-artifact", "Launch");
    blank_artifact.ramble.artifacts.push(ArtifactInput {
        display_name: "  ".to_owned(),
        media_type: "image/png".to_owned(),
        contents: vec![1],
    });
    assert_eq!(
        core.launch(blank_artifact)
            .await
            .expect_err("blank artifact name")
            .code(),
        CoreErrorCode::InvalidArgument
    );
}

#[tokio::test]
async fn imported_session_allows_feedback_drafts_but_rejects_agent_delivery() {
    let (core, facts) = harness();
    let session_id = SessionId::from("imported-session");
    facts.insert_session(SessionRecord {
        session_id: session_id.clone(),
        kind: SessionKind::Imported,
        title: "Imported".to_owned(),
        lifecycle: SessionLifecycle::Ready,
        launch_configuration: None,
        pinned_at: None,
        archived_at: None,
        created_at: "2026-08-30T00:00:00Z".to_owned(),
        updated_at: "2026-08-30T00:00:00Z".to_owned(),
    });
    let steer_error = core
        .steer(SteeringSubmission {
            submission_id: super::SubmissionId::from("imported-steer"),
            session_id: session_id.clone(),
            submission_digest_assertion: None,
            ramble: RambleContent {
                document_json: "{}".to_owned(),
                body_markdown: "Steer".to_owned(),
                artifacts: Vec::new(),
            },
        })
        .await
        .expect_err("imported steer");
    assert_eq!(steer_error.code(), CoreErrorCode::SessionNotManaged);
    let link_error = core
        .record_agent_observation(AgentObservation::AcpSessionLinked(
            AcpSessionLinkObservation {
                session_id: session_id.clone(),
                agent_profile_id: "codex".to_owned(),
                launch_profile_id: "local".to_owned(),
                acp_session_id: "imported-acp".to_owned(),
                capabilities_json: "{}".to_owned(),
                session_toolset_digest: format!("sha256:{}", "4".repeat(64)),
            },
        ))
        .await
        .expect_err("imported link");
    assert_eq!(link_error.code(), CoreErrorCode::SessionNotManaged);

    let request = create_request(&core, session_id, "imported-feedback").await;
    save_feedback_draft(
        &core,
        request.session_id.clone(),
        request.request_id.clone(),
    )
    .await;
    let error = core
        .resolve_feedback(ResolveFeedbackRequest::Cancel(CancelFeedbackRequest {
            request_id: request.request_id,
            reason: "No response needed".to_owned(),
        }))
        .await
        .expect_err("imported feedback delivery");
    assert_eq!(error.code(), CoreErrorCode::SessionNotManaged);
    let claimed = core
        .claim_agent_work(WorkScope {
            session_id: Some(request.session_id),
            limit: 10,
            lease_seconds: 60,
        })
        .await
        .expect("imported work query");
    assert!(claimed.items.is_empty());
}

#[tokio::test]
async fn retry_releases_work_without_completing_feedback_delivery() {
    let (core, _) = harness();
    let launch = launch_session(&core).await;
    let launch_claim = core
        .claim_agent_work(WorkScope {
            session_id: Some(launch.session_id.clone()),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("claim launch");
    core.record_agent_work(AgentWorkResult {
        work_id: launch_claim.items[0].work.work_id.clone(),
        claim_token: launch_claim.items[0].claim_token.clone(),
        disposition: AgentWorkDisposition::Completed {
            evidence: AgentWorkEvidence::PromptTurnCompleted,
        },
    })
    .await
    .expect("complete launch");
    let request = create_request(&core, launch.session_id.clone(), "retry-request").await;
    let draft =
        save_feedback_draft(&core, launch.session_id.clone(), request.request_id.clone()).await;
    let resolution = core
        .resolve_feedback(ResolveFeedbackRequest::Submit(feedback_submission(
            request.request_id,
            draft.revision,
        )))
        .await
        .expect("submit feedback");
    let first_claim = core
        .claim_agent_work(WorkScope {
            session_id: Some(launch.session_id.clone()),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("claim feedback resume");
    let retry_result = AgentWorkResult {
        work_id: first_claim.items[0].work.work_id.clone(),
        claim_token: first_claim.items[0].claim_token.clone(),
        disposition: AgentWorkDisposition::Retry {
            error_code: "ACP_DISCONNECTED".to_owned(),
        },
    };
    let retry = core
        .record_agent_work(retry_result.clone())
        .await
        .expect("record retry");
    assert_eq!(retry.state, AgentWorkState::Pending);
    assert!(retry.delivered.is_none());
    let workbench = core
        .read_workbench(WorkbenchQuery {
            session_id: Some(launch.session_id.clone()),
        })
        .await
        .expect("workbench after retry");
    assert_eq!(
        workbench.pending_deliveries[0].state,
        DeliveryState::Pending
    );
    assert_eq!(
        workbench.pending_deliveries[0].delivery_id,
        resolution.delivery_id
    );
    assert_eq!(workbench.pending_deliveries[0].attempt_count, 1);
    assert_eq!(
        workbench.pending_deliveries[0].last_error_code.as_deref(),
        Some("ACP_DISCONNECTED")
    );
    assert_eq!(
        workbench.pending_agent_work[0].last_error_code.as_deref(),
        Some("ACP_DISCONNECTED")
    );
    let stale = core
        .record_agent_work(retry_result)
        .await
        .expect_err("old token was released");
    assert_eq!(stale.code(), CoreErrorCode::WorkClaimConflict);
    let second_claim = core
        .claim_agent_work(WorkScope {
            session_id: Some(launch.session_id),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("reclaim");
    assert_eq!(
        second_claim.items[0].work.work_id,
        first_claim.items[0].work.work_id
    );
    assert_ne!(
        second_claim.items[0].claim_token,
        first_claim.items[0].claim_token
    );
    assert_eq!(second_claim.items[0].work.attempt_count, 2);
    assert_eq!(
        second_claim.items[0].work.last_error_code.as_deref(),
        Some("ACP_DISCONNECTED")
    );
    let reclaimed = core
        .read_workbench(WorkbenchQuery {
            session_id: Some(second_claim.items[0].work.session_id.clone()),
        })
        .await
        .expect("workbench after reclaim");
    assert_eq!(reclaimed.pending_deliveries[0].attempt_count, 2);
    assert_eq!(
        reclaimed.pending_deliveries[0].last_error_code.as_deref(),
        Some("ACP_DISCONNECTED")
    );
}

#[tokio::test]
async fn expired_claim_is_reclaimed_in_stable_order() {
    let (core, facts) = harness();
    let launch = launch_session(&core).await;
    core.steer(SteeringSubmission {
        submission_id: super::SubmissionId::from("reclaim-steer"),
        session_id: launch.session_id.clone(),
        submission_digest_assertion: None,
        ramble: RambleContent {
            document_json: "{}".to_owned(),
            body_markdown: "Second work item".to_owned(),
            artifacts: Vec::new(),
        },
    })
    .await
    .expect("create steering work");
    let scope = WorkScope {
        session_id: Some(launch.session_id),
        limit: 10,
        lease_seconds: 60,
    };
    let first = facts
        .claim_work(WorkClaim {
            scope: scope.clone(),
            claim_token: WorkClaimToken::from("expired-token"),
            claimed_at: "2026-08-30T00:00:00Z".to_owned(),
            lease_until: "2026-08-30T00:01:00Z".to_owned(),
        })
        .await
        .expect("first claim");
    let first_order = first
        .items
        .iter()
        .map(|item| (item.work.created_at.clone(), item.work.work_id.clone()))
        .collect::<Vec<_>>();
    assert!(first_order.windows(2).all(|pair| pair[0] <= pair[1]));
    let reclaimed = facts
        .claim_work(WorkClaim {
            scope,
            claim_token: WorkClaimToken::from("replacement-token"),
            claimed_at: "2026-08-30T00:02:00Z".to_owned(),
            lease_until: "2026-08-30T00:03:00Z".to_owned(),
        })
        .await
        .expect("expired reclaim");
    assert_eq!(reclaimed.items.len(), first.items.len());
    assert!(
        reclaimed
            .items
            .iter()
            .all(|item| item.work.attempt_count == 2)
    );
    assert!(
        reclaimed
            .items
            .iter()
            .all(|item| item.claim_token == WorkClaimToken::from("replacement-token"))
    );
}

#[tokio::test]
async fn lease_expiry_is_exclusive_for_record_and_inclusive_for_reclaim() {
    let (core, facts) = harness();
    let launch = launch_session(&core).await;
    let scope = WorkScope {
        session_id: Some(launch.session_id),
        limit: 1,
        lease_seconds: 60,
    };
    let first = facts
        .claim_work(WorkClaim {
            scope: scope.clone(),
            claim_token: WorkClaimToken::from("boundary-old"),
            claimed_at: "2026-08-30T00:00:00Z".to_owned(),
            lease_until: "2026-08-30T00:01:00Z".to_owned(),
        })
        .await
        .expect("boundary claim");
    let expired = facts
        .record_work(StoredWorkResult {
            result: AgentWorkResult {
                work_id: first.items[0].work.work_id.clone(),
                claim_token: first.items[0].claim_token.clone(),
                disposition: AgentWorkDisposition::Completed {
                    evidence: AgentWorkEvidence::PromptTurnCompleted,
                },
            },
            recorded_at: "2026-08-30T00:01:00Z".to_owned(),
        })
        .await
        .expect_err("completion at expiry");
    assert_eq!(expired, super::ports::FactStoreError::WorkClaimConflict);
    let reclaimed = facts
        .claim_work(WorkClaim {
            scope,
            claim_token: WorkClaimToken::from("boundary-new"),
            claimed_at: "2026-08-30T00:01:00Z".to_owned(),
            lease_until: "2026-08-30T00:02:00Z".to_owned(),
        })
        .await
        .expect("reclaim at expiry");
    assert_eq!(reclaimed.items[0].work.work_id, first.items[0].work.work_id);

    let unknown = facts
        .record_work(StoredWorkResult {
            result: AgentWorkResult {
                work_id: AgentWorkId::from("unknown-work"),
                claim_token: WorkClaimToken::from("unknown-token"),
                disposition: AgentWorkDisposition::Retry {
                    error_code: "UNKNOWN".to_owned(),
                },
            },
            recorded_at: "2026-08-30T00:00:00Z".to_owned(),
        })
        .await
        .expect_err("unknown work");
    assert_eq!(unknown, super::ports::FactStoreError::WorkNotFound);
}

#[tokio::test]
async fn session_recovery_reads_durable_rambles_and_feedback_context() {
    let (core, _) = harness();
    let launch = launch_session(&core).await;
    let session_id = launch.session_id.clone();
    let link = core
        .record_agent_observation(AgentObservation::AcpSessionLinked(
            AcpSessionLinkObservation {
                session_id: session_id.clone(),
                agent_profile_id: "codex".to_owned(),
                launch_profile_id: "local".to_owned(),
                acp_session_id: "recovery-acp".to_owned(),
                capabilities_json: "{}".to_owned(),
                session_toolset_digest: format!("sha256:{}", "5".repeat(64)),
            },
        ))
        .await
        .expect("record recovery link");
    core.steer(SteeringSubmission {
        submission_id: super::SubmissionId::from("recovery-steer"),
        session_id: session_id.clone(),
        submission_digest_assertion: None,
        ramble: RambleContent {
            document_json: "{}".to_owned(),
            body_markdown: "Continue with the reviewed approach.".to_owned(),
            artifacts: Vec::new(),
        },
    })
    .await
    .expect("persist steering");
    let request = create_request(&core, session_id.clone(), "recovery-request").await;
    let expected_title = request.title.clone();
    let expected_actions = request.actions.clone();
    let resolution = core
        .resolve_feedback(ResolveFeedbackRequest::Cancel(CancelFeedbackRequest {
            request_id: request.request_id,
            reason: "Recover without this feedback".to_owned(),
        }))
        .await
        .expect("persist pending delivery");

    let recovery = core
        .read_session_recovery(session_id.clone())
        .await
        .expect("session recovery");
    assert_eq!(recovery.session.session_id, session_id);
    assert_eq!(
        recovery.current_acp_link.map(|value| value.link_id),
        Some(link.link_id)
    );
    assert_eq!(
        recovery
            .launch_submission
            .as_ref()
            .map(|value| value.submission_id.clone()),
        Some(launch.submission_id)
    );
    assert_eq!(recovery.steering_submissions.len(), 1);
    assert_eq!(
        recovery.steering_submissions[0].body_markdown,
        "Continue with the reviewed approach."
    );
    assert_eq!(recovery.pending_feedback.len(), 1);
    assert_eq!(recovery.pending_feedback[0].request.title, expected_title);
    assert_eq!(
        recovery.pending_feedback[0].request.actions,
        expected_actions
    );
    assert_eq!(
        recovery.pending_feedback[0].delivery.delivery_id,
        resolution.delivery_id
    );
    assert!(recovery.pending_agent_work.iter().any(|work| {
        work.kind == AgentWorkKind::FeedbackResume && work.state == AgentWorkState::Pending
    }));
}

#[tokio::test]
async fn idempotent_replay_returns_the_current_work_and_delivery_projection() {
    let (core, _) = harness();
    let launch_input = launch_input("current-replay", "Launch");
    let launch = core
        .launch(launch_input.clone())
        .await
        .expect("first launch");
    let launch_claim = core
        .claim_agent_work(WorkScope {
            session_id: Some(launch.session_id.clone()),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("claim launch");
    core.record_agent_work(AgentWorkResult {
        work_id: launch_claim.items[0].work.work_id.clone(),
        claim_token: launch_claim.items[0].claim_token.clone(),
        disposition: AgentWorkDisposition::Completed {
            evidence: AgentWorkEvidence::PromptTurnCompleted,
        },
    })
    .await
    .expect("complete launch");
    let launch_replay = core.launch(launch_input).await.expect("launch replay");
    assert_eq!(launch_replay.agent_work_state, AgentWorkState::Completed);

    let request = create_request(&core, launch.session_id.clone(), "delivery-replay").await;
    let draft =
        save_feedback_draft(&core, launch.session_id.clone(), request.request_id.clone()).await;
    let feedback = feedback_submission(request.request_id, draft.revision);
    let first = core
        .resolve_feedback(ResolveFeedbackRequest::Submit(feedback.clone()))
        .await
        .expect("first resolution");
    let feedback_claim = core
        .claim_agent_work(WorkScope {
            session_id: Some(launch.session_id),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("claim feedback resume");
    core.record_agent_work(AgentWorkResult {
        work_id: feedback_claim.items[0].work.work_id.clone(),
        claim_token: feedback_claim.items[0].claim_token.clone(),
        disposition: AgentWorkDisposition::Completed {
            evidence: AgentWorkEvidence::FeedbackConsumedAndTurnCompleted {
                delivery_id: first.delivery_id.clone(),
            },
        },
    })
    .await
    .expect("complete feedback resume");
    let replay = core
        .resolve_feedback(ResolveFeedbackRequest::Submit(feedback))
        .await
        .expect("resolution replay");
    assert_eq!(replay.delivery_state, DeliveryState::Delivered);
}
