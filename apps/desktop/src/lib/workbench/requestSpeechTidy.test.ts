import type { JSONContent } from '@tiptap/core'
import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import type { FeedbackWorkspaceView } from '../feedback'
import { snapshotFeedbackDraftDocument, type FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import type { TidyConfig } from '../lightCleanup'
import { applySpeechWriteback } from '../speech/speechWriteback'
import { asrParagraphAttrs, type SpeechCleanupSegment } from '../speech/speechBlockMetadata'
import { reconcileFieldSpeechSegments } from '../speech/fieldSpeechSegments'
import { createRequestSpeechTidy } from '../speech/requestSpeechTidy'

const config: TidyConfig = { provider: 'openai', apiKey: 'test-key', baseUrl: '', model: 'test-model', reasoningEffort: 'none', locale: 'en' }
const paragraph = (text: string, id?: string): JSONContent => ({ type: 'paragraph',
  ...(id ? { attrs: asrParagraphAttrs(id, 'pending') } : {}), content: [{ type: 'text', text }] })
const snapshotOf = (workspace: FeedbackWorkspaceView): FeedbackDraftSnapshot => ({
  documentJson: workspace.draft.document_json!, bodyMarkdown: workspace.draft.body_markdown,
})
function apply(workspace: FeedbackWorkspaceView, snapshot: FeedbackDraftSnapshot) {
  workspace.draft = { ...workspace.draft, document_json: snapshot.documentJson, body_markdown: snapshot.bodyMarkdown }
}
function appendSpeech(workspace: FeedbackWorkspaceView, questionId: string, id: string, text: string) {
  apply(workspace, applySpeechWriteback(workspace, { id, text, requestId: workspace.request.request_id, requestTitle: 'Questions',
    destination: { kind: 'question_answer', questionId, questionLabel: questionId } }))
}
function fixture(requestId = 'r'): FeedbackWorkspaceView {
  const snapshot = snapshotFeedbackDraftDocument({ type: 'doc', content: [paragraph('Typed note'), paragraph('Um spoken note', 'notes-1')] })
  const workspace: FeedbackWorkspaceView = {
    request: { request_id: requestId, host_id: 'host', host_session_id: 'session', source_hint: null, title: 'Questions', what_happened: '',
      status: 'in_progress', resolution: null, allow_finish: false, final_summary: null, revision: 1, created_at: '', updated_at: '' },
    workbench: { type: 'questions', version: 1, data: { questions: ['q1', 'q2'].map((id) => ({ id, prompt: id, allowOther: true,
      options: [{ value: 'yes', label: 'Yes' }, { value: 'no', label: 'No' }] })) } },
    actions: [], context_refs: [], request_attachments: [], attachments: [], feedback: null,
    draft: { document_json: JSON.stringify({ ...JSON.parse(snapshot.documentJson), opaque: { keep: true }, workbenchState: {
      type: 'questions', answers: ['q1', 'q2'].map((id) => ({ id, value: `Typed ${id}`, label: `Typed ${id}`, wasCustom: true })),
    } }), body_markdown: snapshot.bodyMarkdown, saved_revision: 1, updated_at: '' },
  }
  appendSpeech(workspace, 'q1', 'answer-1', 'Um first answer')
  appendSpeech(workspace, 'q2', 'answer-2', 'Um second answer')
  return workspace
}
function edit(workspace: FeedbackWorkspaceView, change: (envelope: any) => void) {
  const previous = snapshotOf(workspace)
  const envelope = JSON.parse(previous.documentJson)
  change(envelope)
  const next = snapshotFeedbackDraftDocument(envelope.doc, JSON.stringify(envelope))
  apply(workspace, reconcileFieldSpeechSegments(previous, next))
}
function answers(workspace: FeedbackWorkspaceView) {
  return JSON.parse(workspace.draft.document_json!).workbenchState.answers as Array<{ id: string; value: string; label: string; wasCustom: boolean }>
}
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}
function harness() {
  let workspace: FeedbackWorkspaceView | null = fixture()
  let locked = false
  let settings: TidyConfig | null = config
  const tidy = vi.fn(async (segments: SpeechCleanupSegment[]): Promise<string[] | null> => segments.map((segment) => segment.text.replace('Um ', '')))
  const configure = vi.fn()
  const commit = vi.fn((requestId: string, change: (latest: FeedbackWorkspaceView) => FeedbackDraftSnapshot) => {
    if (!workspace || workspace.request.request_id !== requestId) throw new Error('Request changed')
    apply(workspace, change(workspace))
  })
  const controller = createRequestSpeechTidy({ getWorkspace: () => workspace, getConfig: () => settings,
    isLocked: () => locked, tidy, commit, onConfigure: configure })
  controller.refresh()
  return { controller, tidy, commit, configure, state: () => get(controller), workspace: () => workspace!,
    setWorkspace: (next: FeedbackWorkspaceView | null) => { workspace = next },
    setLocked: (value: boolean) => { locked = value }, setConfig: (value: TidyConfig | null) => { settings = value } }
}

describe('request-wide speech tidy', () => {
  it('sends one batch for notes and both answers, keeping typed text, metadata, and each original destination', async () => {
    const h = harness()
    expect(h.state().pendingCount).toBe(3)
    await h.controller.run()
    expect(h.tidy).toHaveBeenCalledTimes(1)
    expect(h.tidy.mock.calls[0][0].map(({ segmentId, text }) => ({ segmentId, text }))).toEqual([
      { segmentId: 'notes-1', text: 'Um spoken note' }, { segmentId: 'answer-1', text: 'Um first answer' }, { segmentId: 'answer-2', text: 'Um second answer' },
    ])
    expect(h.workspace().draft.body_markdown).toContain('Typed note')
    expect(h.workspace().draft.body_markdown).toContain('spoken note')
    expect(answers(h.workspace()).map((answer) => answer.value)).toEqual(['Typed q1\nfirst answer', 'Typed q2\nsecond answer'])
    expect(JSON.parse(h.workspace().draft.document_json!).opaque).toEqual({ keep: true })
    expect(h.state()).toMatchObject({ applied: 3, skipped: 0, pendingCount: 0, busy: false, canUndo: true })
  })

  it('keeps new input outside the in-flight batch and does not retarget old answers when another answer is edited', async () => {
    const h = harness(), pending = deferred<string[]>()
    h.tidy.mockReturnValueOnce(pending.promise)
    const flight = h.controller.run()
    appendSpeech(h.workspace(), 'q2', 'answer-3', 'Um next segment')
    edit(h.workspace(), (envelope) => { envelope.doc.content.push(paragraph('Typed while waiting')) })
    pending.resolve(['spoken note', 'first answer', 'second answer'])
    await flight
    expect(answers(h.workspace()).map((answer) => answer.value)).toEqual(['Typed q1\nfirst answer', 'Typed q2\nsecond answer\nUm next segment'])
    expect(h.workspace().draft.body_markdown).toContain('Typed while waiting')
    expect(h.state()).toMatchObject({ applied: 3, skipped: 0, pendingCount: 1 })
  })

  it.each(['changed', 'deleted'] as const)('skips a %s document segment while applying untouched answers', async (kind) => {
    const h = harness(), pending = deferred<string[]>()
    h.tidy.mockReturnValueOnce(pending.promise)
    const flight = h.controller.run()
    edit(h.workspace(), (envelope) => {
      if (kind === 'deleted') envelope.doc.content.pop()
      else envelope.doc.content[1].content[0].text = 'My corrected note'
    })
    pending.resolve(['MODEL NOTE', 'first answer', 'second answer'])
    await flight
    expect(h.workspace().draft.body_markdown).not.toContain('MODEL NOTE')
    expect(h.workspace().draft.body_markdown).toBe(kind === 'deleted' ? 'Typed note' : 'Typed note\n\nMy corrected note')
    expect(h.state()).toMatchObject({ applied: 2, skipped: 1 })
  })

  it.each(['edited', 'cleared', 'option'] as const)('preserves a custom answer %s during processing', async (kind) => {
    const h = harness(), pending = deferred<string[]>()
    h.tidy.mockReturnValueOnce(pending.promise)
    const flight = h.controller.run()
    edit(h.workspace(), (envelope) => {
      const list = envelope.workbenchState.answers
      if (kind === 'cleared') list.shift()
      else Object.assign(list[0], kind === 'option' ? { value: 'yes', label: 'Yes', wasCustom: false, index: 1 }
        : { value: 'Human replacement', label: 'Human replacement' })
    })
    pending.resolve(['spoken note', 'MODEL ANSWER', 'second answer'])
    await flight
    expect(answers(h.workspace()).find((answer) => answer.id === 'q1')?.value).toBe(kind === 'cleared' ? undefined : kind === 'option' ? 'yes' : 'Human replacement')
    expect(answers(h.workspace()).find((answer) => answer.id === 'q2')?.value).toBe('Typed q2\nsecond answer')
    expect(h.state()).toMatchObject({ applied: 2, skipped: 1 })
  })

  it('discards a result after switching requests even when returning to the original before it arrives', async () => {
    const h = harness(), original = h.workspace(), before = snapshotOf(original), pending = deferred<string[]>()
    h.tidy.mockReturnValueOnce(pending.promise)
    const flight = h.controller.run()
    h.setWorkspace(fixture('other')); h.controller.refresh()
    h.setWorkspace(original); h.controller.refresh()
    pending.resolve(['wrong note', 'wrong first', 'wrong second'])
    await flight
    expect(h.commit).not.toHaveBeenCalled()
    expect(snapshotOf(original)).toEqual(before)
    expect(h.state()).toMatchObject({ requestId: 'r', busy: false, applied: 0 })
  })

  it.each(['missing', 'unsupported', 'completed', 'cancelled', 'locked'] as const)('does not start for a %s request', async (kind) => {
    const h = harness()
    if (kind === 'missing') h.setWorkspace(null)
    else if (kind === 'unsupported') h.workspace().workbench!.version = 99
    else if (kind === 'locked') h.setLocked(true)
    else h.workspace().request.status = kind
    h.controller.refresh(undefined, 1)
    await h.controller.run()
    expect(h.tidy).not.toHaveBeenCalled()
    expect(h.commit).not.toHaveBeenCalled()
    expect(h.state().disabled).toBe(true)
  })

  it.each(['locked', 'closed', 'unsupported'] as const)('does not apply when the request becomes %s in flight', async (kind) => {
    const h = harness(), pending = deferred<string[]>()
    h.tidy.mockReturnValueOnce(pending.promise)
    const before = snapshotOf(h.workspace()), flight = h.controller.run()
    if (kind === 'locked') h.setLocked(true)
    else if (kind === 'closed') h.workspace().request.status = 'completed'
    else h.workspace().workbench!.version = 99
    pending.resolve(['wrong note', 'wrong first', 'wrong second'])
    await flight
    expect(snapshotOf(h.workspace())).toEqual(before)
    expect(h.commit).not.toHaveBeenCalled()
    expect(h.state().error).toContain('No text was replaced')
  })

  it.each(['failure', 'mismatched output'] as const)('keeps all original text after model %s', async (kind) => {
    const h = harness(), before = snapshotOf(h.workspace())
    if (kind === 'failure') h.tidy.mockRejectedValueOnce(new Error('Offline'))
    else h.tidy.mockResolvedValueOnce(['not enough'])
    await h.controller.run()
    expect(snapshotOf(h.workspace())).toEqual(before)
    expect(h.commit).not.toHaveBeenCalled()
    expect(h.state()).toMatchObject({ busy: false, pendingCount: 3, applied: 0, canUndo: false })
    expect(h.state().error).not.toBe('')
  })

  it('undo restores the complete pre-tidy draft, including provenance and opaque metadata', async () => {
    const h = harness(), before = snapshotOf(h.workspace())
    await h.controller.run()
    h.controller.undo()
    expect(snapshotOf(h.workspace())).toEqual(before)
    expect(h.state()).toMatchObject({ pendingCount: 3, canUndo: false, applied: 0 })
  })

  it.each(['document', 'markdown'] as const)('refuses undo after a later %s edit, even without a refresh', async (kind) => {
    const h = harness()
    await h.controller.run()
    if (kind === 'document') edit(h.workspace(), (envelope) => { envelope.doc.content.push(paragraph('Later input')) })
    else h.workspace().draft.body_markdown += '\nLater input'
    const edited = snapshotOf(h.workspace())
    h.controller.undo()
    expect(snapshotOf(h.workspace())).toEqual(edited)
    expect(h.state().error).toContain('Your edits have been kept')
    h.controller.refresh()
    expect(h.state().canUndo).toBe(false)
  })

  it('aggregates the automatic threshold across destinations and does not repeatedly retry an unchanged failed batch', async () => {
    const h = harness()
    h.tidy.mockRejectedValue(new Error('Offline'))
    h.controller.refresh(undefined, 4)
    expect(h.tidy).not.toHaveBeenCalled()
    h.controller.refresh(undefined, 3)
    await vi.waitFor(() => expect(h.state().busy).toBe(false))
    expect(h.tidy).toHaveBeenCalledTimes(1)
    for (let i = 0; i < 3; i++) h.controller.refresh(undefined, 3)
    await Promise.resolve()
    expect(h.tidy).toHaveBeenCalledTimes(1)
    appendSpeech(h.workspace(), 'q2', 'answer-3', 'New speech')
    h.controller.refresh(undefined, 3)
    await vi.waitFor(() => expect(h.state().busy).toBe(false))
    expect(h.tidy).toHaveBeenCalledTimes(2)
  })

  it('does not immediately redo an automatic tidy after undo, but processes newly added speech', async () => {
    const h = harness(), before = snapshotOf(h.workspace())
    h.controller.refresh(undefined, 3)
    await vi.waitFor(() => expect(h.state().busy).toBe(false))
    h.controller.undo()
    h.controller.refresh(undefined, 3)
    await Promise.resolve()
    expect(snapshotOf(h.workspace())).toEqual(before)
    expect(h.tidy).toHaveBeenCalledTimes(1)
    appendSpeech(h.workspace(), 'q1', 'answer-3', 'Um new speech')
    h.controller.refresh(undefined, 3)
    await vi.waitFor(() => expect(h.state().busy).toBe(false))
    expect(h.tidy).toHaveBeenCalledTimes(2)
    expect(answers(h.workspace())[0].value).toBe('Typed q1\nfirst answer\nnew speech')
  })

  it('requires a configured model and keeps concurrent button presses in one flight', async () => {
    const h = harness(), pending = deferred<string[]>()
    h.setConfig(null)
    h.controller.refresh(undefined, 3)
    expect(h.configure).not.toHaveBeenCalled()
    await h.controller.run()
    expect(h.configure).toHaveBeenCalledTimes(1)
    expect(h.tidy).not.toHaveBeenCalled()
    h.setConfig(config)
    h.tidy.mockReturnValueOnce(pending.promise)
    const flight = h.controller.run()
    await h.controller.run()
    expect(h.tidy).toHaveBeenCalledTimes(1)
    pending.resolve(['spoken note', 'first answer', 'second answer'])
    await flight
  })
})
