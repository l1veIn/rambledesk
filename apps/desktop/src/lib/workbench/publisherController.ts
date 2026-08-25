// Feedback submission and publication orchestration. Keeps the submit flow
// (preview reuse, auto-cook, direct publish) out of the App.svelte shell;
// reactive state stays in the component via the context callbacks.

import { invoke } from '@tauri-apps/api/core'

import type {
  FeedbackRequestView,
  FeedbackWorkspaceView,
  SubmitFeedbackInput,
} from '../feedback'
import { normalizePublishedFeedback, type PublishedFeedbackPackage } from '../publishedFeedback'
import type { CookingSubmission, CookedPreview } from './cookingController'
import type { SubmitStage } from './types'

type PublisherControllerContext = {
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
  awaitCaptureWork: () => Promise<void>
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
            await invoke<PublishedFeedbackPackage | null>('read_published_feedback', {
              requestId,
            }),
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
    const result = await invoke<FeedbackRequestView>('submit_feedback', { input })
    const visible = applyVisibleSubmissionResult(result)
    context.showSubmittedToast(cookedMarkdown !== undefined)
    if (visible) {
      await loadVisiblePublishedFeedback(result.request_id, cookedMarkdown, uncookedMarkdown)
    }
    await context.refreshNavigation(true)
  }

  async function submitFeedback() {
    const workspace = context.getWorkspace()
    if (!workspace || !context.getCanSubmit()) return
    if (context.getRambleCanExit()) await context.exitRamble()
    // Publishing is terminal for the draft, so speech still being transcribed
    // has to land first — exitRamble only covers it while Ramble is engaged.
    await context.awaitCaptureWork()
    const requestId = workspace.request.request_id
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

    // A generated cooked draft is published directly: no second model call, no
    // extra wait. If the user edited the cooked text, submit the current editor
    // body as the cooked variant while preserving the original uncooked source.
    if (context.getCookingEnabled() && context.getPreview()) {
      const preview = context.getPreview()!
      context.setSubmitting(true)
      context.setSubmitStage('publishing')
      try {
        await publishFeedback(
          {
            request_id: submission.request.request_id,
            expected_revision: submission.savedRevision,
            cooked_markdown: submission.body,
            cooking_model: preview.model,
            uncooked_markdown: preview.original,
          },
          submission.body,
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
