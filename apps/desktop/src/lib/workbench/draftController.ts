// Draft persistence state machine for the workbench: debounced autosave with
// revision-aware conflict handling. The editing state lives in the draft session;
// this controller owns only the save timer and the in-flight save promise.

import { get } from 'svelte/store'

import type { ApplicationTransport } from '../application/applicationTransport'
import type { DraftView, FeedbackWorkspaceView, SaveDraftInput } from '../feedback'
import type { FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import type { DraftSession } from './draftSession'

export type DraftControllerContext = {
  transport: ApplicationTransport
  messageFrom: (cause: unknown) => string
  isInteractionLocked: () => boolean
  isWorkspaceTerminal: () => boolean
  getWorkspace: () => FeedbackWorkspaceView | null
  /** Owns the current and last-accepted documents for the open request. */
  session: DraftSession
  setWorkspaceDraft: (draft: DraftView) => void
}

export type DraftController = ReturnType<typeof createDraftController>

export function createDraftController(context: DraftControllerContext) {
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let activeSave: Promise<boolean> | null = null

  function cancelPendingSave() {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = undefined
    }
  }

  function scheduleSave(delayMs = 700) {
    cancelPendingSave()
    saveTimer = setTimeout(() => void saveDraftNow(), delayMs)
  }

  function updateDraft(snapshot: FeedbackDraftSnapshot) {
    if (
      context.isInteractionLocked() ||
      context.getWorkspace() === null ||
      context.isWorkspaceTerminal()
    ) return
    context.session.edit(snapshot)
    scheduleSave()
  }

  function saveDraftNow(): Promise<boolean> {
    cancelPendingSave()
    // Even a locally clean draft must wait: an in-flight save may replace its
    // saved baseline. Every caller joins the whole drain, including its failure.
    if (activeSave) return activeSave

    // Publish the shared promise before beginSave notifies session subscribers.
    activeSave = Promise.resolve().then(drainSaves)
    return activeSave
  }

  async function drainSaves(): Promise<boolean> {
    try {
      while (true) {
        const workspace = context.getWorkspace()
        if (!workspace || context.isWorkspaceTerminal() || !context.session.isDirty()) return true

        const requestId = workspace.request.request_id
        const snapshotToSave = context.session.snapshot()
        const revisionToSave = get(context.session).savedRevision
        context.session.beginSave()
        try {
          const input: SaveDraftInput = {
            request_id: requestId,
            document_json: snapshotToSave.documentJson,
            body_markdown: snapshotToSave.bodyMarkdown,
            expected_revision: revisionToSave,
          }
          const saved: DraftView = await context.transport.call('saveFeedbackDraft', input)
          if (context.getWorkspace()?.request.request_id === requestId) {
            context.session.acceptSaved(snapshotToSave, saved.saved_revision)
            context.setWorkspaceDraft(saved)
          }
        } catch (cause) {
          if (context.getWorkspace()?.request.request_id === requestId) {
            // A debounce created during this save must not silently retry a
            // failed CAS. Leave another request's autosave schedule untouched.
            cancelPendingSave()
            context.session.failSave(context.messageFrom(cause))
          }
          return false
        }
        // Re-read both the current document and accepted revision after each save,
        // so edits made while awaiting the server stay inside the same drain.
      }
    } finally {
      // Clear at the final dirty check, before promise settlement gives queued
      // callers a chance to attach new edits to an already completed drain.
      activeSave = null
    }
  }

  return {
    updateDraft,
    saveDraftNow,
    cancelPendingSave,
    scheduleSave,
    isDirty: () => context.session.isDirty(),
    hasPendingSave: () => activeSave !== null,
  }
}
