// Feedback submission and publication orchestration. Keeps the submit flow
// (preview reuse, auto-cook, direct publish) out of the App.svelte shell;
// reactive state stays in the component via the context callbacks.

import type { ApplicationTransport } from '../application/applicationTransport'
import type {
  FeedbackRequestView,
  FeedbackWorkspaceView,
  SubmitFeedbackInput,
} from '../feedback'
import { normalizePublishedFeedback } from '../publishedFeedback'
import type { CookingSubmission, CookedPreview } from './cookingController'
import type { SubmitStage } from './types'

type PublisherControllerContext = {
  transport: ApplicationTransport
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  isPreviewMode: () => boolean
  getWorkspace: () => FeedbackWorkspaceView | null
  setWorkspace: (workspace: FeedbackWorkspaceView) => void
  setCompletedResult: (result: FeedbackRequestView | null) => void
  setPublishedFeedback: (
    feedback: { markdown: string; uncooked_markdown?: string } | null,
  ) => void
  setSavePhase: (phase: 'saved') => void
  setPageError: (message: string) => void
  getCanSubmit: () => boolean
  getRambleCanExit: () => boolean
  exitRamble: () => Promise<void>
  hasPendingSpeech?: (requestId: string) => boolean
  getSpeechStopError?: () => string
  saveDraftNow: () => Promise<boolean>
  getDraftBody: () => string
  getSavedRevision: () => number
  getCookingEnabled: () => boolean
  getPreview: () => CookedPreview | null
  setPreview: (preview: CookedPreview | null) => void
  setCooking: (requestId: string, cooking: boolean) => void
  cookAndPublish: (submission: CookingSubmission) => Promise<void>
  setSubmitting: (submitting: boolean) => void
  setSubmitStage: (stage: SubmitStage) => void
  refreshNavigation: (force: boolean) => Promise<void>
  showSubmittedToast: (cooked: boolean) => void
}

export type PublisherController = ReturnType<typeof createPublisherController>

export function createPublisherController(context: PublisherControllerContext) {
  function applyVisibleSubmissionResult(result: FeedbackRequestView): boolean {
    const workspace = context.getWorkspace()
    if (workspace?.request.request_id !== result.request_id) return false
    context.setCompletedResult(result)
    context.setWorkspace({
      ...workspace,
      feedback: result.feedback,
      request: {
        ...workspace.request,
        status: result.status,
        resolution: result.resolution,
        allow_finish: result.allow_finish,
        final_summary: result.final_summary,
        updated_at: result.updated_at,
      },
    })
    context.setSavePhase('saved')
    return true
  }

  async function loadVisiblePublishedFeedback(
    requestId: string,
    cookedMarkdown: string | undefined,
    uncookedMarkdown: string,
  ) {
    if (context.getWorkspace()?.request.request_id !== requestId) return
    try {
      const next = context.isPreviewMode()
        ? {
            markdown: cookedMarkdown ?? uncookedMarkdown,
            uncooked_markdown: uncookedMarkdown,
          }
        : normalizePublishedFeedback(
            await context.transport.call('readPublishedFeedback', { request_id: requestId }),
          )
      if (context.getWorkspace()?.request.request_id === requestId) {
        context.setPublishedFeedback(next)
      }
    } catch (cause) {
      context.setPageError(context.messageFrom(cause))
    }
  }

  async function publishFeedback(
    input: SubmitFeedbackInput,
    cookedMarkdown: string | undefined,
    uncookedMarkdown: string,
  ) {
    const result = await context.transport.call('submitFeedback', input)
    const visible = applyVisibleSubmissionResult(result)
    context.showSubmittedToast(cookedMarkdown !== undefined)
    if (visible) {
      await loadVisiblePublishedFeedback(result.request_id, cookedMarkdown, uncookedMarkdown)
    }
    await context.refreshNavigation(true)
  }

  async function submitFeedback() {
    const workspace = context.getWorkspace()
    if (!workspace) return
    if (
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) {
      return
    }
    // Never publish an empty reply — whitespace-only bodies count as empty.
    // Guards the submit path even when the UI gate failed to disable the
    // button (empty drafts should not reach the host from any view).
    if (context.getDraftBody().trim().length === 0) {
      context.setPageError(context.tr('Cannot send an empty reply. Write some feedback content first.'))
      return
    }
    if (!context.getCanSubmit()) return
    if (context.getRambleCanExit()) await context.exitRamble()
    const speechError = context.getSpeechStopError?.()
    if (speechError) {
      context.setPageError(speechError)
      return
    }
    const requestId = workspace.request.request_id
    if (context.hasPendingSpeech?.(requestId)) {
      context.setPageError(context.tr('Review the pending speech in the capsule before submitting feedback.'))
      return
    }
    if (!(await context.saveDraftNow())) return
    if (
      context.getWorkspace()?.request.request_id !== requestId ||
      !context.getCanSubmit()
    ) return

    const submission: CookingSubmission = {
      request: context.getWorkspace()!.request,
      actions: context.getWorkspace()!.actions,
      body: context.getDraftBody(),
      savedRevision: context.getSavedRevision(),
    }
    context.setPageError('')

    // A generated cooked preview is published directly without ever replacing
    // the canonical draft or making a second model call.
    if (context.getCookingEnabled() && context.getPreview()) {
      const preview = context.getPreview()!
      context.setSubmitting(true)
      context.setSubmitStage('publishing')
      try {
        await publishFeedback(
          {
            request_id: submission.request.request_id,
            expected_revision: submission.savedRevision,
            cooked_markdown: preview.markdown,
            cooking_model: preview.model,
            uncooked_markdown: preview.original,
          },
          preview.markdown,
          preview.original,
        )
        context.setPreview(null)
      } catch (cause) {
        context.setPageError(context.messageFrom(cause))
      } finally {
        context.setSubmitting(false)
        context.setSubmitStage('idle')
      }
      return
    }

    if (context.getCookingEnabled()) {
      context.setCooking(requestId, true)
      void context.cookAndPublish(submission)
      return
    }

    context.setSubmitting(true)
    context.setSubmitStage('publishing')
    try {
      await publishFeedback(
        {
          request_id: submission.request.request_id,
          expected_revision: submission.savedRevision,
        },
        undefined,
        submission.body,
      )
    } catch (cause) {
      context.setPageError(context.messageFrom(cause))
    } finally {
      context.setSubmitting(false)
      context.setSubmitStage('idle')
    }
  }

  return { submitFeedback, publishFeedback, applyVisibleSubmissionResult }
}
