import type { FeedbackResolution, FeedbackResultView, FeedbackWorkspaceView } from '../feedback'
import type { HostProfile, ResumePrompt } from './types'
import type { HostSessionSummary, SessionManagement } from '../generated/feedback'

export function shouldShowResumePromptButton(
  feedbackResult: FeedbackResultView | null,
  resolution: FeedbackResolution | null | undefined,
  management?: SessionManagement,
): boolean {
  return management?.kind !== 'managed' && feedbackResult !== null && resolution === 'feedback_submitted'
}

export function requestSessionManagement(
  request: Readonly<{ host_id: string; host_session_id: string }> | null | undefined,
  sessions: readonly HostSessionSummary[],
): SessionManagement | undefined {
  return request ? sessions.find((session) => session.host_id === request.host_id
    && session.host_session_id === request.host_session_id)?.management : undefined
}

export function buildResumePrompt(
  workspace: FeedbackWorkspaceView,
  hostProfile: HostProfile,
  tr: (source: string, values?: Record<string, string | number>) => string,
): ResumePrompt {
  const requestId = workspace.request.request_id
  const hostLabel = hostProfile.label
  return {
    request_id: requestId,
    host_id: hostProfile.id || workspace.request.host_id || 'unknown',
    host_label: hostLabel,
    title: tr('Feedback submitted · return to host'),
    body: tr(
      'Return to {host} and click the waiting Continue or confirmation option first. Only paste the fallback resume prompt below if the host is not waiting.',
      { host: hostLabel },
    ),
    resume_prompt: `RambleDesk feedback request ${requestId} is completed.\nCall get_feedback with this request_id, verify the package, and continue the original task.`,
    reason: 'completed',
  }
}
