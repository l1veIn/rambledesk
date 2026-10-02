import { describe, expect, it, vi } from 'vitest'
import type { WorkbenchSpec, WorkbenchState } from '../../generated/feedback'
import { workbenchFieldTarget, type InputTarget, type InputWriteWorkspace } from '../../domain/inputTarget'
import { decodeFeedbackDraftEnvelope, snapshotFeedbackDraftMarkdown, type FeedbackDraftSnapshot } from '../../feedbackDraftDocument'
import { withWorkbenchState } from '../../workbenchState'
import { applyInputTextWriteback } from '../../inputTextWriteback'
import { readWorkbenchField } from '../../workbenchFields'
import { applySpeechWriteback } from '../../speech/speechWriteback'
import { applyFieldSpeechCleanup, collectFieldSpeechSegments, fieldSpeechSegmentsForTarget } from '../../speech/fieldSpeechSegments'
import { captureFieldTidy, replaceFieldTidy } from '../../speech/fieldSpeechText'
import { removeWorkbenchAttachmentReferences } from '../../input/fieldAttachmentText'

type ProbeState = { type: 'field_probe'; entityId: string; opinion: string; editable: boolean }
type ProbeData = { source_version: string; material: string }
const probeRegistry = vi.hoisted(() => {
  const decode = (raw: unknown): WorkbenchState | null => {
    const state = raw as ProbeState | undefined
    return state?.type === 'field_probe' && typeof state.entityId === 'string' && typeof state.opinion === 'string'
      && typeof state.editable === 'boolean' ? raw as WorkbenchState : null
  }
  const field = {
    accepts: (target: InputTarget) => target.destination.kind === 'workbench_field' && target.destination.field === 'opinion',
    text(raw: WorkbenchState, target: InputTarget): string | null {
      const state = raw as unknown as ProbeState
      return target.destination.kind === 'workbench_field' && state.type === 'field_probe' && state.editable
        && target.destination.entityId === state.entityId ? state.opinion : null
    },
    removeAttachment(raw: WorkbenchState, _attachmentId: string, remove: (value: string) => string) {
      const state = raw as unknown as ProbeState
      return { ...state, opinion: remove(state.opinion) } as unknown as WorkbenchState
    },
    read({ spec, state: raw, target }: { spec: WorkbenchSpec; state: WorkbenchState; target: InputTarget }) {
      const state = raw as unknown as ProbeState, data = spec.data as unknown as ProbeData
      const destination = target.destination
      if (destination.kind !== 'workbench_field' || state.type !== 'field_probe' || !state.editable
        || destination.entityId !== state.entityId || destination.sourceVersion !== data.source_version) throw new Error('The opinion target is no longer available.')
      return { value: state.opinion, limit: 100, contract: JSON.stringify(data), identity: state.entityId,
        replace: (opinion: string) => ({ ...state, opinion }) as unknown as WorkbenchState }
    },
  }
  const definition = { type: 'field_probe', version: 1, fields: [field], decodeState: decode }
  return {
    workbenchDefinitions: [definition],
    getWorkbenchDefinition: (type: string, version = 1) => type === definition.type && version === 1 ? definition : undefined,
    resolveWorkbenchDefinition: (spec: WorkbenchSpec | undefined) => spec?.type === definition.type && spec.version === 1 ? definition : null,
    decodeRegisteredWorkbenchState: decode,
  }
})
// A future module enters only through the definition registry; no central field code knows this type.
vi.mock('../definitions/registry', () => probeRegistry)

const target = () => workbenchFieldTarget({ requestId: 'r', requestTitle: 'Rating' }, {
  workbenchType: 'field_probe', version: 1, field: 'opinion', entityId: 'review', sourceVersion: 'draft-1', label: 'Opinion',
})
const spec = (): WorkbenchSpec => ({ type: 'field_probe', version: 1,
  data: { source_version: 'draft-1', material: 'Immutable evidence' } }) as unknown as WorkbenchSpec
const state = (changes: Partial<ProbeState> = {}): WorkbenchState => ({
  type: 'field_probe', entityId: 'review', opinion: '', editable: true, ...changes,
}) as unknown as WorkbenchState
function workspace(changes: Partial<ProbeState> = {}): InputWriteWorkspace {
  const initial = withWorkbenchState(snapshotFeedbackDraftMarkdown('Keep this feedback'), state(changes))
  const envelope = decodeFeedbackDraftEnvelope(initial.documentJson)!
  return { request: { request_id: 'r', status: 'in_progress' }, workbench: spec(),
    draft: { document_json: JSON.stringify({ ...envelope, futureEvidence: { preserved: true } }), body_markdown: initial.bodyMarkdown } }
}
function updated(current: InputWriteWorkspace, snapshot: FeedbackDraftSnapshot): InputWriteWorkspace {
  return { ...current, draft: { document_json: snapshot.documentJson, body_markdown: snapshot.bodyMarkdown } }
}
const opinion = (snapshot: FeedbackDraftSnapshot) => (decodeFeedbackDraftEnvelope(snapshot.documentJson)?.workbenchState as ProbeState).opinion

describe('registered extension field input', () => {
  it('routes pasted text and attachment references through the local adapter with idempotent receipts', () => {
    const current = workspace()
    const input = { target: target(), text: 'Useful command\n[shot](attachment://shot)', id: 'paste-1' }
    const snapshot = applyInputTextWriteback(current, input)
    expect(opinion(snapshot)).toBe(input.text)
    expect(snapshot.bodyMarkdown).toBe('Keep this feedback')
    expect(decodeFeedbackDraftEnvelope(snapshot.documentJson)?.futureEvidence).toEqual({ preserved: true })
    const renamed = target()
    if (renamed.destination.kind === 'workbench_field') renamed.destination.label = 'A different label'
    expect(applyInputTextWriteback(updated(current, snapshot), { ...input, target: renamed })).toEqual(snapshot)
    expect(() => applyInputTextWriteback(updated(current, snapshot), { ...input, text: 'Changed receipt' })).toThrow('different write')
    const removed = removeWorkbenchAttachmentReferences(snapshot, 'shot')
    expect(opinion(removed)).toBe('Useful command\n')
    expect(removed.bodyMarkdown).toBe(snapshot.bodyMarkdown)
  })

  it('records extension speech and performs guarded span cleanup without losing the shared body', () => {
    const current = workspace({ opinion: 'Typed opinion' })
    const snapshot = applySpeechWriteback(current, { ...target(), id: 'speech-1', text: '嗯，新菜单很清楚🙂' })
    expect(opinion(snapshot)).toBe('Typed opinion\n嗯，新菜单很清楚🙂')
    expect(fieldSpeechSegmentsForTarget(snapshot, target())).toHaveLength(1)
    expect(collectFieldSpeechSegments(updated(current, snapshot))).toHaveLength(1)
    const cleaned = applyFieldSpeechCleanup(updated(current, snapshot), [{
      segmentId: 'speech-1', target: target(), originalText: '嗯，新菜单很清楚🙂', nextText: '新菜单很清楚🙂',
    }])
    expect(cleaned.applied).toEqual(['speech-1'])
    expect(opinion(cleaned.snapshot)).toBe('Typed opinion\n新菜单很清楚🙂')
    expect(cleaned.snapshot.bodyMarkdown).toBe('Keep this feedback')
    expect(applySpeechWriteback(updated(current, snapshot), { ...target(), id: 'speech-1', text: '嗯，新菜单很清楚🙂' })).toEqual(snapshot)
  })

  it('fails fixed-target writes after removal, source change, request change or unsupported versions', () => {
    const captured = target()
    const unavailable = [workspace({ editable: false }), workspace({ entityId: 'replacement' }),
      { ...workspace(), request: { request_id: 'other', status: 'in_progress' as const } },
      { ...workspace(), workbench: { ...spec(), version: 2 } },
      { ...workspace(), workbench: { ...spec(), data: { source_version: 'draft-2', material: 'Immutable evidence' } } as unknown as WorkbenchSpec },
    ]
    for (const current of unavailable) {
      const before = current.draft.document_json
      expect(() => applyInputTextWriteback(current, { target: captured, text: 'Do not redirect', id: 'paste' })).toThrow()
      expect(() => applySpeechWriteback(current, { ...captured, text: 'Do not redirect', id: 'speech' })).toThrow()
      expect(current.draft.document_json).toBe(before)
      expect(current.draft.body_markdown).toBe('Keep this feedback')
    }
    expect(() => readWorkbenchField(workspace(), { ...captured, destination: { kind: 'unknown', raw: { field: '/doc' } } })).toThrow('unavailable')
  })

  it('protects manual edits and changed entity identity while asynchronous tidy is pending', () => {
    const current = workspace({ opinion: 'Captured original' })
    const capture = captureFieldTidy(current, target())
    expect(() => replaceFieldTidy(workspace({ opinion: 'Edited while waiting' }), capture, 'Captured original', 'Cleaned')).toThrow('edited')
    expect(() => replaceFieldTidy(workspace({ opinion: 'Captured original', entityId: 'replacement' }), capture, 'Captured original', 'Cleaned')).toThrow('no longer available')
    const replaced = replaceFieldTidy(current, capture, 'Captured original', 'Cleaned')
    expect(opinion(replaced)).toBe('Cleaned')
    expect(decodeFeedbackDraftEnvelope(replaced.documentJson)?.futureEvidence).toEqual({ preserved: true })
    expect(() => readWorkbenchField(current, target()).replace('x'.repeat(101))).toThrow('limit')
  })
})
