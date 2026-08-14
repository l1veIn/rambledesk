// Draft persistence state machine for the workbench: debounced autosave with
// revision-aware conflict handling. All state lives in the component; this
// controller only owns the save timer and the in-flight save promise.

import { invoke } from '@tauri-apps/api/core'

import type { DraftView, FeedbackWorkspaceView, SaveDraftInput } from '../feedback'
import type { SavePhase } from './types'

export type DraftControllerContext = {
  messageFrom: (cause: unknown) => string
  isPreviewMode: () => boolean
  isInteractionLocked: () => boolean
  isWorkspaceTerminal: () => boolean
  getWorkspace: () => FeedbackWorkspaceView | null
  getBody: () => string
  setBody: (body: string) => void
  getSavedBody: () => string
  setSavedBody: (body: string) => void
  getSavedRevision: () => number
  setSavedRevision: (revision: number) => void
  getPhase: () => SavePhase
  setPhase: (phase: SavePhase) => void
  setMessage: (message: string) => void
  setWorkspaceDraft: (draft: DraftView) => void
}

export type DraftController = ReturnType<typeof createDraftController>

export function createDraftController(context: DraftControllerContext) {
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let activeSave: Promise<boolean> | null = null

  function dirty(): boolean {
    return context.getBody() !== context.getSavedBody()
  }

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

  function updateDraft(value: string) {
    if (
      context.isInteractionLocked() ||
      context.getWorkspace() === null ||
      context.isWorkspaceTerminal()
    ) return
    context.setBody(value)
    context.setPhase(context.getBody() === context.getSavedBody() ? 'saved' : 'unsaved')
    context.setMessage('')
    scheduleSave()
  }

  async function saveDraftNow(): Promise<boolean> {
    cancelPendingSave()
    const workspace = context.getWorkspace()
    if (!workspace || context.isWorkspaceTerminal() || !dirty()) return true
    if (activeSave) {
      await activeSave
      return dirty() ? saveDraftNow() : context.getPhase() !== 'error'
    }

    const requestId = workspace.request.request_id
    const bodyToSave = context.getBody()
    const revisionToSave = context.getSavedRevision()
    context.setPhase('saving')
    context.setMessage('')

    activeSave = (async () => {
      try {
        const input: SaveDraftInput = {
          request_id: requestId,
          body_markdown: bodyToSave,
          expected_revision: revisionToSave,
        }
        const saved: DraftView = context.isPreviewMode()
          ? {
              body_markdown: bodyToSave,
              saved_revision: revisionToSave + 1,
              updated_at: new Date().toISOString(),
            }
          : await invoke<DraftView>('save_feedback_draft', { input })
        if (context.getWorkspace()?.request.request_id === requestId) {
          context.setSavedBody(bodyToSave)
          context.setSavedRevision(saved.saved_revision)
          context.setWorkspaceDraft(saved)
          context.setPhase(context.getBody() === bodyToSave ? 'saved' : 'unsaved')
        }
        return true
      } catch (cause) {
        context.setPhase('error')
        context.setMessage(context.messageFrom(cause))
        return false
      }
    })()

    const succeeded = await activeSave
    activeSave = null
    if (succeeded && context.getWorkspace()?.request.request_id === requestId && dirty()) {
      return saveDraftNow()
    }
    return succeeded
  }

  return { updateDraft, saveDraftNow, cancelPendingSave, scheduleSave, isDirty: dirty }
}
