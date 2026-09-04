use rambledesk_core::{AgentConnectionKind, AgentDistribution, AgentVerificationStatus, AgentVerification, AgentDependency, AgentCatalogEntry, AgentInstallSource, AgentCheckStatus, AgentCatalogCheck, AgentDependencyInspection, AgentInspection, InstallAgentInput, AgentInstallPhase, AgentInstallProgress, InstalledAgent};
use rambledesk_core::{SessionActivityContent, SessionContentBlock, SessionToolKind, SessionToolStatus, SessionToolLocation, SessionToolCall};
use std::{fs, path::PathBuf};

use rambledesk_core::{
    ActionInput, AddAttachmentInput, AgentConfig, AgentConfigInput, ApplicationError,
    ApplicationErrorCode, ApplicationEvent, ApplicationFeedbackRequestView,
    ApplicationFeedbackResultView, ApplicationFeedbackWorkspaceView, ApplicationHostProfileView,
    ApplicationResourceKey, ApplicationSnapshotMetadata, ApproveFeedbackInput, AttachmentView,
    CancelFeedbackInput, ContextRef, CreateManagedSessionInput, DeleteFeedbackRequestInput,
    DraftView, ExecutionMode, FeedbackPackageAttachment, FeedbackPackageContent,
    FeedbackPackageManifest, FeedbackPackageView, FeedbackRequestSummary, FeedbackRequestView,
    FeedbackResolution, FeedbackResultView, FeedbackStatus, FeedbackWorkspaceView,
    GetFeedbackInput, HostSessionInput, HostSessionSummary, ListFeedbackRequestsInput,
    ListFeedbackRequestsOutput, ListHostSessionsInput, ManagedSessionInput, ReadAttachmentInput,
    RecoverFeedbackInput, RemoveAttachmentInput, RenameHostSessionInput, ReorderAttachmentsInput,
    RequestAttachmentView, SaveAgentConfigInput, SaveDraftInput, SessionManagement,
    SessionProtocol, SessionRecord, SetHostPinnedInput, SetHostSessionPinnedInput,
    SubmitFeedbackInput,
};
use rambledesk_core::{
    AgentConnectionCheck, AgentSessionCapabilities, ManagedSessionSnapshot, SessionActivityState,
    SessionConnectionState, SessionRuntime,
};
use rambledesk_core::{
    FeedbackDelivery, FeedbackDeliveryState, ResolveDeliveryAction, ResolveFeedbackDeliveryInput,
};
use rambledesk_core::{
    RespondManagedPermissionInput, SendManagedPromptInput, SessionPermission,
    SessionPermissionOption,
};
use rambledesk_core::{SessionActivity, SessionActivityKind};
use rambledesk_core::{SessionRecovery, SessionRecoveryStatus};
use ts_rs::{Config, TS};

fn exported<T: TS>() -> String {
    T::decl(&Config::default()).replacen("type ", "export type ", 1)
}

fn exported_application_error_codes() -> String {
    let values = ApplicationErrorCode::ALL
        .iter()
        .map(|code| format!("\"{}\"", code.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "export const APPLICATION_ERROR_CODES = [{values}] as const satisfies readonly ApplicationErrorCode[];"
    )
}

fn exported_feedback_package_manifest() -> String {
    exported::<FeedbackPackageManifest>().replace(
        "request_attachments: Array<FeedbackPackageAttachment>",
        "request_attachments?: Array<FeedbackPackageAttachment>",
    )
}

fn exported_feedback_package_content() -> String {
    exported::<FeedbackPackageContent>().replace(
        "request_attachment_paths: Array<string>",
        "request_attachment_paths?: Array<string>",
    )
}

fn main() -> std::io::Result<()> {
    let declarations = [
        exported::<AgentConnectionKind>(),
        exported::<AgentDistribution>(),
        exported::<AgentVerificationStatus>(),
        exported::<AgentVerification>(),
        exported::<AgentDependency>(),
        exported::<AgentCatalogEntry>(),
        exported::<AgentInstallSource>(),
        exported::<AgentCheckStatus>(),
        exported::<AgentCatalogCheck>(),
        exported::<AgentDependencyInspection>(),
        exported::<AgentInspection>(),
        exported::<InstallAgentInput>(),
        exported::<AgentInstallPhase>(),
        exported::<AgentInstallProgress>(),
        exported::<InstalledAgent>(),

        exported::<SessionActivityContent>(),
        exported::<SessionContentBlock>(),
        exported::<SessionToolKind>(),
        exported::<SessionToolStatus>(),
        exported::<SessionToolLocation>(),
        exported::<SessionToolCall>(),

        exported::<SessionRecoveryStatus>(),
        exported::<SessionRecovery>(),
        exported::<FeedbackDelivery>(),
        exported::<FeedbackDeliveryState>(),
        exported::<ResolveDeliveryAction>(),
        exported::<ResolveFeedbackDeliveryInput>(),
        exported::<SessionPermissionOption>(),
        exported::<SessionPermission>(),
        exported::<RespondManagedPermissionInput>(),
        exported::<SendManagedPromptInput>(),
        exported::<SessionActivityKind>(),
        exported::<SessionActivity>(),
        exported::<AgentSessionCapabilities>(),
        exported::<SessionConnectionState>(),
        exported::<SessionActivityState>(),
        exported::<SessionRuntime>(),
        exported::<ManagedSessionSnapshot>(),
        exported::<AgentConnectionCheck>(),
        exported::<SessionProtocol>(),
        exported::<SessionManagement>(),
        exported::<AgentConfig>(),
        exported::<SaveAgentConfigInput>(),
        exported::<AgentConfigInput>(),
        exported::<SessionRecord>(),
        exported::<CreateManagedSessionInput>(),
        exported::<ManagedSessionInput>(),
        exported::<FeedbackStatus>(),
        exported::<FeedbackResolution>(),
        exported::<ExecutionMode>(),
        exported::<ApplicationErrorCode>(),
        exported_application_error_codes(),
        exported::<ApplicationError>(),
        exported::<ApplicationResourceKey>(),
        exported::<ApplicationSnapshotMetadata>(),
        exported::<ApplicationEvent>(),
        exported::<ApplicationHostProfileView>(),
        exported::<ActionInput>(),
        exported::<ContextRef>(),
        exported::<GetFeedbackInput>(),
        exported::<FeedbackResultView>(),
        exported::<ApplicationFeedbackResultView>(),
        exported::<FeedbackPackageAttachment>(),
        exported_feedback_package_manifest(),
        exported_feedback_package_content(),
        exported::<FeedbackPackageView>(),
        exported::<FeedbackRequestView>(),
        exported::<ApplicationFeedbackRequestView>(),
        exported::<FeedbackRequestSummary>(),
        exported::<HostSessionSummary>(),
        exported::<HostSessionInput>(),
        exported::<RenameHostSessionInput>(),
        exported::<SetHostSessionPinnedInput>(),
        exported::<SetHostPinnedInput>(),
        exported::<ListHostSessionsInput>(),
        exported::<ListFeedbackRequestsInput>(),
        exported::<ListFeedbackRequestsOutput>(),
        exported::<DeleteFeedbackRequestInput>(),
        exported::<DraftView>(),
        exported::<AttachmentView>(),
        exported::<RequestAttachmentView>(),
        exported::<FeedbackWorkspaceView>(),
        exported::<ApplicationFeedbackWorkspaceView>(),
        exported::<AddAttachmentInput>(),
        exported::<ReadAttachmentInput>(),
        exported::<RemoveAttachmentInput>(),
        exported::<ReorderAttachmentsInput>(),
        exported::<SaveDraftInput>(),
        exported::<SubmitFeedbackInput>(),
        exported::<CancelFeedbackInput>(),
        exported::<ApproveFeedbackInput>(),
        exported::<RecoverFeedbackInput>(),
    ];
    let output = format!(
        "/* This file is generated by `pnpm contracts:generate`. Do not edit. */\n\n{}\n",
        declarations.join("\n")
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src/lib/generated/feedback.ts");
    fs::write(path, output)
}
