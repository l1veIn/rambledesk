import {
  APPLICATION_ERROR_CODES,
  type AddAttachmentInput,
  type ApplicationError,
  type ApplicationErrorCode,
  type ApproveFeedbackInput,
  type CancelFeedbackInput,
  type DeleteFeedbackRequestInput,
  type DraftView,
  type FeedbackPackageView,
  type FeedbackRequestSummary,
  type FeedbackRequestView,
  type FeedbackWorkspaceView,
  type GetFeedbackInput,
  type HostSessionInput,
  type HostSessionSummary,
  type ListFeedbackRequestsInput,
  type ListFeedbackRequestsOutput,
  type ListHostSessionsInput,
  type RemoveAttachmentInput,
  type ReadAttachmentInput,
  type RenameHostSessionInput,
  type ReorderAttachmentsInput,
  type SaveDraftInput,
  type SetHostPinnedInput,
  type SetHostSessionPinnedInput,
  type SubmitFeedbackInput,
} from '../generated/feedback'
import type { HostProfile } from '../generated/hosts'

type ApplicationCommandContract<Input, Output> = Readonly<{
  input: Input
  output: Output
}>

/**
 * Cross-client application operations. Inputs are domain/transport contracts,
 * never a Tauri `{ input }` envelope or camelCase invoke argument object.
 */
export type ApplicationCommandMap = Readonly<{
  listFeedbackInbox: ApplicationCommandContract<undefined, FeedbackRequestSummary[]>
  listHostSessions: ApplicationCommandContract<undefined, HostSessionSummary[]>
  listArchivedHostSessions: ApplicationCommandContract<ListHostSessionsInput, HostSessionSummary[]>
  listHostProfiles: ApplicationCommandContract<undefined, HostProfile[]>
  listFeedbackRequests: ApplicationCommandContract<ListFeedbackRequestsInput, ListFeedbackRequestsOutput>
  getFeedbackWorkspace: ApplicationCommandContract<GetFeedbackInput, FeedbackWorkspaceView>
  readPublishedFeedback: ApplicationCommandContract<GetFeedbackInput, FeedbackPackageView | null>
  saveFeedbackDraft: ApplicationCommandContract<SaveDraftInput, DraftView>
  addFeedbackAttachment: ApplicationCommandContract<AddAttachmentInput, FeedbackWorkspaceView>
  removeFeedbackAttachment: ApplicationCommandContract<RemoveAttachmentInput, FeedbackWorkspaceView>
  reorderFeedbackAttachments: ApplicationCommandContract<ReorderAttachmentsInput, FeedbackWorkspaceView>
  submitFeedback: ApplicationCommandContract<SubmitFeedbackInput, FeedbackRequestView>
  approveFeedbackRequest: ApplicationCommandContract<ApproveFeedbackInput, FeedbackRequestView>
  cancelFeedbackRequest: ApplicationCommandContract<CancelFeedbackInput, FeedbackRequestView>
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
