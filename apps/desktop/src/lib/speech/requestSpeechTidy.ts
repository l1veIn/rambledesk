import type { JSONContent } from '@tiptap/core'
import { writable } from 'svelte/store'
import type { FeedbackWorkspaceView } from '../feedback'
import { decodeFeedbackDraftDocument, snapshotFeedbackDraftDocument, type FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { tidySpeechSegments, type TidyConfig } from '../lightCleanup'
import { shouldAutoTidy } from '../tidyAuto'
import { applySpeechCleanupResults, speechCleanupCandidates, SPEECH_SEGMENT_ID_ATTR } from './speechBlockMetadata'
import { collectFieldSpeechSegments, applyFieldSpeechCleanup } from './fieldSpeechSegments'
import { workbenchIsReadOnly } from '../workbench/definitions/registry'

export type RequestSpeechTidyState = {
  requestId: string
  pendingCount: number
  busy: boolean
  applied: number
  skipped: number
  error: string
  canUndo: boolean
  disabled: boolean
}
export const emptyRequestSpeechTidy: RequestSpeechTidyState = {
  requestId: '', pendingCount: 0, busy: false, applied: 0, skipped: 0, error: '', canUndo: false, disabled: true,
}
type RequestSpeechTidyContext = {
  getWorkspace(): FeedbackWorkspaceView | null
  getConfig(): TidyConfig | null
  isLocked(): boolean
  /** Must call edit synchronously against the latest local draft, then publish it. */
  commit(requestId: string, edit: (workspace: FeedbackWorkspaceView) => FeedbackDraftSnapshot): void
  onConfigure?(): void
  tidy?: typeof tidySpeechSegments
}

function snapshot(workspace: FeedbackWorkspaceView): FeedbackDraftSnapshot {
  return { documentJson: workspace.draft.document_json ?? '', bodyMarkdown: workspace.draft.body_markdown }
}
function sameSnapshot(left: FeedbackDraftSnapshot, right: FeedbackDraftSnapshot): boolean {
  return left.documentJson === right.documentJson && left.bodyMarkdown === right.bodyMarkdown
}
function editable(workspace: FeedbackWorkspaceView | null): workspace is FeedbackWorkspaceView {
  return !!workspace && workspace.request.status !== 'completed' && workspace.request.status !== 'cancelled' && !workbenchIsReadOnly(workspace.workbench)
}
function documentFingerprints(doc: JSONContent): Map<string, string> {
  const result = new Map<string, string>()
  function visit(node: JSONContent) {
    const id = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
    if (typeof id === 'string') result.set(id, JSON.stringify(node))
    node.content?.forEach(visit)
  }
  visit(doc)
  return result
}
function captureBatch(workspace: FeedbackWorkspaceView) {
  const doc = decodeFeedbackDraftDocument(workspace.draft.document_json)
  return {
    document: doc ? speechCleanupCandidates(doc) : [],
    fields: collectFieldSpeechSegments(workspace),
    fingerprints: doc ? documentFingerprints(doc) : new Map<string, string>(),
  }
}
type SpeechTidyBatch = ReturnType<typeof captureBatch>

function automaticBatchKey(requestId: string, threshold: number, batch: SpeechTidyBatch): string {
  return JSON.stringify([requestId, threshold, batch.document, batch.fields])
}

/** Rebase the captured batch onto the latest draft. Each destination applies its
 * own stale-edit guard, so one changed field does not discard other results. */
function applyBatch(workspace: FeedbackWorkspaceView, batch: SpeechTidyBatch, results: readonly string[]) {
  const before = snapshot(workspace)
  const doc = decodeFeedbackDraftDocument(before.documentJson)
  const fingerprints = doc ? documentFingerprints(doc) : new Map<string, string>()
  const documentReplacements = batch.document.flatMap((segment, index) => {
    if (fingerprints.get(segment.segmentId) !== batch.fingerprints.get(segment.segmentId)) return []
    return [{ segmentId: segment.segmentId, originalText: segment.text, nextText: results[index]! }]
  })
  const documentResult = doc && documentReplacements.length ? applySpeechCleanupResults(doc, documentReplacements) : null
  const withDocument = documentResult?.changed
    ? snapshotFeedbackDraftDocument(documentResult.document, before.documentJson) : before
  const fieldReplacements = batch.fields.map((segment, index) => ({
    segmentId: segment.segmentId,
    target: segment.target,
    originalText: segment.text,
    nextText: results[batch.document.length + index]!,
  }))
  const fieldResult = applyFieldSpeechCleanup({ ...workspace, draft: {
    ...workspace.draft, document_json: withDocument.documentJson, body_markdown: withDocument.bodyMarkdown,
  } }, fieldReplacements)
  return {
    before,
    after: fieldResult.snapshot,
    applied: (documentResult?.replacementsApplied ?? 0) + fieldResult.applied.length,
  }
}

/** One request-wide batch. Routing is captured per segment, independently of focus. */
export function createRequestSpeechTidy(context: RequestSpeechTidyContext) {
  let state = { ...emptyRequestSpeechTidy }
  const store = writable(state)
  let generation = 0
  let lastAutomatic = ''
  let automaticThreshold = 0
  let undo: { requestId: string; before: FeedbackDraftSnapshot; after: FeedbackDraftSnapshot } | null = null
  function patch(value: Partial<RequestSpeechTidyState>) {
    const next = { ...state, ...value }
    if (Object.keys(next).some((key) => next[key as keyof typeof next] !== state[key as keyof typeof state])) {
      state = next; store.set(state)
    }
  }
  function refresh(workspace: FeedbackWorkspaceView | null = context.getWorkspace(), threshold = automaticThreshold) {
    automaticThreshold = threshold
    const requestId = workspace?.request.request_id ?? ''
    if (requestId !== state.requestId) {
      generation += 1; undo = null; lastAutomatic = ''
      patch({ ...emptyRequestSpeechTidy, requestId })
    }
    const batch = editable(workspace) ? captureBatch(workspace) : null
    const pendingCount = batch ? batch.document.length + batch.fields.length : 0
    const unchanged = !!workspace && undo?.requestId === requestId && sameSnapshot(snapshot(workspace), undo.after)
    patch({ pendingCount, disabled: context.isLocked() || !editable(workspace), canUndo: !!unchanged && !context.isLocked() && editable(workspace) })
    if (!shouldAutoTidy(pendingCount, threshold)) { lastAutomatic = ''; return }
    const config = context.getConfig()
    if (state.busy || context.isLocked() || !config?.apiKey.trim() || !config.model.trim() || !batch) return
    const key = automaticBatchKey(requestId, threshold, batch)
    if (key !== lastAutomatic) { lastAutomatic = key; void run(true) }
  }
  async function run(automatic = false) {
    const workspace = context.getWorkspace()
    if (state.busy || context.isLocked() || !editable(workspace)) return
    const config = context.getConfig()
    if (!config?.apiKey.trim() || !config.model.trim()) {
      if (!automatic) { patch({ error: 'Configure Tidy in Settings → Post-processing → Tidy first.' }); context.onConfigure?.() }
      return
    }
    const batch = captureBatch(workspace)
    const all = [...batch.document, ...batch.fields]
    if (!all.length) return
    const requestId = workspace.request.request_id
    const runGeneration = generation
    patch({ busy: true, applied: 0, skipped: 0, error: '' })
    try {
      const result = await (context.tidy ?? tidySpeechSegments)(all, config)
      if (runGeneration !== generation || context.getWorkspace()?.request.request_id !== requestId) return
      if (!result || result.length !== all.length) throw new Error('Tidy did not write back because the model output did not match the original segments.')
      if (context.isLocked() || !editable(context.getWorkspace())) throw new Error('The request changed while tidying. No text was replaced.')
      let applied = 0
      context.commit(requestId, (latest) => {
        if (latest.request.request_id !== requestId || !editable(latest)) throw new Error('The request changed while tidying. No text was replaced.')
        const changed = applyBatch(latest, batch, result)
        applied = changed.applied
        if (applied) undo = { requestId, before: changed.before, after: changed.after }
        return changed.after
      })
      patch({ applied, skipped: all.length - applied })
    } catch (cause) {
      if (runGeneration === generation) patch({ error: cause instanceof Error ? cause.message : String(cause) })
    } finally {
      if (runGeneration === generation) { patch({ busy: false }); refresh() }
    }
  }
  function undoLast() {
    if (!undo || state.busy || context.isLocked()) return
    const action = undo
    try {
      context.commit(action.requestId, (latest) => {
        if (latest.request.request_id !== action.requestId || !editable(latest) || !sameSnapshot(snapshot(latest), action.after)) {
          throw new Error('The text has changed since tidying. Your edits have been kept.')
        }
        return action.before
      })
      // Undo is an explicit choice to keep this batch's original wording. Do not
      // immediately redo it just because its restored count meets Auto Tidy.
      const restored = context.getWorkspace()
      if (restored?.request.request_id === action.requestId) {
        lastAutomatic = automaticBatchKey(action.requestId, automaticThreshold, captureBatch(restored))
      }
      undo = null; patch({ applied: 0, skipped: 0, error: '', canUndo: false }); refresh()
    } catch (cause) { patch({ error: cause instanceof Error ? cause.message : String(cause) }) }
  }
  return { subscribe: store.subscribe, refresh, run, undo: undoLast }
}
export type RequestSpeechTidyController = ReturnType<typeof createRequestSpeechTidy>
