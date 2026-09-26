import { writable } from 'svelte/store'
import type { FeedbackWorkspaceView } from '../feedback'
import type { TidyConfig } from '../lightCleanup'
import type { DraftSession } from './draftSession'
import type { DraftController } from './draftController'
import { provideVoiceInput, type VoiceInputContext, type VoiceInputState } from '../speech/voiceInputContext'
import { provideRequestSpeechTools } from '../speech/requestSpeechToolsContext'
import { createRequestSpeechTidy } from '../speech/requestSpeechTidy'

/** Composes request input against the latest local draft, including unsaved typing. */
export function createRequestInputComposition(context: {
  getWorkspace(): FeedbackWorkspaceView | null
  draft: DraftSession
  draftController: DraftController
  getConfig(): TidyConfig | null
  isLocked(): boolean
  onConfigure(): void
  voice: Omit<VoiceInputContext, 'state'>
}) {
  function workspace() {
    const current = context.getWorkspace()
    const snapshot = context.draft.snapshot()
    return current ? { ...current, draft: { ...current.draft, document_json: snapshot.documentJson, body_markdown: snapshot.bodyMarkdown } } : null
  }
  const voiceState = writable<VoiceInputState>({ requestId: '', documentTarget: null, nextTarget: null, recording: false, disabled: true })
  provideVoiceInput({ ...context.voice, state: voiceState })
  const tidy = createRequestSpeechTidy({
    getWorkspace: workspace, getConfig: context.getConfig, isLocked: context.isLocked, onConfigure: context.onConfigure,
    commit(requestId, edit) {
      const current = workspace()
      if (!current || current.request.request_id !== requestId || context.isLocked()) throw new Error('The request changed while tidying. No text was replaced.')
      context.draft.edit(edit(current), { loadEditor: true })
      context.draftController.scheduleSave()
    },
  })
  provideRequestSpeechTools(tidy)
  return { voiceState, tidy, workspace }
}
