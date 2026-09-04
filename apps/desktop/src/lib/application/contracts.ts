import {
  APPLICATION_ERROR_CODES,
  type AgentConfig,
  type AgentConfigInput,
  type AgentConnectionCheck,
  type ApplicationFeedbackRequestView,
  type ApplicationFeedbackWorkspaceView,
  type ApplicationHostProfileView,
  type AddAttachmentInput,
  type ApplicationError,
  type ApplicationErrorCode,
  type ApproveFeedbackInput,
  type CancelFeedbackInput,
  type CreateManagedSessionInput,
  type DeleteFeedbackRequestInput,
  type DraftView,
  type FeedbackPackageView,
  type FeedbackRequestSummary,
  type GetFeedbackInput,
  type HostSessionInput,
  type HostSessionSummary,
  type ListFeedbackRequestsInput,
  type ListFeedbackRequestsOutput,
  type ListHostSessionsInput,
  type ManagedSessionInput,
  type ManagedSessionSnapshot,
  type RemoveAttachmentInput,
  type ReadAttachmentInput,
  type RenameHostSessionInput,
  type ReorderAttachmentsInput,
  type SaveDraftInput,
  type SaveAgentConfigInput,
  type SendManagedPromptInput,
  type RespondManagedPermissionInput,
  type ResolveFeedbackDeliveryInput,
  type SetHostPinnedInput,
  type SetHostSessionPinnedInput,
  type SubmitFeedbackInput,
} from '../generated/feedback'

type ApplicationCommandContract<Input, Output> = Readonly<{
  input: Input
  output: Output
}>

/**
 * The cross-client attachment contract keeps bytes binary. Rust's generated
 * DTO remains the Tauri wire shape, where serde receives a number array.
 */
export type ApplicationAddAttachmentInput = Omit<AddAttachmentInput, 'contents'> &
  Readonly<{ contents: ArrayBuffer }>

/**
 * Cross-client application operations. Inputs are domain/transport contracts,
 * never a Tauri `{ input }` envelope or camelCase invoke argument object.
 */
export type ApplicationCommandMap = Readonly<{
  listAgentConfigs: ApplicationCommandContract<undefined, AgentConfig[]>
  saveAgentConfig: ApplicationCommandContract<SaveAgentConfigInput, AgentConfig>
  deleteAgentConfig: ApplicationCommandContract<AgentConfigInput, void>
  checkAgentConfig: ApplicationCommandContract<AgentConfigInput, AgentConnectionCheck>
  createManagedSession: ApplicationCommandContract<CreateManagedSessionInput, ManagedSessionSnapshot>
  getManagedSession: ApplicationCommandContract<ManagedSessionInput, ManagedSessionSnapshot>
  startManagedSession: ApplicationCommandContract<ManagedSessionInput, ManagedSessionSnapshot>
  stopManagedSession: ApplicationCommandContract<ManagedSessionInput, ManagedSessionSnapshot>
  cancelManagedPrompt: ApplicationCommandContract<ManagedSessionInput, ManagedSessionSnapshot>
  sendManagedPrompt: ApplicationCommandContract<SendManagedPromptInput, ManagedSessionSnapshot>
  respondManagedPermission: ApplicationCommandContract<RespondManagedPermissionInput, ManagedSessionSnapshot>
  resolveFeedbackDelivery: ApplicationCommandContract<ResolveFeedbackDeliveryInput, ManagedSessionSnapshot>
  listFeedbackInbox: ApplicationCommandContract<undefined, FeedbackRequestSummary[]>
  listHostSessions: ApplicationCommandContract<undefined, HostSessionSummary[]>
  listArchivedHostSessions: ApplicationCommandContract<ListHostSessionsInput, HostSessionSummary[]>
  listHostProfiles: ApplicationCommandContract<undefined, ApplicationHostProfileView[]>
  listFeedbackRequests: ApplicationCommandContract<ListFeedbackRequestsInput, ListFeedbackRequestsOutput>
  getFeedbackWorkspace: ApplicationCommandContract<GetFeedbackInput, ApplicationFeedbackWorkspaceView>
  readPublishedFeedback: ApplicationCommandContract<GetFeedbackInput, FeedbackPackageView | null>
  saveFeedbackDraft: ApplicationCommandContract<SaveDraftInput, DraftView>
  addFeedbackAttachment: ApplicationCommandContract<ApplicationAddAttachmentInput, ApplicationFeedbackWorkspaceView>
  removeFeedbackAttachment: ApplicationCommandContract<RemoveAttachmentInput, ApplicationFeedbackWorkspaceView>
  reorderFeedbackAttachments: ApplicationCommandContract<ReorderAttachmentsInput, ApplicationFeedbackWorkspaceView>
  submitFeedback: ApplicationCommandContract<SubmitFeedbackInput, ApplicationFeedbackRequestView>
  approveFeedbackRequest: ApplicationCommandContract<ApproveFeedbackInput, ApplicationFeedbackRequestView>
  cancelFeedbackRequest: ApplicationCommandContract<CancelFeedbackInput, ApplicationFeedbackRequestView>
  renameHostSession: ApplicationCommandContract<RenameHostSessionInput, HostSessionSummary>
  setHostSessionPinned: ApplicationCommandContract<SetHostSessionPinnedInput, HostSessionSummary>
  archiveHostSession: ApplicationCommandContract<HostSessionInput, HostSessionSummary>
  unarchiveHostSession: ApplicationCommandContract<HostSessionInput, HostSessionSummary>
  deleteHostSession: ApplicationCommandContract<HostSessionInput, void>
  deleteFeedbackRequest: ApplicationCommandContract<DeleteFeedbackRequestInput, void>
  setHostPinned: ApplicationCommandContract<SetHostPinnedInput, HostSessionSummary[]>
  readFeedbackAttachment: ApplicationCommandContract<ReadAttachmentInput, ArrayBuffer>
  readRequestAttachment: ApplicationCommandContract<ReadAttachmentInput, ArrayBuffer>
}>

export type ApplicationCommandName = keyof ApplicationCommandMap
export type ApplicationCommandInput<Name extends ApplicationCommandName> =
  ApplicationCommandMap[Name]['input']
export type ApplicationCommandResult<Name extends ApplicationCommandName> =
  ApplicationCommandMap[Name]['output']

const applicationErrorCodes: ReadonlySet<string> = new Set(APPLICATION_ERROR_CODES)

export function isApplicationErrorCode(value: unknown): value is ApplicationErrorCode {
  return typeof value === 'string' && applicationErrorCodes.has(value)
}

export function isApplicationError(value: unknown): value is ApplicationError {
  if (value === null || typeof value !== 'object') return false
  const candidate = value as Record<string, unknown>
  return (
    isApplicationErrorCode(candidate.code) &&
    typeof candidate.message === 'string' &&
    typeof candidate.retryable === 'boolean'
  )
}
