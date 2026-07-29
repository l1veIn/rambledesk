import type { FeedbackStatus } from './generated/feedback'

export type {
  ActionInput,
  ContextRef,
  DraftView,
  ExecutionMode,
  FeedbackRequestSummary,
  FeedbackRequestView,
  FeedbackResultView,
  FeedbackStatus,
  FeedbackWorkspaceView,
  SaveDraftInput,
  SubmitFeedbackInput,
} from './generated/feedback'

export function requestStatusLabel(status: FeedbackStatus): string {
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
