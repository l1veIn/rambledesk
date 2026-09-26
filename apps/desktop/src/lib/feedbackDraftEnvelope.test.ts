import { describe, expect, it } from 'vitest'
import {
  applyFeedbackDraftSnapshot,
  decodeFeedbackDraftEnvelope,
  restoreFeedbackDraftSnapshot,
  snapshotFeedbackDraftMarkdown,
  updateFeedbackDraftDocument,
  updateFeedbackDraftState,
} from './feedbackDraftDocument'
import { createDraftSession } from './workbench/draftSession'

const futureState = { type: 'future_workbench', nested: { selection: [1, 3] } }
const saved = () => ({ ...snapshotFeedbackDraftMarkdown('Original notes'), documentJson: JSON.stringify({
  ...JSON.parse(snapshotFeedbackDraftMarkdown('Original notes').documentJson),
  workbenchState: futureState,
  futureMetadata: { version: 4, value: 'must survive' },
}) })

describe('complete feedback draft envelopes', () => {
  it('preserves opaque state and extensions while restoring and transforming only the document', () => {
    const restored = restoreFeedbackDraftSnapshot(saved().documentJson, 'stale markdown')
    const changed = updateFeedbackDraftDocument(restored, (doc) => ({ ...doc, content: [] }))
    expect(changed.bodyMarkdown).toBe('')
    expect(decodeFeedbackDraftEnvelope(changed.documentJson)).toMatchObject({ workbenchState: futureState, futureMetadata: { version: 4, value: 'must survive' } })
    expect(restored.bodyMarkdown).toBe('Original notes')
  })

  it('accepts editor-only snapshots without interpreting or dropping workbench state', () => {
    const next = applyFeedbackDraftSnapshot(saved(), snapshotFeedbackDraftMarkdown('Edited notes'))
    expect(next.bodyMarkdown).toBe('Edited notes')
    expect(decodeFeedbackDraftEnvelope(next.documentJson)?.workbenchState).toEqual(futureState)
    expect(next.bodyMarkdown).not.toContain('future_workbench')
  })

  it('uses explicit incoming state and allows clearing it without restoring the previous state', () => {
    const next = updateFeedbackDraftState(saved(), { type: 'single_choice', selected_option_id: null })
    expect(decodeFeedbackDraftEnvelope(applyFeedbackDraftSnapshot(saved(), next).documentJson)?.workbenchState).toEqual({ type: 'single_choice', selected_option_id: null })
    const cleared = updateFeedbackDraftState(next, null)
    expect(decodeFeedbackDraftEnvelope(applyFeedbackDraftSnapshot(next, cleared).documentJson)?.workbenchState).toBeNull()
  })

  it('preserves opaque state through session edits and stale saves without copying it to another request', () => {
    const session = createDraftSession()
    session.adopt({ document_json: saved().documentJson, body_markdown: '', saved_revision: 1, updated_at: null })
    session.edit(snapshotFeedbackDraftMarkdown('Local notes'))
    session.reconcile({ document_json: saved().documentJson, body_markdown: '', saved_revision: 2, updated_at: null })
    expect(session.snapshot().bodyMarkdown).toBe('Local notes')
    expect(decodeFeedbackDraftEnvelope(session.snapshot().documentJson)?.workbenchState).toEqual(futureState)
    session.adopt({ document_json: null, body_markdown: 'Other request', saved_revision: 0, updated_at: null })
    expect(decodeFeedbackDraftEnvelope(session.snapshot().documentJson)?.workbenchState).toBeUndefined()
  })
})
