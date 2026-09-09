import type { FeedbackResolution, FeedbackResultView, FeedbackWorkspaceView } from '../feedback'
import type { HostProfile } from '../domain/hostProfile'
import type { ResumePrompt } from '../domain/resumePrompt'

type Translate = (source: string, values?: Record<string, string | number>) => string

/** Only the product's explicitly marked guidance belongs to the Client locale. */
export function resumePromptPresentation(prompt: ResumePrompt, tr: Translate): Pick<ResumePrompt, 'title' | 'body'> {
  if (!prompt.default_presentation) return { title: prompt.title, body: prompt.body }
  return prompt.reason === 'cancelled' ? {
    title: tr('Feedback cancelled · return to host'),
    body: tr('Return to {host} and click the waiting confirmation to finish. Only paste the fallback prompt below and call get_feedback if the host is not waiting.', { host: prompt.host_label }),
  } : {
    title: tr('Feedback submitted · return to host'),
    body: tr('Return to {host} and click the waiting Continue or confirmation option first. Only paste the fallback resume prompt below if the host is not waiting.', { host: prompt.host_label }),
  }
}

export function shouldShowResumePromptButton(
  feedbackResult: FeedbackResultView | null,
  resolution: FeedbackResolution | null | undefined,
  managedSessionId?: string | null,
): boolean {
  return !managedSessionId && feedbackResult !== null && resolution === 'feedback_submitted'
}

export function buildResumePrompt(
  workspace: FeedbackWorkspaceView,
  hostProfile: HostProfile,
  tr: Translate,
): ResumePrompt {
  const requestId = workspace.request.request_id
  const hostLabel = hostProfile.label
  const prompt: ResumePrompt = {
    request_id: requestId,
    host_id: hostProfile.id || workspace.request.host_id || 'unknown',
    host_label: hostLabel,
    title: '',
    body: '',
    resume_prompt: `RambleDesk feedback request ${requestId} is completed.\nCall get_feedback with this request_id, verify the package, and continue the original task.`,
    reason: 'completed',
    default_presentation: true,
  }
  return { ...prompt, ...resumePromptPresentation(prompt, tr) }
}
