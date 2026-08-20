import type { FeedbackStatus } from './generated/feedback'
import type { Locale } from './preferences'

export type {
  ActionInput,
  AddAttachmentInput,
  ApproveFeedbackInput,
  AttachmentView,
  CancelFeedbackInput,
  ContextRef,
  DeleteFeedbackRequestInput,
  DraftView,
  ExecutionMode,
  FeedbackRequestSummary,
  FeedbackResolution,
  FeedbackRequestView,
  FeedbackResultView,
  FeedbackStatus,
  FeedbackWorkspaceView,
  HostSessionInput,
  HostSessionSummary,
  ListFeedbackRequestsInput,
  ListFeedbackRequestsOutput,
  ListHostSessionsInput,
  RemoveAttachmentInput,
  RenameHostSessionInput,
  ReorderAttachmentsInput,
  RequestAttachmentView,
  SaveDraftInput,
  SetHostPinnedInput,
  SetHostSessionPinnedInput,
  SubmitFeedbackInput,
} from './generated/feedback'

export function requestStatusLabel(status: FeedbackStatus, locale: Locale = 'zh-CN'): string {
  if (locale === 'en') {
    switch (status) {
      case 'waiting':
        return 'Waiting'
      case 'in_progress':
        return 'In progress'
      case 'completed':
        return 'Submitted'
      case 'cancelled':
        return 'Cancelled'
    }
  }
  switch (status) {
    case 'waiting':
      return '等待开始'
    case 'in_progress':
      return '反馈中'
    case 'completed':
      return '已提交'
    case 'cancelled':
      return '已取消'
  }
}
