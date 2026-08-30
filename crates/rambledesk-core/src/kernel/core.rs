use std::{collections::HashSet, sync::Arc};

use time::{Duration, OffsetDateTime};

use super::{
    AcpSessionLinkId, AcpSessionLinkSnapshot, AgentObservation, AgentObservationCommit,
    AgentWorkBatch, AgentWorkDisposition, AgentWorkId, AgentWorkKind, AgentWorkPayload,
    AgentWorkRecord, AgentWorkRecordOutcome, AgentWorkResult, AgentWorkState, ArtifactId,
    CancelFeedbackRequest, CoreError, CoreErrorCode, CreateFeedbackRequest, DeliveredArtifact,
    DeliveryId, DeliveryState, DraftArtifact, DraftCommit, DraftMutation, DraftSnapshot,
    FactMutation, FactMutationOutcome, FactQuery, FactQueryOutcome, FeedbackDeliveryEnvelope,
    FeedbackDeliveryRecord, FeedbackLookup, FeedbackRequestCommit, FeedbackRequestSnapshot,
    FeedbackRequestStatus, FeedbackResolution, FeedbackResolutionCommit, FeedbackResolutionOutcome,
    FeedbackSubmission, GetFeedback, GetFeedbackOutcome, LaunchCommit, LaunchOutcome,
    LaunchSubmission, PackageId, PackagePurpose, PackageRecord, RambleIntent,
    RambleSubmissionRecord, RequestId, ResolveFeedbackRequest, SessionId, SessionKind,
    SessionLifecycle, SessionRecord, SessionRecoverySnapshot, SteeringCommit, SteeringOutcome,
    SteeringSubmission, StoredDraftMutation, StoredWorkResult, WorkClaim, WorkClaimToken,
    WorkScope, WorkbenchQuery, WorkbenchSnapshot,
    core_support::*,
    digest::{
        ManifestDigestInput, agent_work_payload_digest, feedback_request_digest,
        feedback_submission_digest, launch_submission_digest, package_content_digest,
        package_manifest_digest, steering_submission_digest,
    },
    ports::{ArtifactStore, FactStore},
};

#[derive(Clone)]
pub struct Core {
    pub(super) facts: Arc<dyn FactStore>,
    pub(super) artifacts: Arc<dyn ArtifactStore>,
}

impl Core {
    pub fn new(facts: Arc<dyn FactStore>, artifacts: Arc<dyn ArtifactStore>) -> Self {
        Self { facts, artifacts }
    }

    pub async fn launch(&self, input: LaunchSubmission) -> Result<LaunchOutcome, CoreError> {
        validate_id("submission_id", input.submission_id.as_str())?;
        validate_nonblank("title", &input.title, 1, 160)?;
        validate_launch_configuration(&input.launch_configuration)?;
        validate_ramble(&input.ramble.document_json, &input.ramble.body_markdown)?;
        validate_artifacts(&input.ramble.artifacts)?;

        let submission_digest = launch_submission_digest(&input);
        if let Some(assertion) = &input.submission_digest_assertion {
            verify_submission_digest(assertion, &submission_digest)?;
        }

        let now = now()?;
        let session_id = SessionId::new_id();
        let package_id = PackageId::new_id();
        let work_id = AgentWorkId::new_id();
        let (package_artifacts, submission_artifacts) = self
            .stage_launch_package_artifacts(&input.ramble.body_markdown, &input.ramble.artifacts)
            .await?;
        let package_content_digest =
            package_content_digest(PackagePurpose::Launch, None, &package_artifacts);
        let package_manifest_digest = package_manifest_digest(ManifestDigestInput {
            package_id: &package_id,
            submission_id: &input.submission_id,
            purpose: PackagePurpose::Launch,
            request_id: None,
            content_digest: &package_content_digest,
            schema_version: PACKAGE_SCHEMA_VERSION,
            artifacts: &package_artifacts,
            published_at: &now,
        });
        let package = PackageRecord {
            package_id: package_id.clone(),
            submission_id: input.submission_id.clone(),
            purpose: PackagePurpose::Launch,
            request_id: None,
            content_digest: package_content_digest.clone(),
            manifest_digest: package_manifest_digest.clone(),
            schema_version: PACKAGE_SCHEMA_VERSION,
            artifacts: package_artifacts,
            published_at: now.clone(),
        };
        let session = SessionRecord {
            session_id: session_id.clone(),
            kind: SessionKind::Managed,
            title: input.title,
            lifecycle: SessionLifecycle::Ready,
            launch_configuration: Some(input.launch_configuration),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let submission = RambleSubmissionRecord {
            submission_id: input.submission_id.clone(),
            session_id: session_id.clone(),
            intent: RambleIntent::Launch,
            request_id: None,
            document_json: input.ramble.document_json,
            body_markdown: input.ramble.body_markdown.clone(),
            submission_digest: submission_digest.clone(),
            artifacts: submission_artifacts,
            created_at: now.clone(),
        };
        let work = AgentWorkRecord {
            work_id: work_id.clone(),
            session_id: session_id.clone(),
            kind: AgentWorkKind::LaunchPrompt,
            source_id: input.submission_id.to_string(),
            payload_digest: agent_work_payload_digest(
                "launch_prompt",
                input.submission_id.as_str(),
                &input.ramble.body_markdown,
            ),
            payload: AgentWorkPayload::Launch {
                submission_id: input.submission_id.clone(),
                package_id: package_id.clone(),
                prompt_markdown: input.ramble.body_markdown,
            },
            state: AgentWorkState::Pending,
            attempt_count: 0,
            last_error_code: None,
            last_error_at: None,
            created_at: now,
            completed_at: None,
        };
        let outcome = LaunchOutcome {
            session_id,
            submission_id: input.submission_id,
            submission_digest,
            package_id,
            package_content_digest,
            package_manifest_digest,
            agent_work_id: work_id,
            agent_work_state: AgentWorkState::Pending,
        };
        match self
            .facts
            .apply(FactMutation::Launch(Box::new(LaunchCommit {
                session,
                submission,
                package,
                work,
                outcome: outcome.clone(),
            })))
            .await?
        {
            FactMutationOutcome::Launch(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn steer(&self, input: SteeringSubmission) -> Result<SteeringOutcome, CoreError> {
        validate_id("submission_id", input.submission_id.as_str())?;
        validate_id("session_id", input.session_id.as_str())?;
        validate_ramble(&input.ramble.document_json, &input.ramble.body_markdown)?;
        validate_artifacts(&input.ramble.artifacts)?;
        let submission_digest = steering_submission_digest(&input);
        if let Some(assertion) = &input.submission_digest_assertion {
            verify_submission_digest(assertion, &submission_digest)?;
        }

        let now = now()?;
        let work_id = AgentWorkId::new_id();
        let submission_artifacts = self
            .stage_submission_artifacts(&input.ramble.artifacts)
            .await?;
        let submission = RambleSubmissionRecord {
            submission_id: input.submission_id.clone(),
            session_id: input.session_id.clone(),
            intent: RambleIntent::Steering,
            request_id: None,
            document_json: input.ramble.document_json,
            body_markdown: input.ramble.body_markdown.clone(),
            submission_digest: submission_digest.clone(),
            artifacts: submission_artifacts,
            created_at: now.clone(),
        };
        let work = AgentWorkRecord {
            work_id: work_id.clone(),
            session_id: input.session_id.clone(),
            kind: AgentWorkKind::SteeringPrompt,
            source_id: input.submission_id.to_string(),
            payload_digest: agent_work_payload_digest(
                "steering_prompt",
                input.submission_id.as_str(),
                &input.ramble.body_markdown,
            ),
            payload: AgentWorkPayload::Steering {
                submission_id: input.submission_id.clone(),
                prompt_markdown: input.ramble.body_markdown,
            },
            state: AgentWorkState::Pending,
            attempt_count: 0,
            last_error_code: None,
            last_error_at: None,
            created_at: now,
            completed_at: None,
        };
        let outcome = SteeringOutcome {
            session_id: input.session_id,
            submission_id: input.submission_id,
            submission_digest,
            agent_work_id: work_id,
            agent_work_state: AgentWorkState::Pending,
        };
        match self
            .facts
            .apply(FactMutation::Steering(Box::new(SteeringCommit {
                submission,
                work,
                outcome: outcome.clone(),
            })))
            .await?
        {
            FactMutationOutcome::Steering(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn request_feedback(
        &self,
        input: CreateFeedbackRequest,
    ) -> Result<FeedbackRequestSnapshot, CoreError> {
        validate_id("session_id", input.session_id.as_str())?;
        if let Some(source_link_id) = &input.source_link_id {
            validate_id("source_link_id", source_link_id.as_str())?;
        }
        validate_nonblank("title", &input.title, 1, 160)?;
        validate_nonblank("instructions", &input.instructions, 1, 12_000)?;
        validate_actions(&input.actions)?;
        validate_context_refs(&input.context_refs)?;
        validate_artifacts(&input.artifacts)?;
        let request_id = input.request_id.unwrap_or_else(RequestId::new_id);
        validate_id("request_id", request_id.as_str())?;
        let input_digest = feedback_request_digest(
            input.session_id.as_str(),
            input.source_link_id.as_ref(),
            &input.title,
            &input.instructions,
            &input.actions,
            &input.context_refs,
            &input.artifacts,
        );
        let request_artifacts = self.stage_request_artifacts(&input.artifacts).await?;
        let created_at = now()?;
        let request = FeedbackRequestSnapshot {
            request_id,
            session_id: input.session_id,
            source_link_id: input.source_link_id,
            title: input.title,
            instructions: input.instructions,
            actions: input.actions,
            context_refs: input.context_refs,
            input_digest,
            status: FeedbackRequestStatus::Waiting,
            resolution: None,
            response_package_id: None,
            cancel_reason: None,
            request_artifacts,
            created_at,
            resolved_at: None,
        };
        match self
            .facts
            .apply(FactMutation::FeedbackRequest(Box::new(
                FeedbackRequestCommit {
                    request: request.clone(),
                },
            )))
            .await?
        {
            FactMutationOutcome::FeedbackRequest(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn resolve_feedback(
        &self,
        input: ResolveFeedbackRequest,
    ) -> Result<FeedbackResolutionOutcome, CoreError> {
        match input {
            ResolveFeedbackRequest::Submit(input) => self.submit_feedback(input).await,
            ResolveFeedbackRequest::Cancel(input) => self.cancel_feedback(input).await,
        }
    }

    pub async fn mutate_draft(&self, input: DraftMutation) -> Result<DraftSnapshot, CoreError> {
        let mutation = match input {
            DraftMutation::Save(input) => {
                validate_id("draft_id", input.draft_id.as_str())?;
                validate_draft_identity(&input)?;
                validate_ramble(&input.document_json, &input.body_markdown)?;
                StoredDraftMutation::Save(input)
            }
            DraftMutation::AddArtifact(input) => {
                validate_id("draft_id", input.draft_id.as_str())?;
                validate_artifacts(std::slice::from_ref(&input.artifact))?;
                let blob = self.put_blob(&input.artifact.contents).await?;
                StoredDraftMutation::AddArtifact {
                    draft_id: input.draft_id,
                    expected_revision: input.expected_revision,
                    artifact: DraftArtifact {
                        artifact_id: ArtifactId::new_id(),
                        position: u32::MAX,
                        display_name: input.artifact.display_name,
                        media_type: input.artifact.media_type,
                        size_bytes: blob.size_bytes,
                        sha256: blob.sha256,
                        storage_key: blob.storage_key,
                    },
                }
            }
            DraftMutation::RemoveArtifact(input) => {
                validate_id("draft_id", input.draft_id.as_str())?;
                validate_id("artifact_id", input.artifact_id.as_str())?;
                StoredDraftMutation::RemoveArtifact(input)
            }
            DraftMutation::ReorderArtifacts(input) => {
                validate_id("draft_id", input.draft_id.as_str())?;
                let mut ids = HashSet::new();
                for id in &input.artifact_ids {
                    validate_id("artifact_id", id.as_str())?;
                    if !ids.insert(id.as_str()) {
                        return Err(CoreError::invalid_argument("artifact_ids must be unique"));
                    }
                }
                StoredDraftMutation::ReorderArtifacts(input)
            }
        };
        match self
            .facts
            .apply(FactMutation::Draft(Box::new(DraftCommit {
                mutation,
                now: now()?,
            })))
            .await?
        {
            FactMutationOutcome::Draft(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn get_feedback(&self, input: GetFeedback) -> Result<GetFeedbackOutcome, CoreError> {
        validate_id("request_id", input.request_id.as_str())?;
        match self
            .facts
            .query(FactQuery::Feedback(input.request_id))
            .await?
        {
            FactQueryOutcome::Feedback(FeedbackLookup::Waiting { request, .. }) => {
                Ok(GetFeedbackOutcome::Waiting {
                    request_id: request.request_id,
                    session_id: request.session_id,
                })
            }
            FactQueryOutcome::Feedback(FeedbackLookup::Terminal { delivery, .. }) => {
                let mut artifacts = Vec::new();
                if let Some(package) = &delivery.package {
                    for artifact in &package.artifacts {
                        let contents = self
                            .artifacts
                            .open_verified(&artifact.storage_key, &artifact.sha256)
                            .await?;
                        if contents.len() as u64 != artifact.size_bytes {
                            return Err(CoreError::new(
                                CoreErrorCode::ArtifactDigestMismatch,
                                "artifact size does not match Package metadata",
                                false,
                            ));
                        }
                        artifacts.push(DeliveredArtifact {
                            artifact_id: artifact.artifact_id.clone(),
                            role: artifact.role.clone(),
                            position: artifact.position,
                            display_name: artifact.display_name.clone(),
                            media_type: artifact.media_type.clone(),
                            size_bytes: artifact.size_bytes,
                            sha256: artifact.sha256.clone(),
                            contents,
                        });
                    }
                }
                let package_id = delivery
                    .package
                    .as_ref()
                    .map(|value| value.package_id.clone());
                let package_content_digest = delivery
                    .package
                    .as_ref()
                    .map(|value| value.content_digest.clone());
                let package_manifest_digest = delivery
                    .package
                    .as_ref()
                    .map(|value| value.manifest_digest.clone());
                Ok(GetFeedbackOutcome::Terminal(FeedbackDeliveryEnvelope {
                    delivery_id: delivery.delivery_id,
                    request_id: delivery.request_id,
                    session_id: delivery.session_id,
                    resolution: delivery.resolution,
                    package_id,
                    package_content_digest,
                    package_manifest_digest,
                    artifacts,
                    cancel_reason: delivery.cancel_reason,
                }))
            }
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn read_workbench(
        &self,
        query: WorkbenchQuery,
    ) -> Result<WorkbenchSnapshot, CoreError> {
        if let Some(session_id) = &query.session_id {
            validate_id("session_id", session_id.as_str())?;
        }
        match self.facts.query(FactQuery::Workbench(query)).await? {
            FactQueryOutcome::Workbench(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn read_session_recovery(
        &self,
        session_id: SessionId,
    ) -> Result<SessionRecoverySnapshot, CoreError> {
        validate_id("session_id", session_id.as_str())?;
        match self
            .facts
            .query(FactQuery::SessionRecovery(session_id))
            .await?
        {
            FactQueryOutcome::SessionRecovery(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn claim_agent_work(&self, scope: WorkScope) -> Result<AgentWorkBatch, CoreError> {
        if !(1..=100).contains(&scope.limit) {
            return Err(CoreError::invalid_argument(
                "work claim limit must be between 1 and 100",
            ));
        }
        if !(1..=3_600).contains(&scope.lease_seconds) {
            return Err(CoreError::invalid_argument(
                "work lease must be between 1 and 3600 seconds",
            ));
        }
        let claimed = OffsetDateTime::now_utc();
        let lease_until = claimed + Duration::seconds(i64::from(scope.lease_seconds));
        self.facts
            .claim_work(WorkClaim {
                scope,
                claim_token: WorkClaimToken::new_id(),
                claimed_at: format_time(claimed)?,
                lease_until: format_time(lease_until)?,
            })
            .await
            .map_err(Into::into)
    }

    pub async fn record_agent_observation(
        &self,
        observation: AgentObservation,
    ) -> Result<AcpSessionLinkSnapshot, CoreError> {
        let AgentObservation::AcpSessionLinked(input) = &observation;
        validate_id("session_id", input.session_id.as_str())?;
        validate_nonblank("agent_profile_id", &input.agent_profile_id, 1, 128)?;
        validate_nonblank("launch_profile_id", &input.launch_profile_id, 1, 128)?;
        validate_text("acp_session_id", &input.acp_session_id, 1, 4_096)?;
        validate_text("capabilities_json", &input.capabilities_json, 1, 100_000)?;
        validate_digest("session_toolset_digest", &input.session_toolset_digest)?;
        let timestamp = now()?;
        let link = AcpSessionLinkSnapshot {
            link_id: AcpSessionLinkId::new_id(),
            session_id: input.session_id.clone(),
            agent_profile_id: input.agent_profile_id.clone(),
            launch_profile_id: input.launch_profile_id.clone(),
            acp_session_id: input.acp_session_id.clone(),
            capabilities_json: input.capabilities_json.clone(),
            session_toolset_digest: input.session_toolset_digest.clone(),
            is_current: true,
            created_at: timestamp.clone(),
            last_used_at: timestamp,
        };
        match self
            .facts
            .apply(FactMutation::AgentObservation(Box::new(
                AgentObservationCommit {
                    observation,
                    link: link.clone(),
                },
            )))
            .await?
        {
            FactMutationOutcome::AgentObservation(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn record_agent_work(
        &self,
        result: AgentWorkResult,
    ) -> Result<AgentWorkRecordOutcome, CoreError> {
        validate_id("work_id", result.work_id.as_str())?;
        validate_id("claim_token", result.claim_token.as_str())?;
        if let AgentWorkDisposition::Retry { error_code } = &result.disposition {
            validate_nonblank("error_code", error_code, 1, 256)?;
        }
        self.facts
            .record_work(StoredWorkResult {
                result,
                recorded_at: now()?,
            })
            .await
            .map_err(Into::into)
    }

    async fn submit_feedback(
        &self,
        input: FeedbackSubmission,
    ) -> Result<FeedbackResolutionOutcome, CoreError> {
        validate_id("submission_id", input.submission_id.as_str())?;
        validate_id("request_id", input.request_id.as_str())?;
        validate_ramble(&input.document_json, &input.feedback_markdown)?;
        validate_text("uncooked_markdown", &input.uncooked_markdown, 1, 100_000)?;
        validate_artifacts(&input.artifacts)?;
        if let Some(model) = &input.cooking_model {
            validate_text("cooking_model", model, 1, 500)?;
        }
        let submission_digest = feedback_submission_digest(&input);
        if let Some(assertion) = &input.submission_digest_assertion {
            verify_submission_digest(assertion, &submission_digest)?;
        }
        let (request, session) = self.lookup_request(&input.request_id).await?;
        if session.kind != SessionKind::Managed {
            return Err(CoreError::from(
                super::ports::FactStoreError::SessionNotManaged,
            ));
        }
        let session_id = session.session_id;
        let now = now()?;
        let package_id = PackageId::new_id();
        let delivery_id = DeliveryId::new_id();
        let work_id = AgentWorkId::new_id();
        let (package_artifacts, submission_artifacts) = self
            .stage_response_package_artifacts(
                &input.feedback_markdown,
                &input.uncooked_markdown,
                &input.artifacts,
            )
            .await?;
        let package_content_digest = package_content_digest(
            PackagePurpose::Response,
            Some(&input.request_id),
            &package_artifacts,
        );
        let package_manifest_digest = package_manifest_digest(ManifestDigestInput {
            package_id: &package_id,
            submission_id: &input.submission_id,
            purpose: PackagePurpose::Response,
            request_id: Some(&input.request_id),
            content_digest: &package_content_digest,
            schema_version: PACKAGE_SCHEMA_VERSION,
            artifacts: &package_artifacts,
            published_at: &now,
        });
        let package = PackageRecord {
            package_id: package_id.clone(),
            submission_id: input.submission_id.clone(),
            purpose: PackagePurpose::Response,
            request_id: Some(input.request_id.clone()),
            content_digest: package_content_digest.clone(),
            manifest_digest: package_manifest_digest.clone(),
            schema_version: PACKAGE_SCHEMA_VERSION,
            artifacts: package_artifacts,
            published_at: now.clone(),
        };
        let submission = RambleSubmissionRecord {
            submission_id: input.submission_id,
            session_id: session_id.clone(),
            intent: RambleIntent::Feedback,
            request_id: Some(input.request_id.clone()),
            document_json: input.document_json,
            body_markdown: input.feedback_markdown,
            submission_digest,
            artifacts: submission_artifacts,
            created_at: now.clone(),
        };
        let mut resolved_request = request;
        resolved_request.status = FeedbackRequestStatus::Submitted;
        resolved_request.resolution = Some(FeedbackResolution::Submitted);
        resolved_request.response_package_id = Some(package_id.clone());
        resolved_request.resolved_at = Some(now.clone());
        let delivery = FeedbackDeliveryRecord {
            delivery_id: delivery_id.clone(),
            request_id: input.request_id.clone(),
            session_id: session_id.clone(),
            resolution: FeedbackResolution::Submitted,
            package: Some(package.clone()),
            cancel_reason: None,
            state: DeliveryState::Pending,
            attempt_count: 0,
            last_error_code: None,
            last_error_at: None,
            created_at: now.clone(),
            delivered_at: None,
        };
        let work = feedback_resume_work(
            work_id.clone(),
            session_id,
            delivery_id.clone(),
            input.request_id.clone(),
            now,
        );
        let outcome = FeedbackResolutionOutcome {
            request: resolved_request,
            package_id: Some(package_id),
            package_content_digest: Some(package_content_digest),
            package_manifest_digest: Some(package_manifest_digest),
            delivery_id,
            delivery_state: DeliveryState::Pending,
            agent_work_id: work_id,
        };
        match self
            .facts
            .apply(FactMutation::FeedbackResolution(Box::new(
                FeedbackResolutionCommit {
                    request_id: input.request_id,
                    expected_draft_revision: Some(input.expected_draft_revision),
                    submission: Some(submission),
                    package: Some(package),
                    resolution: FeedbackResolution::Submitted,
                    cancel_reason: None,
                    delivery,
                    work,
                    outcome: outcome.clone(),
                },
            )))
            .await?
        {
            FactMutationOutcome::FeedbackResolution(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    async fn cancel_feedback(
        &self,
        input: CancelFeedbackRequest,
    ) -> Result<FeedbackResolutionOutcome, CoreError> {
        validate_id("request_id", input.request_id.as_str())?;
        validate_nonblank("reason", &input.reason, 1, 4_000)?;
        let (request, session) = self.lookup_request(&input.request_id).await?;
        if session.kind != SessionKind::Managed {
            return Err(CoreError::from(
                super::ports::FactStoreError::SessionNotManaged,
            ));
        }
        let session_id = session.session_id;
        let now = now()?;
        let delivery_id = DeliveryId::new_id();
        let work_id = AgentWorkId::new_id();
        let mut resolved_request = request;
        resolved_request.status = FeedbackRequestStatus::Cancelled;
        resolved_request.resolution = Some(FeedbackResolution::Cancelled);
        resolved_request.cancel_reason = Some(input.reason.clone());
        resolved_request.resolved_at = Some(now.clone());
        let delivery = FeedbackDeliveryRecord {
            delivery_id: delivery_id.clone(),
            request_id: input.request_id.clone(),
            session_id: session_id.clone(),
            resolution: FeedbackResolution::Cancelled,
            package: None,
            cancel_reason: Some(input.reason.clone()),
            state: DeliveryState::Pending,
            attempt_count: 0,
            last_error_code: None,
            last_error_at: None,
            created_at: now.clone(),
            delivered_at: None,
        };
        let work = feedback_resume_work(
            work_id.clone(),
            session_id,
            delivery_id.clone(),
            input.request_id.clone(),
            now,
        );
        let outcome = FeedbackResolutionOutcome {
            request: resolved_request,
            package_id: None,
            package_content_digest: None,
            package_manifest_digest: None,
            delivery_id,
            delivery_state: DeliveryState::Pending,
            agent_work_id: work_id,
        };
        match self
            .facts
            .apply(FactMutation::FeedbackResolution(Box::new(
                FeedbackResolutionCommit {
                    request_id: input.request_id,
                    expected_draft_revision: None,
                    submission: None,
                    package: None,
                    resolution: FeedbackResolution::Cancelled,
                    cancel_reason: Some(input.reason),
                    delivery,
                    work,
                    outcome: outcome.clone(),
                },
            )))
            .await?
        {
            FactMutationOutcome::FeedbackResolution(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    async fn lookup_request(
        &self,
        request_id: &RequestId,
    ) -> Result<(FeedbackRequestSnapshot, SessionRecord), CoreError> {
        match self
            .facts
            .query(FactQuery::Feedback(request_id.clone()))
            .await?
        {
            FactQueryOutcome::Feedback(FeedbackLookup::Waiting { request, session }) => {
                Ok((request, session))
            }
            FactQueryOutcome::Feedback(FeedbackLookup::Terminal {
                request, session, ..
            }) => Ok((request, session)),
            _ => Err(unexpected_store_outcome()),
        }
    }
}
