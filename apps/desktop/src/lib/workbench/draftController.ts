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

  async function saveDraftNow(): Promise<boolean> {
    cancelPendingSave()
    const workspace = context.getWorkspace()
    if (!workspace || context.isWorkspaceTerminal() || !context.session.isDirty()) return true
    if (activeSave) {
      await activeSave
      return context.session.isDirty() ? saveDraftNow() : get(context.session).phase !== 'error'
    }

    const requestId = workspace.request.request_id
    const snapshotToSave = context.session.snapshot()
    const revisionToSave = get(context.session).savedRevision
    context.session.beginSave()

    activeSave = (async () => {
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
        return true
      } catch (cause) {
        context.session.failSave(context.messageFrom(cause))
        return false
      }
    })()

    const succeeded = await activeSave
    activeSave = null
    if (
      succeeded &&
      context.getWorkspace()?.request.request_id === requestId &&
      context.session.isDirty()
    ) {
      return saveDraftNow()
    }
    return succeeded
  }

  return {
    updateDraft,
    saveDraftNow,
    cancelPendingSave,
    scheduleSave,
    isDirty: () => context.session.isDirty(),
  }
}
