import { get } from 'svelte/store'

import type { ApplicationTransport } from '../application/applicationTransport'
import type { ApproveFeedbackInput, CancelFeedbackInput } from '../feedback'
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
  rambleCanExit: () => boolean
  exitRamble: () => Promise<void>
  refreshNavigation: () => Promise<void>
  setPageError: (message: string) => void
  notifyApproved: () => void
  notifyCancelled: () => void
}

export type SubmissionController = ReturnType<typeof createSubmissionController>

export function createSubmissionController(context: SubmissionControllerContext) {
  async function approveFeedback() {
    const workspace = get(context.session).workspace
    if (!workspace || !workspace.request.allow_finish || get(context.session).approving) return
    if (!window.confirm(context.tr('Approve this final summary and end Pi’s Ramble flow?'))) return
    if (context.rambleCanExit()) await context.exitRamble()

    context.session.beginApprove()
    context.setPageError('')
    try {
      const input: ApproveFeedbackInput = { request_id: workspace.request.request_id }
      const result = await context.transport.call('approveFeedbackRequest', input)
      context.session.applyMutationResult(result)
      context.notifyApproved()
      await context.refreshNavigation()
    } catch (cause) {
      context.setPageError(context.messageFrom(cause))
    } finally {
      context.session.endApprove()
    }
  }

  async function cancelFeedback() {
    const workspace = get(context.session).workspace
    if (!workspace || !context.canCancel()) return
    if (context.rambleCanExit()) await context.exitRamble()

    context.session.beginCancel()
    context.setPageError('')
    try {
      const input: CancelFeedbackInput = {
        request_id: workspace.request.request_id,
        reason: 'Human cancelled from RambleDesk',
      }
      const result = await context.transport.call('cancelFeedbackRequest', input)
      context.session.applyMutationResult(result)
      context.draftSession.markSaved()
      context.notifyCancelled()
      await context.refreshNavigation()
    } catch (cause) {
      context.setPageError(context.messageFrom(cause))
    } finally {
      context.session.endCancel()
    }
  }

  async function openFeedbackPackage() {
    const workspace = get(context.session).workspace
    if (!context.session.feedbackResult() || !workspace) return
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

  return { approveFeedback, cancelFeedback, openFeedbackPackage }
}
