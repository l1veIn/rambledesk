use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use async_trait::async_trait;

use super::{
    AcpSessionLinkSnapshot, AgentWorkBatch, AgentWorkDisposition, AgentWorkEvidence,
    AgentWorkPayload, AgentWorkRecordOutcome, AgentWorkState, ClaimedAgentWork, DeliveryState,
    DraftSnapshot, FactMutation, FactMutationOutcome, FactQuery, FactQueryOutcome, FeedbackLookup,
    FeedbackRequestStatus, FeedbackResolution, FeedbackResolutionOutcome, LaunchOutcome, PackageId,
    RambleIntent, RequestId, SessionId, SessionRecord, StoredDraftMutation, StoredWorkResult,
    SubmissionId, WorkClaim, WorkbenchSnapshot,
    ports::{FactStore, FactStoreError},
    test_adapters::normalize_positions,
};

mod memory_helpers;
mod session_organization;

#[derive(Default)]
pub(super) struct MemoryState {
    pub(super) sessions: HashMap<SessionId, SessionRecord>,
    pub(super) submissions: HashMap<SubmissionId, super::RambleSubmissionRecord>,
    pub(super) launch_outcomes: HashMap<SubmissionId, LaunchOutcome>,
    pub(super) steering_outcomes: HashMap<SubmissionId, super::SteeringOutcome>,
    pub(super) packages: HashMap<PackageId, super::PackageRecord>,
    pub(super) requests: HashMap<RequestId, super::FeedbackRequestSnapshot>,
    pub(super) deliveries: HashMap<RequestId, super::FeedbackDeliveryRecord>,
    pub(super) resolution_outcomes: HashMap<RequestId, FeedbackResolutionOutcome>,
    pub(super) drafts: HashMap<super::DraftId, DraftSnapshot>,
    pub(super) work: HashMap<super::AgentWorkId, super::AgentWorkRecord>,
    pub(super) claims: HashMap<super::AgentWorkId, (super::WorkClaimToken, String)>,
    pub(super) links: Vec<AcpSessionLinkSnapshot>,
}

#[derive(Default)]
pub(super) struct MemoryFactStore {
    state: Mutex<MemoryState>,
}

#[async_trait]
impl FactStore for MemoryFactStore {
    async fn apply(&self, mutation: FactMutation) -> Result<FactMutationOutcome, FactStoreError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match mutation {
            FactMutation::Launch(commit) => {
                if let Some(existing) = state.submissions.get(&commit.submission.submission_id) {
                    if existing.submission_digest != commit.submission.submission_digest {
                        return Err(FactStoreError::IdempotencyConflict);
                    }
                    let mut outcome = state
                        .launch_outcomes
                        .get(&commit.submission.submission_id)
                        .cloned()
                        .ok_or(FactStoreError::CorruptData);
                    if let Ok(outcome) = &mut outcome {
                        outcome.agent_work_state = state
                            .work
                            .get(&outcome.agent_work_id)
                            .ok_or(FactStoreError::CorruptData)?
                            .state;
                    }
                    return outcome.map(FactMutationOutcome::Launch);
                }
                state
                    .sessions
                    .insert(commit.session.session_id.clone(), commit.session);
                state.submissions.insert(
                    commit.submission.submission_id.clone(),
                    commit.submission.clone(),
                );
                state
                    .packages
                    .insert(commit.package.package_id.clone(), commit.package);
                state.work.insert(commit.work.work_id.clone(), commit.work);
                state
                    .launch_outcomes
                    .insert(commit.submission.submission_id, commit.outcome.clone());
                Ok(FactMutationOutcome::Launch(commit.outcome))
            }
            FactMutation::Steering(commit) => {
                match state.sessions.get(&commit.submission.session_id) {
                    None => return Err(FactStoreError::SessionNotFound),
                    Some(session) if session.kind != super::SessionKind::Managed => {
                        return Err(FactStoreError::SessionNotManaged);
                    }
                    Some(_) => {}
                }
                if let Some(existing) = state.submissions.get(&commit.submission.submission_id) {
                    if existing.submission_digest != commit.submission.submission_digest {
                        return Err(FactStoreError::IdempotencyConflict);
                    }
                    let mut outcome = state
                        .steering_outcomes
                        .get(&commit.submission.submission_id)
                        .cloned()
                        .ok_or(FactStoreError::CorruptData);
                    if let Ok(outcome) = &mut outcome {
                        outcome.agent_work_state = state
                            .work
                            .get(&outcome.agent_work_id)
                            .ok_or(FactStoreError::CorruptData)?
                            .state;
                    }
                    return outcome.map(FactMutationOutcome::Steering);
                }
                state.submissions.insert(
                    commit.submission.submission_id.clone(),
                    commit.submission.clone(),
                );
                state.work.insert(commit.work.work_id.clone(), commit.work);
                state
                    .steering_outcomes
                    .insert(commit.submission.submission_id, commit.outcome.clone());
                Ok(FactMutationOutcome::Steering(commit.outcome))
            }
            FactMutation::FeedbackRequest(commit) => {
                if !state.sessions.contains_key(&commit.request.session_id) {
                    return Err(FactStoreError::SessionNotFound);
                }
                if commit
                    .request
                    .source_link_id
                    .as_ref()
                    .is_some_and(|link_id| {
                        !state.links.iter().any(|link| {
                            link.link_id == *link_id && link.session_id == commit.request.session_id
                        })
                    })
                {
                    return Err(FactStoreError::AcpSessionLinkNotFound);
                }
                if let Some(existing) = state.requests.get(&commit.request.request_id) {
                    if existing.input_digest != commit.request.input_digest {
                        return Err(FactStoreError::IdempotencyConflict);
                    }
                    return Ok(FactMutationOutcome::FeedbackRequest(existing.clone()));
                }
                state
                    .requests
                    .insert(commit.request.request_id.clone(), commit.request.clone());
                Ok(FactMutationOutcome::FeedbackRequest(commit.request))
            }
            FactMutation::FeedbackResolution(commit) => {
                let existing = state
                    .requests
                    .get(&commit.request_id)
                    .cloned()
                    .ok_or(FactStoreError::RequestNotFound)?;
                if state
                    .sessions
                    .get(&existing.session_id)
                    .is_none_or(|session| session.kind != super::SessionKind::Managed)
                {
                    return Err(FactStoreError::SessionNotManaged);
                }
                if existing.status != FeedbackRequestStatus::Waiting {
                    match commit.resolution {
                        FeedbackResolution::Submitted => {
                            let same = commit.submission.as_ref().is_some_and(|incoming| {
                                state.submissions.values().any(|stored| {
                                    stored.request_id.as_ref() == Some(&commit.request_id)
                                        && stored.submission_id == incoming.submission_id
                                        && stored.submission_digest == incoming.submission_digest
                                })
                            });
                            if !same {
                                return Err(
                                    if existing.resolution == Some(FeedbackResolution::Submitted) {
                                        FactStoreError::IdempotencyConflict
                                    } else {
                                        FactStoreError::RequestTerminal
                                    },
                                );
                            }
                        }
                        FeedbackResolution::Cancelled => {
                            if existing.resolution != Some(FeedbackResolution::Cancelled)
                                || existing.cancel_reason != commit.cancel_reason
                            {
                                return Err(FactStoreError::RequestTerminal);
                            }
                        }
                    }
                    let mut outcome = state
                        .resolution_outcomes
                        .get(&commit.request_id)
                        .cloned()
                        .ok_or(FactStoreError::CorruptData)?;
                    outcome.delivery_state = state
                        .deliveries
                        .get(&commit.request_id)
                        .ok_or(FactStoreError::CorruptData)?
                        .state;
                    return Ok(FactMutationOutcome::FeedbackResolution(outcome));
                }
                if let Some(expected) = commit.expected_draft_revision {
                    let revision = state
                        .drafts
                        .values()
                        .find(|value| value.request_id.as_ref() == Some(&commit.request_id))
                        .map(|value| value.revision)
                        .unwrap_or(0);
                    if revision != expected {
                        return Err(FactStoreError::DraftConflict);
                    }
                }
                if commit.resolution == FeedbackResolution::Submitted
                    && (commit.submission.is_none() || commit.package.is_none())
                {
                    return Err(FactStoreError::CorruptData);
                }
                if commit.resolution == FeedbackResolution::Cancelled
                    && (commit.submission.is_some() || commit.package.is_some())
                {
                    return Err(FactStoreError::CorruptData);
                }
                state
                    .requests
                    .insert(commit.request_id.clone(), commit.outcome.request.clone());
                if let Some(submission) = commit.submission {
                    state
                        .submissions
                        .insert(submission.submission_id.clone(), submission);
                }
                if let Some(package) = commit.package {
                    state.packages.insert(package.package_id.clone(), package);
                }
                state
                    .deliveries
                    .insert(commit.request_id.clone(), commit.delivery);
                state.work.insert(commit.work.work_id.clone(), commit.work);
                state
                    .drafts
                    .retain(|_, draft| draft.request_id.as_ref() != Some(&commit.request_id));
                state
                    .resolution_outcomes
                    .insert(commit.request_id, commit.outcome.clone());
                Ok(FactMutationOutcome::FeedbackResolution(commit.outcome))
            }
            FactMutation::Draft(commit) => {
                let draft = match commit.mutation {
                    StoredDraftMutation::Save(input) => {
                        let existing = state.drafts.get(&input.draft_id).cloned();
                        let revision = existing.as_ref().map(|value| value.revision).unwrap_or(0);
                        if revision != input.expected_revision {
                            return Err(FactStoreError::DraftConflict);
                        }
                        if existing.as_ref().is_some_and(|draft| {
                            draft.intent != input.intent
                                || draft.session_id != input.session_id
                                || draft.request_id != input.request_id
                        }) {
                            return Err(FactStoreError::DraftConflict);
                        }
                        let created_at = existing
                            .as_ref()
                            .map(|value| value.created_at.clone())
                            .unwrap_or_else(|| commit.now.clone());
                        let artifacts = existing.map(|value| value.artifacts).unwrap_or_default();
                        DraftSnapshot {
                            draft_id: input.draft_id,
                            intent: input.intent,
                            session_id: input.session_id,
                            request_id: input.request_id,
                            launch_configuration: input.launch_configuration,
                            document_json: input.document_json,
                            body_markdown: input.body_markdown,
                            revision: revision + 1,
                            artifacts,
                            created_at,
                            updated_at: commit.now,
                        }
                    }
                    StoredDraftMutation::AddArtifact {
                        draft_id,
                        expected_revision,
                        mut artifact,
                    } => {
                        let mut existing = state
                            .drafts
                            .get(&draft_id)
                            .cloned()
                            .ok_or(FactStoreError::DraftConflict)?;
                        if existing.revision != expected_revision {
                            return Err(FactStoreError::DraftConflict);
                        }
                        artifact.position = existing.artifacts.len() as u32;
                        existing.artifacts.push(artifact);
                        existing.revision += 1;
                        existing.updated_at = commit.now;
                        existing
                    }
                    StoredDraftMutation::RemoveArtifact(input) => {
                        let mut existing = state
                            .drafts
                            .get(&input.draft_id)
                            .cloned()
                            .ok_or(FactStoreError::DraftConflict)?;
                        if existing.revision != input.expected_revision {
                            return Err(FactStoreError::DraftConflict);
                        }
                        existing
                            .artifacts
                            .retain(|value| value.artifact_id != input.artifact_id);
                        normalize_positions(&mut existing.artifacts);
                        existing.revision += 1;
                        existing.updated_at = commit.now;
                        existing
                    }
                    StoredDraftMutation::ReorderArtifacts(input) => {
                        let mut existing = state
                            .drafts
                            .get(&input.draft_id)
                            .cloned()
                            .ok_or(FactStoreError::DraftConflict)?;
                        if existing.revision != input.expected_revision
                            || existing.artifacts.len() != input.artifact_ids.len()
                        {
                            return Err(FactStoreError::DraftConflict);
                        }
                        let mut next = Vec::with_capacity(existing.artifacts.len());
                        for id in input.artifact_ids {
                            next.push(
                                existing
                                    .artifacts
                                    .iter()
                                    .find(|value| value.artifact_id == id)
                                    .cloned()
                                    .ok_or(FactStoreError::DraftConflict)?,
                            );
                        }
                        existing.artifacts = next;
                        normalize_positions(&mut existing.artifacts);
                        existing.revision += 1;
                        existing.updated_at = commit.now;
                        existing
                    }
                };
                state.drafts.insert(draft.draft_id.clone(), draft.clone());
                Ok(FactMutationOutcome::Draft(draft))
            }
            FactMutation::AgentObservation(commit) => {
                let super::AgentObservation::AcpSessionLinked(input) = &commit.observation;
                match state.sessions.get(&input.session_id) {
                    None => return Err(FactStoreError::SessionNotFound),
                    Some(session) if session.kind != super::SessionKind::Managed => {
                        return Err(FactStoreError::SessionNotManaged);
                    }
                    Some(_) => {}
                }
                let existing = state.links.iter().position(|value| {
                    value.session_id == input.session_id
                        && value.agent_profile_id == input.agent_profile_id
                        && value.launch_profile_id == input.launch_profile_id
                        && value.acp_session_id == input.acp_session_id
                });
                for link in &mut state.links {
                    if link.session_id == input.session_id {
                        link.is_current = false;
                    }
                }
                if let Some(position) = existing {
                    let link = &mut state.links[position];
                    link.capabilities_json = input.capabilities_json.clone();
                    link.session_toolset_digest = input.session_toolset_digest.clone();
                    link.is_current = true;
                    link.last_used_at = commit.link.last_used_at;
                    return Ok(FactMutationOutcome::AgentObservation(link.clone()));
                }
                state.links.push(commit.link.clone());
                Ok(FactMutationOutcome::AgentObservation(commit.link))
            }
            FactMutation::SessionOrganization(commit) => {
                session_organization::apply(&mut state, *commit)
            }
        }
    }

    async fn query(&self, query: FactQuery) -> Result<FactQueryOutcome, FactStoreError> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match query {
            FactQuery::Feedback(request_id) => {
                let request = state
                    .requests
                    .get(&request_id)
                    .cloned()
                    .ok_or(FactStoreError::RequestNotFound)?;
                if request.status == FeedbackRequestStatus::Waiting {
                    let session = state
                        .sessions
                        .get(&request.session_id)
                        .cloned()
                        .ok_or(FactStoreError::CorruptData)?;
                    Ok(FactQueryOutcome::Feedback(FeedbackLookup::Waiting {
                        request,
                        session,
                    }))
                } else {
                    let delivery = state
                        .deliveries
                        .get(&request_id)
                        .cloned()
                        .ok_or(FactStoreError::CorruptData)?;
                    let session = state
                        .sessions
                        .get(&request.session_id)
                        .cloned()
                        .ok_or(FactStoreError::CorruptData)?;
                    Ok(FactQueryOutcome::Feedback(FeedbackLookup::Terminal {
                        request,
                        session,
                        delivery: Box::new(delivery),
                    }))
                }
            }
            FactQuery::Workbench(query) => {
                let matches = |session_id: &SessionId| {
                    query
                        .session_id
                        .as_ref()
                        .is_none_or(|expected| expected == session_id)
                        && state
                            .sessions
                            .get(session_id)
                            .is_some_and(|session| session.archived_at.is_none())
                };
                let mut sessions = state
                    .sessions
                    .values()
                    .filter(|value| matches(&value.session_id))
                    .cloned()
                    .collect::<Vec<_>>();
                sessions.sort_by(|left, right| {
                    right.pinned_at.cmp(&left.pinned_at).then_with(|| {
                        right
                            .updated_at
                            .cmp(&left.updated_at)
                            .then_with(|| right.session_id.cmp(&left.session_id))
                    })
                });
                let mut current_acp_links = state
                    .links
                    .iter()
                    .filter(|value| value.is_current && matches(&value.session_id))
                    .cloned()
                    .collect::<Vec<_>>();
                current_acp_links.sort_by(|left, right| {
                    right
                        .last_used_at
                        .cmp(&left.last_used_at)
                        .then_with(|| right.link_id.cmp(&left.link_id))
                });
                let mut feedback_requests = state
                    .requests
                    .values()
                    .filter(|value| matches(&value.session_id))
                    .cloned()
                    .collect::<Vec<_>>();
                feedback_requests.sort_by(|left, right| {
                    left.created_at
                        .cmp(&right.created_at)
                        .then_with(|| left.request_id.cmp(&right.request_id))
                });
                let waiting_feedback = feedback_requests
                    .iter()
                    .filter(|value| value.status == FeedbackRequestStatus::Waiting)
                    .cloned()
                    .collect();
                let mut drafts = state
                    .drafts
                    .values()
                    .filter(|value| value.session_id.as_ref().is_none_or(&matches))
                    .cloned()
                    .collect::<Vec<_>>();
                drafts.sort_by(|left, right| {
                    right
                        .updated_at
                        .cmp(&left.updated_at)
                        .then_with(|| right.draft_id.cmp(&left.draft_id))
                });
                let mut pending_deliveries = state
                    .deliveries
                    .values()
                    .filter(|value| {
                        matches(&value.session_id) && value.state == DeliveryState::Pending
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                pending_deliveries.sort_by(|left, right| {
                    left.created_at
                        .cmp(&right.created_at)
                        .then_with(|| left.delivery_id.cmp(&right.delivery_id))
                });
                let mut pending_agent_work = state
                    .work
                    .values()
                    .filter(|value| {
                        matches(&value.session_id) && value.state != AgentWorkState::Completed
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                pending_agent_work.sort_by(|left, right| {
                    left.created_at
                        .cmp(&right.created_at)
                        .then_with(|| left.work_id.cmp(&right.work_id))
                });
                Ok(FactQueryOutcome::Workbench(WorkbenchSnapshot {
                    sessions,
                    current_acp_links,
                    feedback_requests,
                    waiting_feedback,
                    drafts,
                    pending_deliveries,
                    pending_agent_work,
                }))
            }
            FactQuery::ArchivedSessions => Ok(FactQueryOutcome::ArchivedSessions(
                session_organization::archived(&state),
            )),
            FactQuery::SessionRecovery(session_id) => {
                let session = state
                    .sessions
                    .get(&session_id)
                    .cloned()
                    .ok_or(FactStoreError::SessionNotFound)?;
                let current_links = state
                    .links
                    .iter()
                    .filter(|link| link.session_id == session_id && link.is_current)
                    .cloned()
                    .collect::<Vec<_>>();
                if current_links.len() > 1 {
                    return Err(FactStoreError::CorruptData);
                }
                let mut launches = state
                    .submissions
                    .values()
                    .filter(|submission| {
                        submission.session_id == session_id
                            && submission.intent == RambleIntent::Launch
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let launch_shape_is_valid = match session.kind {
                    super::SessionKind::Managed => launches.len() == 1,
                    super::SessionKind::Imported => launches.is_empty(),
                };
                if !launch_shape_is_valid {
                    return Err(FactStoreError::CorruptData);
                }
                let mut steering_submissions = state
                    .submissions
                    .values()
                    .filter(|submission| {
                        submission.session_id == session_id
                            && submission.intent == RambleIntent::Steering
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                steering_submissions.sort_by(|left, right| {
                    left.created_at
                        .cmp(&right.created_at)
                        .then_with(|| left.submission_id.cmp(&right.submission_id))
                });
                let mut pending_feedback = state
                    .deliveries
                    .values()
                    .filter(|delivery| {
                        delivery.session_id == session_id
                            && delivery.state == DeliveryState::Pending
                    })
                    .map(|delivery| {
                        let request = state
                            .requests
                            .get(&delivery.request_id)
                            .filter(|request| {
                                request.session_id == session_id
                                    && request.request_id == delivery.request_id
                                    && request.status != FeedbackRequestStatus::Waiting
                            })
                            .cloned()
                            .ok_or(FactStoreError::CorruptData)?;
                        Ok(super::PendingFeedbackRecovery {
                            request,
                            delivery: delivery.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, FactStoreError>>()?;
                pending_feedback.sort_by(|left, right| {
                    left.delivery
                        .created_at
                        .cmp(&right.delivery.created_at)
                        .then_with(|| left.delivery.delivery_id.cmp(&right.delivery.delivery_id))
                });
                let mut pending_agent_work = state
                    .work
                    .values()
                    .filter(|work| {
                        work.session_id == session_id && work.state != AgentWorkState::Completed
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                pending_agent_work.sort_by(|left, right| {
                    left.created_at
                        .cmp(&right.created_at)
                        .then_with(|| left.work_id.cmp(&right.work_id))
                });
                Ok(FactQueryOutcome::SessionRecovery(
                    super::SessionRecoverySnapshot {
                        session,
                        current_acp_link: current_links.into_iter().next(),
                        launch_submission: launches.pop(),
                        steering_submissions,
                        pending_feedback,
                        pending_agent_work,
                    },
                ))
            }
        }
    }

    async fn claim_work(&self, claim: WorkClaim) -> Result<AgentWorkBatch, FactStoreError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut ids = state
            .work
            .values()
            .filter(|value| {
                let managed = state
                    .sessions
                    .get(&value.session_id)
                    .is_some_and(|session| session.kind == super::SessionKind::Managed);
                let claimable = value.state == AgentWorkState::Pending
                    || (value.state == AgentWorkState::Claimed
                        && state
                            .claims
                            .get(&value.work_id)
                            .is_some_and(|(_, lease)| lease.as_str() <= claim.claimed_at.as_str()));
                managed
                    && claimable
                    && claim
                        .scope
                        .session_id
                        .as_ref()
                        .is_none_or(|session_id| session_id == &value.session_id)
            })
            .map(|value| (value.created_at.clone(), value.work_id.clone()))
            .collect::<Vec<_>>();
        ids.sort();
        ids.truncate(claim.scope.limit as usize);
        let mut items = Vec::with_capacity(ids.len());
        for (_, id) in ids {
            let work = state.work.get_mut(&id).ok_or(FactStoreError::CorruptData)?;
            work.state = AgentWorkState::Claimed;
            work.attempt_count += 1;
            let snapshot = work.clone();
            if let AgentWorkPayload::FeedbackResume { delivery_id, .. } = &snapshot.payload {
                let delivery = state
                    .deliveries
                    .values_mut()
                    .find(|delivery| delivery.delivery_id == *delivery_id)
                    .ok_or(FactStoreError::CorruptData)?;
                delivery.attempt_count += 1;
            }
            state
                .claims
                .insert(id, (claim.claim_token.clone(), claim.lease_until.clone()));
            items.push(ClaimedAgentWork {
                work: snapshot,
                claim_token: claim.claim_token.clone(),
                lease_until: claim.lease_until.clone(),
            });
        }
        Ok(AgentWorkBatch { items })
    }

    async fn record_work(
        &self,
        result: StoredWorkResult,
    ) -> Result<AgentWorkRecordOutcome, FactStoreError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let work = state
            .work
            .get(&result.result.work_id)
            .cloned()
            .ok_or(FactStoreError::WorkNotFound)?;
        let Some((claim_token, lease_until)) = state.claims.get(&result.result.work_id) else {
            return Err(FactStoreError::WorkClaimConflict);
        };
        if claim_token != &result.result.claim_token || result.recorded_at >= *lease_until {
            return Err(FactStoreError::WorkClaimConflict);
        }
        if let AgentWorkDisposition::Retry { error_code } = result.result.disposition {
            if work.state != AgentWorkState::Claimed {
                return Err(FactStoreError::WorkClaimConflict);
            }
            let feedback_delivery = match &work.payload {
                AgentWorkPayload::FeedbackResume { delivery_id, .. } => Some(delivery_id.clone()),
                _ => None,
            };
            {
                let work = state
                    .work
                    .get_mut(&result.result.work_id)
                    .ok_or(FactStoreError::WorkNotFound)?;
                work.state = AgentWorkState::Pending;
                work.last_error_code = Some(error_code.clone());
                work.last_error_at = Some(result.recorded_at.clone());
                work.completed_at = None;
            }
            if let Some(delivery_id) = feedback_delivery {
                let delivery = state
                    .deliveries
                    .values_mut()
                    .find(|delivery| delivery.delivery_id == delivery_id)
                    .ok_or(FactStoreError::CorruptData)?;
                delivery.last_error_code = Some(error_code);
                delivery.last_error_at = Some(result.recorded_at.clone());
            }
            state.claims.remove(&result.result.work_id);
            return Ok(AgentWorkRecordOutcome {
                work_id: result.result.work_id,
                state: AgentWorkState::Pending,
                delivered: None,
            });
        }
        let AgentWorkDisposition::Completed { evidence } = &result.result.disposition else {
            unreachable!()
        };
        let delivered = match (&work.payload, evidence) {
            (
                AgentWorkPayload::FeedbackResume { delivery_id, .. },
                AgentWorkEvidence::FeedbackConsumedAndTurnCompleted {
                    delivery_id: observed,
                },
            ) if delivery_id == observed => Some(delivery_id.clone()),
            (AgentWorkPayload::FeedbackResume { .. }, _) => {
                return Err(FactStoreError::WorkClaimConflict);
            }
            (_, AgentWorkEvidence::PromptTurnCompleted) => None,
            _ => return Err(FactStoreError::WorkClaimConflict),
        };
        if work.state == AgentWorkState::Completed {
            return Ok(AgentWorkRecordOutcome {
                work_id: work.work_id,
                state: work.state,
                delivered,
            });
        }
        if work.state != AgentWorkState::Claimed {
            return Err(FactStoreError::WorkClaimConflict);
        }
        if let Some(delivery_id) = &delivered {
            let delivery = state
                .deliveries
                .values_mut()
                .find(|value| value.delivery_id == *delivery_id)
                .ok_or(FactStoreError::CorruptData)?;
            if delivery.state != DeliveryState::Pending {
                return Err(FactStoreError::WorkClaimConflict);
            }
            delivery.state = DeliveryState::Delivered;
            delivery.last_error_code = None;
            delivery.last_error_at = None;
            delivery.delivered_at = Some(result.recorded_at.clone());
        }
        let work = state
            .work
            .get_mut(&result.result.work_id)
            .ok_or(FactStoreError::WorkNotFound)?;
        work.state = AgentWorkState::Completed;
        work.last_error_code = None;
        work.last_error_at = None;
        work.completed_at = Some(result.recorded_at);
        Ok(AgentWorkRecordOutcome {
            work_id: work.work_id.clone(),
            state: work.state,
            delivered,
        })
    }
}
