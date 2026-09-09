import { get } from 'svelte/store'

import type { ApplicationTransport } from '../application/applicationTransport'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import type { FeedbackPreparation } from '../speech/rambleSessionControllerHandle'
import type { SubmitFeedbackInput } from '../feedback'
import { normalizePublishedFeedback } from '../publishedFeedback'
import type { CookingSubmission } from './cookingController'
import type { CookingPreview, CookingSession } from './cookingSession'
import type { DraftSession } from './draftSession'
import type { WorkspaceSession } from './workspaceSession'

type PublisherControllerContext = {
  transport: ApplicationTransport
  session: WorkspaceSession
  draft: DraftSession
  cooking: CookingSession
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  setPageError: (message: string) => void
  /** External policy, such as a managed session being deleted. */
  isReadOnly: () => boolean
  prepareFeedback: (requestId: string) => Promise<FeedbackPreparation>
  saveDraftNow: () => Promise<boolean>
  getCookingEnabled: () => boolean
  cookSubmission: (submission: CookingSubmission) => Promise<CookingPreview>
  refreshNavigation: (force: boolean) => Promise<void>
  showSubmittedToast: (cooked: boolean) => void
}

export type PublisherController = ReturnType<typeof createPublisherController>

/**
 * One submission owns the whole sequence: finish speech, drain saves, freeze the
 * accepted revision, optionally cook, then publish. Callers supply intent, not
 * setters for each intermediate state. The backend remains the publication authority.
 */
export function createPublisherController(context: PublisherControllerContext) {
  let activeSubmission: Promise<void> | null = null

  function stillEditable(requestId: string) {
    const state = get(context.session)
    return state.request?.request_id === requestId && !state.terminal && !context.isReadOnly()
  }

  function reportFor(requestId: string, cause: unknown) {
    if (context.session.requestId() === requestId) {
      context.setPageError(context.messageFrom(cause))
    }
  }

  async function publish(input: SubmitFeedbackInput) {
    const result = await context.transport.call('submitFeedback', input)
    const visible = context.session.applyMutationResult(result)
    if (visible) context.draft.markSaved()
    context.showSubmittedToast(input.cooked_markdown !== undefined)

    // Publication is already committed. A failed read must not undo that fact
    // or make the next click send the same feedback again.
    if (visible) {
      try {
        const published = await readApplicationSnapshot(context.transport, 'readPublishedFeedback', {
          request_id: result.request_id,
        })
        if (context.session.requestId() === result.request_id) {
          context.session.setPublished(normalizePublishedFeedback(published))
        }
      } catch (cause) {
        reportFor(result.request_id, cause)
      }
    }
    try {
      await context.refreshNavigation(true)
    } catch (cause) {
      reportFor(result.request_id, cause)
    }
  }

  async function runSubmission() {
    const initial = get(context.session)
    const requestId = initial.request?.request_id
    if (!requestId || initial.terminal || initial.interactionLocked || context.isReadOnly() ||
      context.cooking.isCooking(requestId)) return
    if (!get(context.draft).body.trim()) {
      context.setPageError(context.tr('Cannot send an empty reply. Write some feedback content first.'))
      return
    }

    let ownsSubmission = false
    let ownsCooking = false
    try {
      // Final speech still has to enter the editable draft. The single flight
      // prevents repeated submissions while this existing queue is draining.
      const preparation = await context.prepareFeedback(requestId)
      if (!stillEditable(requestId) || get(context.session).interactionLocked || context.cooking.isCooking(requestId)) return
      if (preparation.kind === 'failed') {
        context.setPageError(preparation.message)
        return
      }
      if (preparation.kind === 'pending-speech') {
        context.setPageError(context.tr('Review the pending speech in the capsule before submitting feedback.'))
        return
      }

      // Lock before saving, not after it: the confirmed document and revision
      // must stay together until the backend accepts or rejects this submission.
      context.session.setSubmissionStage('saving')
      ownsSubmission = true
      context.setPageError('')
      if (!(await context.saveDraftNow()) || !stillEditable(requestId)) return
      const draft = get(context.draft)
      const workspace = get(context.session).workspace!
      if (!draft.body.trim()) {
        context.setPageError(context.tr('Cannot send an empty reply. Write some feedback content first.'))
        return
      }
      const submission: CookingSubmission = {
        request: workspace.request,
        actions: workspace.actions,
        body: draft.body,
        savedRevision: draft.savedRevision,
      }

      let cooked: CookingPreview | null = null
      if (context.getCookingEnabled()) {
        cooked = context.cooking.preview()
        if (cooked && (cooked.requestId !== requestId ||
          cooked.savedRevision !== submission.savedRevision || cooked.original !== submission.body)) {
          context.setPageError(context.tr('The draft changed after Cooking. Restore the original and Cook again.'))
          return
        }
        if (!cooked) {
          context.session.setSubmissionStage('cooking')
          context.cooking.setCooking(requestId, true)
          ownsCooking = true
          cooked = await context.cookSubmission(submission)
        }
      }
      if (!stillEditable(requestId)) return
      context.session.setSubmissionStage('publishing')
      await publish({
        request_id: requestId,
        expected_revision: submission.savedRevision,
        ...(cooked ? {
          cooked_markdown: cooked.markdown,
          cooking_model: cooked.model,
          uncooked_markdown: cooked.original,
        } : {}),
      })
      if (context.session.requestId() === requestId) context.cooking.setPreview(null)
    } catch (cause) {
      reportFor(requestId, cause)
    } finally {
      if (ownsCooking) context.cooking.setCooking(requestId, false)
      if (ownsSubmission && context.session.requestId() === requestId) context.session.setSubmissionStage('idle')
    }
  }

  function submitFeedback(): Promise<void> {
    if (activeSubmission) return activeSubmission
    // Reserve the flight before any preparation can yield or call back into us.
    activeSubmission = Promise.resolve().then(runSubmission).finally(() => {
      activeSubmission = null
    })
    return activeSubmission
  }

  return { submitFeedback }
}
