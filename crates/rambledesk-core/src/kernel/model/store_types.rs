use super::{
    AcpSessionLinkSnapshot, AgentObservation, AgentWorkRecord, AgentWorkResult, DraftArtifact,
    DraftId, DraftSnapshot, FeedbackDeliveryRecord, FeedbackRequestSnapshot, FeedbackResolution,
    FeedbackResolutionOutcome, LaunchOutcome, PackageRecord, RambleSubmissionRecord,
    RemoveDraftArtifact, ReorderDraftArtifacts, RequestId, SaveDraft, SessionId,
    SessionOrganization, SessionRecord, SteeringOutcome, WorkClaimToken, WorkScope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchQuery {
    pub session_id: Option<SessionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommit {
    pub session: SessionRecord,
    pub submission: RambleSubmissionRecord,
    pub package: PackageRecord,
    pub work: AgentWorkRecord,
    pub outcome: LaunchOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteeringCommit {
    pub submission: RambleSubmissionRecord,
    pub work: AgentWorkRecord,
    pub outcome: SteeringOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackRequestCommit {
    pub request: FeedbackRequestSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackResolutionCommit {
    pub request_id: RequestId,
    pub expected_draft_revision: Option<u64>,
    pub submission: Option<RambleSubmissionRecord>,
    pub package: Option<PackageRecord>,
    pub resolution: FeedbackResolution,
    pub cancel_reason: Option<String>,
    pub delivery: FeedbackDeliveryRecord,
    pub work: AgentWorkRecord,
    pub outcome: FeedbackResolutionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftCommit {
    pub mutation: StoredDraftMutation,
    pub now: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredDraftMutation {
    Save(SaveDraft),
    AddArtifact {
        draft_id: DraftId,
        expected_revision: u64,
        artifact: DraftArtifact,
    },
    RemoveArtifact(RemoveDraftArtifact),
    ReorderArtifacts(ReorderDraftArtifacts),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentObservationCommit {
    pub observation: AgentObservation,
    pub link: AcpSessionLinkSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOrganizationCommit {
    pub mutation: SessionOrganization,
    pub now: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactMutation {
    Launch(Box<LaunchCommit>),
    Steering(Box<SteeringCommit>),
    FeedbackRequest(Box<FeedbackRequestCommit>),
    FeedbackResolution(Box<FeedbackResolutionCommit>),
    Draft(Box<DraftCommit>),
    AgentObservation(Box<AgentObservationCommit>),
    SessionOrganization(Box<SessionOrganizationCommit>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactMutationOutcome {
    Launch(LaunchOutcome),
    Steering(SteeringOutcome),
    FeedbackRequest(FeedbackRequestSnapshot),
    FeedbackResolution(FeedbackResolutionOutcome),
    Draft(DraftSnapshot),
    AgentObservation(AcpSessionLinkSnapshot),
    SessionOrganization(SessionRecord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactQuery {
    Feedback(RequestId),
    Workbench(WorkbenchQuery),
    ArchivedSessions,
    SessionRecovery(SessionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedbackLookup {
    Waiting {
        request: FeedbackRequestSnapshot,
        session: SessionRecord,
    },
    Terminal {
        request: FeedbackRequestSnapshot,
        session: SessionRecord,
        delivery: Box<FeedbackDeliveryRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchSnapshot {
    pub sessions: Vec<SessionRecord>,
    /// Durable resume checkpoints, at most one current link per Session.
    pub current_acp_links: Vec<AcpSessionLinkSnapshot>,
    /// RambleDesk-owned structured request history. This is not an ACP
    /// transcript projection and remains stable after a request resolves.
    pub feedback_requests: Vec<FeedbackRequestSnapshot>,
    pub waiting_feedback: Vec<FeedbackRequestSnapshot>,
    pub drafts: Vec<DraftSnapshot>,
    pub pending_deliveries: Vec<FeedbackDeliveryRecord>,
    pub pending_agent_work: Vec<AgentWorkRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecoverySnapshot {
    pub session: SessionRecord,
    pub current_acp_link: Option<AcpSessionLinkSnapshot>,
    /// Present for Managed Sessions launched by RambleDesk; absent for Imported Sessions.
    pub launch_submission: Option<RambleSubmissionRecord>,
    pub steering_submissions: Vec<RambleSubmissionRecord>,
    pub pending_feedback: Vec<PendingFeedbackRecovery>,
    pub pending_agent_work: Vec<AgentWorkRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFeedbackRecovery {
    pub request: FeedbackRequestSnapshot,
    pub delivery: FeedbackDeliveryRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactQueryOutcome {
    Feedback(FeedbackLookup),
    Workbench(WorkbenchSnapshot),
    ArchivedSessions(Vec<SessionRecord>),
    SessionRecovery(SessionRecoverySnapshot),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkClaim {
    pub scope: WorkScope,
    pub claim_token: WorkClaimToken,
    pub claimed_at: String,
    pub lease_until: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredWorkResult {
    pub result: AgentWorkResult,
    pub recorded_at: String,
}
