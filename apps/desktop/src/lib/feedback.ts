import type { FeedbackStatus } from './generated/feedback'
import type { Locale } from './preferences'

export type {
  ActionInput,
  AddAttachmentInput,
  AttachmentView,
  CancelFeedbackInput,
  ContextRef,
  DraftView,
  ExecutionMode,
  FeedbackRequestSummary,
  FeedbackRequestView,
  FeedbackResultView,
  FeedbackStatus,
  FeedbackWorkspaceView,
  HostSessionSummary,
  ListFeedbackRequestsInput,
  ListFeedbackRequestsOutput,
  RemoveAttachmentInput,
  ReorderAttachmentsInput,
  SaveDraftInput,
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
