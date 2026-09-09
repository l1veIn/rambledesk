// Cooking transforms a saved submission into a separate Markdown variant.
// Publication belongs to the publisher; this module never mutates the draft.

import type { FeedbackWorkspaceView } from '../feedback'
import type { CookingConfig } from '../cooking'
import type { FeedbackPreparation } from '../speech/rambleSessionControllerHandle'
import type { CookingPreview } from './cookingSession'

export type CookingSubmission = {
  request: FeedbackWorkspaceView['request']
  actions: FeedbackWorkspaceView['actions']
  body: string
  savedRevision: number
}

type CookingControllerContext = {
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  getWorkspace: () => FeedbackWorkspaceView | null
  getDraftBody: () => string
  getCookingConfig: () => CookingConfig
  isCookingEnabled: () => boolean
  isCooking: () => boolean
  prepareFeedback: (requestId: string) => Promise<FeedbackPreparation>
  saveDraftNow: () => Promise<boolean>
  setPageError: (message: string) => void
  setCooking: (requestId: string, cooking: boolean) => void
  setPreview: (preview: CookingPreview | null) => void
}

export type CookingController = ReturnType<typeof createCookingController>

export function createCookingController(context: CookingControllerContext) {
  let previewFlight: Promise<void> | null = null
  async function cookBody(input: {
    title: string
    whatHappened: string
    actions: FeedbackWorkspaceView['actions']
    uncookedMarkdown: string
  }): Promise<{ markdown: string; model: string }> {
    const { cookFeedback } = await import('../cooking')
    return cookFeedback(input, context.getCookingConfig())
  }

  /** The same input/save boundary as publishing, with a separate read-only variant. */
  function cookPreviewOnly(): Promise<void> {
    if (previewFlight) return previewFlight
    previewFlight = Promise.resolve().then(preparePreview).finally(() => { previewFlight = null })
    return previewFlight
  }

  async function preparePreview() {
    const workspace = context.getWorkspace()
    if (!workspace || !context.isCookingEnabled() || context.isCooking() ||
      workspace.request.status === 'completed' || workspace.request.status === 'cancelled') return
    const requestId = workspace.request.request_id
    const stillEditable = () => {
      const current = context.getWorkspace()?.request
      return current?.request_id === requestId && current.status !== 'completed' && current.status !== 'cancelled'
    }
    let ownsCooking = false
    try {
      const preparation = await context.prepareFeedback(requestId)
      if (!stillEditable() || !context.isCookingEnabled() || context.isCooking()) return
      if (preparation.kind !== 'ready') {
        context.setPageError(preparation.kind === 'failed' ? preparation.message
          : context.tr('Review the pending speech in the capsule before submitting feedback.'))
        return
      }
      context.setCooking(requestId, true)
      ownsCooking = true
      context.setPageError('')
      if (!(await context.saveDraftNow()) || !stillEditable()) return
      const saved = context.getWorkspace()!
      const original = context.getDraftBody()
      if (!original.trim()) return
      const cooked = await cookSubmission({
        request: saved.request, actions: saved.actions,
        body: original, savedRevision: saved.draft.saved_revision,
      })
      if (!stillEditable() || !context.isCookingEnabled()) return
      if (context.getWorkspace()!.draft.saved_revision !== cooked.savedRevision ||
        context.getDraftBody() !== cooked.original) {
        context.setPageError(context.tr('The draft changed after Cooking. Restore the original and Cook again.'))
        return
      }
      context.setPreview(cooked)
    } catch (cause) {
      if (context.getWorkspace()?.request.request_id === requestId) context.setPageError(context.messageFrom(cause))
    } finally {
      if (ownsCooking) context.setCooking(requestId, false)
    }
  }

  /** Discard the cooked preview and restore the pre-cook draft. */
  function restoreOriginal() {
    if (context.getWorkspace() === null) return
    context.setPreview(null)
  }

  /** Return a variant tied to the exact saved source; errors stay with the caller. */
  async function cookSubmission(submission: CookingSubmission): Promise<CookingPreview> {
    const cooked = await cookBody({
      title: submission.request.title,
      whatHappened: submission.request.what_happened,
      actions: submission.actions,
      uncookedMarkdown: submission.body,
    })
    return {
      ...cooked,
      requestId: submission.request.request_id,
      savedRevision: submission.savedRevision,
      original: submission.body,
    }
  }

  return { cookPreviewOnly, restoreOriginal, cookSubmission }
}
