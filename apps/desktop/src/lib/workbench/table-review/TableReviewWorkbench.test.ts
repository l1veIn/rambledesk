// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { InputTarget } from '../../domain/inputTarget'
import type { TableReviewData } from '../../generated/feedback'
import { locale } from '../../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../../speech/voiceInputContext'
import { inputText, replaceInputText } from '../../../test/tiptap'
import { decodeFeedbackDraftEnvelope, snapshotFeedbackDraftMarkdown, updateFeedbackDraftState } from '../../feedbackDraftDocument'
import { applySpeechWriteback } from '../../speech/speechWriteback'
import { tableReviewDefinition } from '../definitions/table_review/definition'
import { repositoryGuideTable as data } from '../definitions/table_review/examples'
import { readTableReviewState, type TableReviewState } from '../definitions/table_review/state'
import { getWorkbenchTour } from '../onboarding/workbenchTours'
import TableReviewWorkbench from './TableReviewWorkbench.svelte'

const documentTarget: InputTarget = { requestId: 'table', requestTitle: 'Review', destination: { kind: 'document', action: null } }
let view: ReturnType<typeof mount> | undefined
beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => setTimeout(() => callback(0), 0))
  vi.stubGlobal('cancelAnimationFrame', (id: number) => clearTimeout(id))
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  Object.defineProperty(Element.prototype, 'scrollIntoView', { configurable: true, value: vi.fn() })
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined; document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals()
})
async function open(state: TableReviewState | null = null, sourceData = data, readOnly = false) {
  const source = writable({ state, data: sourceData, readOnly, disabled: false, onOpenExpanded: vi.fn() as (() => void) | undefined })
  const props = fromStore(source)
  const onChange = vi.fn((next: TableReviewState) => source.update((value) => ({ ...value, state: next })))
  const selectTarget = vi.fn(), start = vi.fn()
  const voiceState = writable<VoiceInputState>({ requestId: 'table', documentTarget, nextTarget: documentTarget, disabled: false, recording: false })
  view = mount(TableReviewWorkbench, { target: document.body, props: {
    get state() { return props.current.state }, get data() { return props.current.data }, get readOnly() { return props.current.readOnly },
    get disabled() { return props.current.disabled }, get onOpenExpanded() { return props.current.onOpenExpanded }, requestId: 'table', onChange,
  }, context: new Map<symbol, unknown>([[VOICE_INPUT_CONTEXT, { state: voiceState, selectTarget, start, stop: vi.fn() }]]) })
  await tick()
  return { source, props, onChange, voiceState, start, selectTarget }
}
const cell = (row: string, column: string) => document.querySelector<HTMLElement>(`[data-table-cell="${row}:${column}"]`)!
const button = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((node) => node.textContent?.trim() === label)!
const editor = () => document.querySelector<HTMLElement>('[data-table-comment-editor] [contenteditable]')!
async function key(node: HTMLElement, value: string) {
  node.dispatchEvent(new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true })); await tick()
}
async function editValue(value: string) {
  const input = document.querySelector<HTMLTextAreaElement>('[data-table-cell-editor]')!
  input.value = value; input.dispatchEvent(new InputEvent('input', { bubbles: true })); await tick()
  return input
}

describe('table review grid and shared draft', () => {
  it('shows all guide landmarks on a null draft without creating input', async () => {
    const app = await open(), snapshot = JSON.stringify(data)
    const tour = getWorkbenchTour(tableReviewDefinition, 'en')!
    for (const step of tour.steps.filter((step) => step.target !== '[data-feedback-actions]')) expect(document.querySelector(step.target), step.id).not.toBeNull()
    expect(document.querySelector('[role="grid"]')?.getAttribute('aria-colcount')).toBe('5')
    expect(document.querySelectorAll('thead th')).toHaveLength(5)
    expect(app.props.current.state).toBeNull()
    expect(app.onChange).not.toHaveBeenCalled()
    expect(JSON.stringify(data)).toBe(snapshot)
  })
  it('moves with arrows, cancels F2 editing, saves an empty suggestion with Enter and keeps original values', async () => {
    const app = await open(), snapshot = JSON.stringify(data)
    const first = cell('ramble', 'workbench'); first.focus()
    await key(first, 'ArrowRight')
    await vi.waitFor(() => expect(document.activeElement).toBe(cell('ramble', 'guide_version')))
    await key(cell('ramble', 'guide_version'), 'F2')
    expect(document.querySelector<HTMLTextAreaElement>('[data-table-cell-editor]')?.value).toBe('1')
    let input = await editValue('Cancelled')
    await key(input, 'Escape')
    expect(app.onChange).not.toHaveBeenCalled()
    expect(document.querySelector('[data-table-cell-editor]')).toBeNull()
    await key(cell('ramble', 'guide_version'), 'Enter')
    input = await editValue(''); await key(input, 'Enter')
    expect(app.props.current.state?.changes).toEqual([{ row_id: 'ramble', column_id: 'guide_version', value: '' }])
    expect(document.querySelector('[data-table-original]')?.textContent).toBe('1')
    expect(cell('ramble', 'guide_version').classList.contains('suggested')).toBe(true)
    expect(JSON.stringify(data)).toBe(snapshot)
    button('Remove suggestion').click(); await tick()
    expect(app.props.current.state?.changes).toEqual([])
    expect(cell('ramble', 'guide_version').textContent?.trim()).toBe('1')
  })
  it('creates a first voice comment in the host draft before recording and routes shared writes by stable identity', async () => {
    const app = await open()
    app.start.mockImplementationOnce((target: InputTarget) => {
      expect(app.props.current.state?.comments).toHaveLength(1)
      expect(target.destination).toMatchObject({ field: 'comment_body', entityId: app.props.current.state?.comments[0].id,
        sourceVersion: data.source_version, workbenchType: 'table_review', version: 1 })
    })
    cell('ramble', 'steps').click(); await tick()
    document.querySelector<HTMLButtonElement>('[data-table-voice-comment]')!.click(); await tick()
    await vi.waitFor(() => expect(app.start).toHaveBeenCalledOnce())
    replaceInputText(editor(), 'Check whether the guide still has five steps.'); await tick()
    expect(app.props.current.state?.comments[0]).toMatchObject({ row_id: 'ramble', column_id: 'steps', body: 'Check whether the guide still has five steps.' })
    editor().dispatchEvent(new FocusEvent('focus'))
    expect(app.selectTarget.mock.lastCall?.[0].destination).toMatchObject({ field: 'comment_body', entityId: app.props.current.state?.comments[0].id })
    document.querySelector<HTMLButtonElement>('[data-table-add-comment]')!.click(); await tick()
    expect(app.props.current.state?.comments).toHaveLength(1)
    button('Delete comment').click(); await tick()
    expect(app.props.current.state?.comments).toEqual([])
  })
  it.each([null, 'Saved suggestion'])('preserves in-progress text when double-clicking an editor with prior value %j', async (previous) => {
    const state: TableReviewState | null = previous === null ? null : {
      type: 'table_review', changes: [{ row_id: 'ramble', column_id: 'workbench', value: previous }], comments: [],
    }
    const app = await open(state)
    await key(cell('ramble', 'workbench'), 'F2')
    const input = await editValue('New user suggestion')
    for (const [type, detail] of [['click', 1], ['click', 2], ['dblclick', 2]] as const) {
      input.dispatchEvent(new MouseEvent(type, { detail, bubbles: true, cancelable: true }))
      await tick()
    }

    expect(document.querySelector<HTMLTextAreaElement>('[data-table-cell-editor]')?.value).toBe('New user suggestion')
    expect(app.onChange).not.toHaveBeenCalled()
    await key(input, 'Enter')
    expect(app.props.current.state?.changes).toEqual([{ row_id: 'ramble', column_id: 'workbench', value: 'New user suggestion' }])
  })
  it('seeds the first voice suggestion on the selected cell without altering its original value', async () => {
    const app = await open()
    app.start.mockImplementation((target: InputTarget) => {
      expect(app.props.current.state?.changes).toEqual([{ row_id: 'ramble', column_id: 'steps', value: '' }])
      expect(target.destination).toMatchObject({ field: 'change_value', entityId: 'ramble:steps' })
      const snapshot = updateFeedbackDraftState(snapshotFeedbackDraftMarkdown('Keep unrelated notes'), app.props.current.state)
      const written = applySpeechWriteback({ request: { request_id: 'table', status: 'in_progress' },
        workbench: { type: 'table_review', version: 1, data }, draft: { document_json: snapshot.documentJson, body_markdown: snapshot.bodyMarkdown } },
      { ...target, id: 'first-voice-suggestion', text: '6' })
      const next = readTableReviewState(decodeFeedbackDraftEnvelope(written.documentJson)!.workbenchState)!
      expect(next.changes[0].value).toBe('6')
      expect(written.bodyMarkdown).toBe(snapshot.bodyMarkdown)
      app.source.update((value) => ({ ...value, state: next }))
    })
    cell('ramble', 'steps').click(); await tick()
    document.querySelector<HTMLButtonElement>('[data-table-voice-suggestion]')!.click(); await tick()
    await vi.waitFor(() => expect(app.start).toHaveBeenCalledOnce())
    const field = document.querySelector<HTMLTextAreaElement>('[data-table-suggestion-value]')!
    expect(field.value).toBe('6')
    expect(document.querySelector('[data-table-original]')?.textContent).toBe('5')
    expect(app.props.current.state?.changes[0].value).toBe('6')
    expect(tableReviewDefinition.complete({ type: 'table_review', version: 1, data }, app.props.current.state)).toBe(true)
    field.value = '[notes](attachment://a)'; field.dispatchEvent(new InputEvent('input', { bubbles: true })); await tick()
    expect(app.props.current.state?.changes[0].value).toBe('[notes](attachment://a)')
    app.start.mockImplementation((target: InputTarget) => {
      expect(app.props.current.state?.changes[0].value).toBe('[notes](attachment://a)')
      expect(target.destination).toMatchObject({ field: 'change_value', entityId: 'ramble:steps' })
    })
    document.querySelector<HTMLButtonElement>('[data-table-voice-suggestion]')!.click(); await tick()
    await vi.waitFor(() => expect(app.start).toHaveBeenCalledTimes(2))
  })
  it('reveals restored comments from speech, retains read-only history and bounds large table rendering', async () => {
    const large: TableReviewData = { ...data, columns: Array.from({ length: 20 }, (_, index) => ({ id: `c-${index}`, label: `Column ${index + 1}` })),
      rows: Array.from({ length: 1000 }, (_, index) => ({ id: `r-${index}`, cells: Array.from({ length: 20 }, (_, column) => `${index},${column}`) })) }
    const saved: TableReviewState = { type: 'table_review', changes: [{ row_id: 'r-900', column_id: 'c-19', value: 'Corrected' }],
      comments: [{ id: 'late', row_id: 'r-900', column_id: 'c-19', body: 'Saved comment' }] }
    const app = await open(saved, large, true)
    expect(document.querySelectorAll('[data-table-cell]').length).toBeLessThan(2000)
    expect(cell('r-900', 'c-19')).toBeNull()
    app.voiceState.update((value) => ({ ...value, revealSequence: 1, revealTarget: { ...documentTarget, destination: {
      kind: 'workbench_field', workbenchType: 'table_review', version: 1, field: 'comment_body', entityId: 'late', sourceVersion: data.source_version, label: 'T901',
    } } }))
    await tick()
    await vi.waitFor(() => expect(cell('r-900', 'c-19')?.getAttribute('aria-selected')).toBe('true'))
    expect(inputText(editor())).toBe('Saved comment')
    expect(editor().getAttribute('contenteditable')).toBe('false')
    expect(document.querySelector('[data-table-edit]')).toBeNull()
    expect(button('Delete comment')).toBeUndefined()
    expect(button('Remove suggestion')).toBeUndefined()
    await key(cell('r-900', 'c-19'), 'F2')
    expect(document.querySelector('[data-table-cell-editor]')).toBeNull()
    expect(app.onChange).not.toHaveBeenCalled()
  })
  it('saves in-progress editing before opening the shared expanded draft and resets selection only on source replacement', async () => {
    const app = await open()
    cell('ramble', 'steps').click(); await tick()
    await key(cell('ramble', 'steps'), 'F2'); await editValue('6')
    button('Full screen review').click(); await tick()
    expect(app.props.current.state?.changes[0]).toMatchObject({ row_id: 'ramble', column_id: 'steps', value: '6' })
    await vi.waitFor(() => expect(app.props.current.onOpenExpanded).toHaveBeenCalledOnce())
    const calls = app.onChange.mock.calls.length
    app.source.update((value) => ({ ...value, data: { ...data, source_version: 'next' }, onOpenExpanded: undefined, disabled: true })); await tick()
    expect(document.querySelector('[data-table-selected]')?.textContent).toContain('A1')
    expect(button('Full screen review')).toBeUndefined()
    expect(document.querySelector('[data-table-add-comment]')).toBeNull()
    expect(app.onChange.mock.calls.length).toBe(calls)
  })
})
