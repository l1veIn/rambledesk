import { get } from 'svelte/store'

import type { ApplicationTransport } from '../application/applicationTransport'
import type { FeedbackPreparation } from '../speech/rambleSessionControllerHandle'
import type { PublishedFeedbackAction } from '../publishedFeedbackAction'
import type { DraftSession } from './draftSession'
import type { WorkspaceSession } from './workspaceSession'

/**
 * Terminal mutations for the open request: approve, cancel and open the package.
 *
 * State lives in the workspace and draft sessions; this controller owns the calls
 * and the user-facing notifications the shell provides.
 */
export type SubmissionControllerContext = {
  transport: ApplicationTransport
  session: WorkspaceSession
  draftSession: DraftSession
  publishedFeedbackAction: PublishedFeedbackAction
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  canCancel: () => boolean
  prepareFeedback: (requestId: string) => Promise<FeedbackPreparation>
  saveDraftNow: () => Promise<boolean>
  confirmApproval?: (message: string) => boolean
  refreshNavigation: () => Promise<void>
  setPageError: (message: string) => void
  notifyApproved: () => void
  notifyCancelled: () => void
}

export type SubmissionController = ReturnType<typeof createSubmissionController>

export function createSubmissionController(context: SubmissionControllerContext) {
  type Intent = 'approve' | 'cancel'
  let active: { intent: Intent; promise: Promise<void> } | null = null

  function current(requestId: string) {
    return context.session.requestId() === requestId && !context.session.isTerminal()
  }

  function reportFor(requestId: string, cause: unknown) {
    if (context.session.requestId() === requestId) context.setPageError(context.messageFrom(cause))
  }

  async function finishRequest(intent: Intent, requestId: string) {
    const state = get(context.session)
    if (state.request?.request_id !== requestId || state.terminal || state.interactionLocked ||
      (intent === 'approve' ? !state.request?.allow_finish : !context.canCancel())) return
    if (intent === 'approve' && !(context.confirmApproval ?? window.confirm.bind(window))(
      context.tr('Approve this final summary and end Pi’s Ramble flow?'),
    )) return

    let locked = false
    try {
      const preparation = await context.prepareFeedback(requestId)
      if (!current(requestId) || get(context.session).interactionLocked) return
      if (preparation.kind === 'failed') {
        context.setPageError(preparation.message)
        return
      }
      if (preparation.kind === 'pending-speech') {
        context.setPageError(context.tr('Review the pending speech in the capsule before ending this request.'))
        return
      }
      if (intent === 'approve' ? !context.session.request()?.allow_finish : !context.canCancel()) return
      // Final speech must enter the editable draft before we freeze it for saving.
      if (intent === 'approve') context.session.beginApprove()
      else context.session.beginCancel()
      locked = true
      context.setPageError('')
      if (!(await context.saveDraftNow())) {
        reportFor(requestId, get(context.draftSession).message || context.tr('The current draft could not be saved.'))
        return
      }
      if (!current(requestId)) return
      const result = intent === 'approve'
        ? await context.transport.call('approveFeedbackRequest', { request_id: requestId })
        : await context.transport.call('cancelFeedbackRequest', {
          request_id: requestId, reason: 'Human cancelled from RambleDesk',
        })
      const visible = context.session.applyMutationResult(result)
      if (visible) {
        if (intent === 'approve') context.notifyApproved()
        else context.notifyCancelled()
      }
      // The terminal response is committed independently of this follow-up read.
      try {
        await context.refreshNavigation()
      } catch (cause) {
        reportFor(requestId, context.tr('The request was updated, but navigation could not be refreshed: {error}', {
          error: context.messageFrom(cause),
        }))
      }
    } catch (cause) {
      reportFor(requestId, cause)
    } finally {
      if (locked && context.session.requestId() === requestId) {
        if (intent === 'approve') context.session.endApprove()
        else context.session.endCancel()
      }
    }
  }

  function requestFinish(intent: Intent): Promise<void> {
    if (active) return active.intent === intent ? active.promise : Promise.resolve()
    const requestId = context.session.requestId()
    if (!requestId) return Promise.resolve()
    // Reserve before confirmation, input preparation or a store subscriber can re-enter.
    const promise = Promise.resolve().then(() => finishRequest(intent, requestId)).finally(() => {
      active = null
    })
    active = { intent, promise }
    return promise
  }

  async function openFeedbackPackage() {
    const workspace = get(context.session).workspace
    if (!get(context.session).feedbackResult || !workspace) return
    try {
      await context.publishedFeedbackAction.run(workspace.request.request_id)
    } catch (cause) {
      context.setPageError(context.tr(
        context.publishedFeedbackAction.label === 'Open feedback package'
          ? 'Could not open Feedback Package: {error}'
          : 'Could not download published feedback: {error}',
        { error: context.messageFrom(cause) },
      ))
    }
  }

  return {
    approveFeedback: () => requestFinish('approve'),
    cancelFeedback: () => requestFinish('cancel'),
    openFeedbackPackage,
  }
}
