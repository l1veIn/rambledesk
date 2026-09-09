import { derived, get, writable } from 'svelte/store'

import type { DraftView } from '../feedback'
import {
  decodeFeedbackDraftDocument,
  restoreFeedbackDraftDocument,
  snapshotFeedbackDraftDocument,
  type FeedbackDraftSnapshot,
} from '../feedbackDraftDocument'
import type { JSONContent } from '@tiptap/core'
import type { SavePhase } from '../domain/sessionPhases'

/**
 * Editing session for the current request: what the human is typing, what the server
 * last accepted, and where the save state machine stands.
 *
 * The workbench, the draft controller and the submission flow all read the same slice,
 * so this module — not the composition root — owns it.
 */
type DraftSessionFacts = Readonly<{
  body: string
  documentJson: string
  savedBody: string
  savedDocumentJson: string
  savedRevision: number
  phase: SavePhase
  message: string
  editorDocument: JSONContent | null
  editorEpoch: number
}>

export type DraftSessionState = DraftSessionFacts & Readonly<{ dirty: boolean }>

const initial: DraftSessionFacts = {
  body: '',
  documentJson: '',
  savedBody: '',
  savedDocumentJson: '',
  savedRevision: 0,
  phase: 'idle',
  message: '',
  editorDocument: null,
  editorEpoch: 0,
}

export type DraftSession = ReturnType<typeof createDraftSession>

export function createDraftSession() {
  const store = writable<DraftSessionFacts>(initial)
  const state = derived(store, (facts): DraftSessionState => ({
    ...facts,
    dirty: facts.documentJson !== facts.savedDocumentJson,
  }))

  function patch(next: Partial<DraftSessionFacts>) {
    store.update((current) => ({ ...current, ...next }))
  }

  function snapshot(): FeedbackDraftSnapshot {
    const state = get(store)
    return { documentJson: state.documentJson, bodyMarkdown: state.body }
  }

  function savedSnapshot(): FeedbackDraftSnapshot {
    const state = get(store)
    return { documentJson: state.savedDocumentJson, bodyMarkdown: state.savedBody }
  }

  function isDirty(): boolean {
    return get(state).dirty
  }

  /** Loads a server draft as both the current and the last accepted document. */
  function adopt(draft: DraftView, options: { loadEditor?: boolean } = {}): FeedbackDraftSnapshot {
    const restored = restoreFeedbackDraftDocument(draft.document_json, draft.body_markdown)
    const next = snapshotFeedbackDraftDocument(restored)
    patch({
      body: next.bodyMarkdown,
      documentJson: next.documentJson,
      savedBody: next.bodyMarkdown,
      savedDocumentJson: next.documentJson,
      savedRevision: draft.saved_revision,
      phase: draft.updated_at ? 'saved' : 'idle',
      message: '',
      ...(options.loadEditor === false ? {} : { editorDocument: restored, editorEpoch: get(store).editorEpoch + 1 }),
    })
    return next
  }

  /** Applies an editor snapshot as the current draft without touching the saved one. */
  function edit(next: FeedbackDraftSnapshot) {
    patch({
      body: next.bodyMarkdown,
      documentJson: next.documentJson,
      editorDocument: decodeFeedbackDraftDocument(next.documentJson),
      phase: next.documentJson === get(store).savedDocumentJson ? 'saved' : 'unsaved',
      message: '',
    })
  }

  function beginSave() {
    patch({ phase: 'saving', message: '' })
  }

  function acceptSaved(next: FeedbackDraftSnapshot, revision: number) {
    patch({
      savedBody: next.bodyMarkdown,
      savedDocumentJson: next.documentJson,
      savedRevision: revision,
      phase: get(store).documentJson === next.documentJson ? 'saved' : 'unsaved',
    })
  }

  function failSave(message: string) {
    patch({ phase: 'error', message })
  }

  /**
   * A mutation returned a newer server draft. Keep local edits when they differ from
   * what the server just accepted, otherwise adopt the server document.
   */
  function reconcile(draft: DraftView): 'adopted' | 'kept-local' {
    const remote = snapshotFeedbackDraftDocument(
      restoreFeedbackDraftDocument(draft.document_json, draft.body_markdown),
    )
    const local = snapshot()
    patch({
      savedBody: remote.bodyMarkdown,
      savedDocumentJson: remote.documentJson,
      savedRevision: draft.saved_revision,
    })
    if (local.documentJson === remote.documentJson) {
      patch({ body: remote.bodyMarkdown, documentJson: remote.documentJson, phase: 'saved' })
      return 'adopted'
    }
    patch({ phase: 'unsaved' })
    return 'kept-local'
  }

  function markSaved() {
    patch({ phase: 'saved' })
  }

  function markIdle() {
    patch({ phase: 'idle' })
  }

  function reset() {
    store.set(initial)
  }

  return {
    subscribe: state.subscribe,
    snapshot,
    savedSnapshot,
    isDirty,
    adopt,
    edit,
    beginSave,
    acceptSaved,
    failSave,
    reconcile,
    markSaved,
    markIdle,
    reset,
  }
}
