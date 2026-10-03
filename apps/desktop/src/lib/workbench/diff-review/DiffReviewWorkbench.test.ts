// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { InputTarget } from '../../domain/inputTarget'
import type { DiffReviewData } from '../../generated/feedback'
import { locale } from '../../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../../speech/voiceInputContext'
import { inputText, replaceInputText } from '../../../test/tiptap'
import { diffReviewDefinition } from '../definitions/diff_review/definition'
import type { DiffReviewState } from '../definitions/diff_review/state'
import DiffReviewWorkbench from './DiffReviewWorkbench.svelte'
import { getWorkbenchTour } from '../onboarding/workbenchTours'
import { getWorkbenchDefinition } from '../definitions/registry'

const originalData = diffReviewDefinition.examples![0].spec.data as DiffReviewData
const saved: DiffReviewState = { type: 'diff_review', comments: [
  { id: 'saved', anchor: { file_id: 'quickstart', hunk_index: 0, side: 'new', start_line: 2, end_line: 3 }, body: 'Saved comment' },
] }
const largeData: DiffReviewData = { ...originalData, files: [{ id: 'large', old_path: '/dev/null', new_path: 'large.ts', diff: '@@ -0,0 +1,1500 @@\n' + Array.from({ length: 1500 }, (_, index) => `+line ${index + 1}`).join('\n') }] }
const documentTarget: InputTarget = { requestId: 'review', requestTitle: 'Review', destination: { kind: 'document', action: null } }
let view: ReturnType<typeof mount> | undefined
beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => setTimeout(() => callback(0), 0))
  vi.stubGlobal('cancelAnimationFrame', (id: number) => clearTimeout(id))
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  Object.defineProperty(Element.prototype, 'scrollIntoView', { configurable: true, value: vi.fn() })
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined; document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals()
})
async function open(state: DiffReviewState | null = null, data = originalData, readOnly = false) {
  const source = writable({ state, data, disabled: false, readOnly, onOpenExpanded: vi.fn() as (() => void) | undefined })
  const props = fromStore(source)
  const onChange = vi.fn((next: DiffReviewState) => source.update((value) => ({ ...value, state: next })))
  const selectTarget = vi.fn()
  const voiceState = writable<VoiceInputState>({ requestId: 'review', documentTarget, nextTarget: documentTarget, disabled: false, recording: false })
  view = mount(DiffReviewWorkbench, { target: document.body, props: {
    get state() { return props.current.state }, get data() { return props.current.data }, get disabled() { return props.current.disabled },
    get readOnly() { return props.current.readOnly }, get onOpenExpanded() { return props.current.onOpenExpanded }, requestId: 'review', onChange,
  }, context: new Map<symbol, unknown>([[VOICE_INPUT_CONTEXT, { state: voiceState, selectTarget, start: vi.fn(), stop: vi.fn() }]]) })
  await tick()
  return { source, props, onChange, selectTarget, voiceState }
}
const line = (side: 'old' | 'new', number: number, hunk = 0) => document.querySelector<HTMLButtonElement>(`[data-diff-hunk="${hunk}"][data-diff-side="${side}"][data-diff-line="${number}"]`)!
const button = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((node) => node.textContent?.trim() === label)!
const editor = () => document.querySelector<HTMLElement>('[data-diff-comment-editor] [contenteditable]')!
async function key(node: HTMLElement, value: string, shiftKey = false) {
  node.dispatchEvent(new KeyboardEvent('keydown', { key: value, shiftKey, bubbles: true, cancelable: true })); await tick()
}

describe('diff review interactions', () => {
  it('changes display layout and filters files without rewriting feedback or immutable coordinates', async () => {
    const app = await open(saved)
    const original = JSON.stringify(originalData)
    button('Split').click(); await tick()
    expect(document.querySelector('[data-diff-source]')?.getAttribute('data-diff-mode')).toBe('split')
    expect(line('old', 2)).not.toBeNull()
    expect(line('new', 2)).not.toBeNull()
    expect(app.onChange).not.toHaveBeenCalled()
    const search = document.querySelector<HTMLInputElement>('input[type="search"]')!
    search.value = 'quickstart'; search.dispatchEvent(new Event('input', { bubbles: true })); await tick()
    expect(document.querySelectorAll('[data-diff-file]')).toHaveLength(1)
    document.querySelector<HTMLButtonElement>('[data-diff-comment="saved"]')!.click(); await tick()
    expect(line('new', 2).getAttribute('aria-pressed')).toBe('true')
    expect(line('new', 3).getAttribute('aria-pressed')).toBe('true')
    expect(document.querySelector('[data-diff-source]')?.contains(editor())).toBe(true)
    button('Unified').click(); await tick()
    expect(inputText(editor())).toBe('Saved comment')
    expect(app.props.current.state).toEqual(saved)
    expect(app.onChange).not.toHaveBeenCalled()
    expect(JSON.stringify(originalData)).toBe(original)
  })
  it('uses the line gutter to create a precisely anchored inline comment in split mode', async () => {
    const app = await open()
    button('Split').click(); await tick()
    document.querySelector<HTMLButtonElement>('[aria-label="Comment on Old line 2"]')!.click(); await tick()
    expect(app.props.current.state?.comments[0].anchor).toMatchObject({ file_id: 'errors', side: 'old', start_line: 2, end_line: 2 })
    expect(document.querySelector('[data-diff-source]')?.contains(editor())).toBe(true)
    replaceInputText(editor(), 'Keep this validation.'); await tick()
    button('Unified').click(); await tick()
    expect(inputText(editor())).toBe('Keep this validation.')
    expect(line('old', 2).getAttribute('aria-pressed')).toBe('true')
  })
  it('exposes guide landmarks before selecting lines without creating comments or changing the source', async () => {
    const original = JSON.stringify(originalData)
    const app = await open()
    const tour = getWorkbenchTour(getWorkbenchDefinition('diff_review'), 'en')!
    for (const step of tour.steps.filter((step) => step.target !== '[data-feedback-actions]')) {
      expect(document.querySelector(step.target), step.id).not.toBeNull()
    }
    expect(document.querySelectorAll('[data-diff-comment]')).toHaveLength(0)
    expect(app.props.current.state).toBeNull()
    expect(app.onChange).not.toHaveBeenCalled()
    expect(JSON.stringify(originalData)).toBe(original)
  })
  it('selects a new-side keyboard range, autosaves through the shared field and reopens the same anchor', async () => {
    const app = await open()
    const original = JSON.stringify(originalData)
    line('new', 2).focus()
    await key(line('new', 2), 'ArrowDown', true)
    expect(document.activeElement).toBe(line('new', 3))
    expect(line('new', 2).getAttribute('aria-pressed')).toBe('true')
    await key(line('new', 3), 'c')
    expect(app.props.current.state?.comments[0].anchor).toEqual({ file_id: 'errors', hunk_index: 0, side: 'new', start_line: 2, end_line: 3 })
    expect(app.props.current.state?.comments[0].body).toBe('')
    expect(diffReviewDefinition.complete({ ...diffReviewDefinition.examples![0].spec }, app.props.current.state)).toBe(false)
    expect(document.querySelector('textarea')).toBeNull()
    replaceInputText(editor(), 'Keep the actionable hint.'); await tick()
    expect(app.props.current.state?.comments[0].body).toBe('Keep the actionable hint.')
    editor().dispatchEvent(new FocusEvent('focus'))
    expect(app.selectTarget.mock.lastCall?.[0].destination).toMatchObject({ kind: 'workbench_field', workbenchType: 'diff_review',
      version: 1, field: 'comment_body', entityId: app.props.current.state?.comments[0].id, sourceVersion: originalData.source_version })
    document.querySelector<HTMLButtonElement>('[aria-label="Close comment"]')!.click(); await tick()
    button('Add comment').click(); await tick()
    expect(app.props.current.state?.comments).toHaveLength(1)
    expect(inputText(editor())).toBe('Keep the actionable hint.')
    expect(JSON.stringify(originalData)).toBe(original)
  })
  it('keeps old/new selections distinct, supports hunk comments and deletes drafts', async () => {
    const app = await open()
    line('old', 2).click(); await tick()
    button('Add comment').click(); await tick()
    expect(app.props.current.state?.comments[0].anchor.side).toBe('old')
    document.querySelector<HTMLButtonElement>('[data-diff-hunk-comment="0"]')!.click(); await tick()
    expect(app.props.current.state?.comments[1].anchor).toMatchObject({ side: 'new', start_line: null, end_line: null })
    document.querySelector<HTMLButtonElement>('[aria-label="Delete comment"]')!.click(); await tick()
    expect(app.props.current.state?.comments).toHaveLength(1)
    expect(document.querySelector('[data-diff-comment-editor]')).toBeNull()
    await vi.waitFor(() => expect(document.activeElement).toBe(document.querySelector('[data-diff-add-comment]')))
  })
  it('restores comments across files and reveals the durable generic speech destination', async () => {
    const app = await open(saved)
    expect(app.onChange).not.toHaveBeenCalled()
    app.voiceState.update((state) => ({ ...state, revealSequence: 1, revealTarget: { ...documentTarget, destination: {
      kind: 'workbench_field', workbenchType: 'diff_review', version: 1, field: 'comment_body', entityId: 'saved', sourceVersion: originalData.source_version, label: 'Saved',
    } } }))
    await tick()
    expect(document.querySelector('[data-diff-file="quickstart"]')?.getAttribute('aria-current')).toBe('page')
    expect(line('new', 2).getAttribute('aria-pressed')).toBe('true')
    expect(line('new', 3).getAttribute('aria-pressed')).toBe('true')
    expect(inputText(editor())).toBe('Saved comment')
    expect(app.onChange).not.toHaveBeenCalled()
    document.querySelector<HTMLButtonElement>('[data-diff-file="errors"]')!.click(); await tick()
    expect(document.querySelector('[data-diff-comment-editor]')).toBeNull()
    document.querySelector<HTMLButtonElement>('[data-diff-comment="saved"]')!.click(); await tick()
    expect(inputText(editor())).toBe('Saved comment')
  })
  it('renders read-only historical comments and prevents create/edit/delete while retaining navigation', async () => {
    const app = await open(saved, originalData, true)
    document.querySelector<HTMLButtonElement>('[data-diff-comment="saved"]')!.click(); await tick()
    expect(inputText(editor())).toBe('Saved comment')
    expect(editor().getAttribute('contenteditable')).toBe('false')
    expect(document.querySelector('[data-diff-add-comment]')).toBeNull()
    expect(document.querySelector('[data-diff-hunk-comment]')).toBeNull()
    expect(document.querySelector('[aria-label="Delete comment"]')).toBeNull()
    await key(line('new', 2), 'c')
    expect(app.onChange).not.toHaveBeenCalled()
    app.source.update((value) => ({ ...value, readOnly: false, disabled: true })); await tick()
    expect(document.querySelector('[data-diff-add-comment]')).toBeNull()
    expect(app.onChange).not.toHaveBeenCalled()
  })
  // Three full jsdom windows (1000 → 500 → 1000) test behavior, not a rendering-time SLA.
  it('bounds large diff rendering when jumping to historical anchors and returning to the previous window', async () => {
    const history: DiffReviewState = { type: 'diff_review', comments: [{ id: 'late', body: 'Later line', anchor: { file_id: 'large', hunk_index: 0, side: 'new', start_line: 1400, end_line: 1401 } }] }
    const app = await open(history, largeData)
    expect(document.querySelectorAll('[data-diff-text]')).toHaveLength(1000)
    expect(line('new', 1400)).toBeNull()
    document.querySelector<HTMLButtonElement>('[data-diff-comment="late"]')!.click(); await tick()
    expect(document.querySelectorAll('[data-diff-text]')).toHaveLength(500)
    expect(line('new', 1400).getAttribute('aria-pressed')).toBe('true')
    expect(line('new', 1401).getAttribute('aria-pressed')).toBe('true')
    expect(app.onChange).not.toHaveBeenCalled()
    button('Previous lines').click(); await tick()
    expect(document.querySelectorAll('[data-diff-text]')).toHaveLength(1000)
    expect(line('new', 1000)).not.toBeNull()
    expect(line('new', 1400)).toBeNull()
    expect(app.props.current.state).toEqual(history)
    expect(app.onChange).not.toHaveBeenCalled()
  }, 10_000)
  it('moves a large diff keyboard range across the source window boundary', async () => {
    const app = await open(null, largeData)
    expect(document.querySelectorAll('[data-diff-text]')).toHaveLength(1000)
    expect(line('new', 1001)).toBeNull()
    line('new', 1000).focus()
    await key(line('new', 1000), 'ArrowDown', true)
    expect(document.activeElement).toBe(line('new', 1001))
    expect(document.querySelectorAll('[data-diff-text]')).toHaveLength(500)
    expect(line('new', 1001).getAttribute('aria-pressed')).toBe('true')
    expect(app.onChange).not.toHaveBeenCalled()
  })
  it('gates expanded mode through the host and resets transient selection on source replacement', async () => {
    const app = await open()
    button('Full screen review').click()
    expect(app.props.current.onOpenExpanded).toHaveBeenCalledOnce()
    line('new', 2).click(); await tick()
    app.source.update((value) => ({ ...value, onOpenExpanded: undefined, data: { ...value.data, source_version: 'next-source' } })); await tick()
    expect(button('Full screen review')).toBeUndefined()
    expect(document.querySelector('[data-diff-add-comment]')?.hasAttribute('disabled')).toBe(true)
    expect(line('new', 2).getAttribute('aria-pressed')).toBe('false')
    expect(app.onChange).not.toHaveBeenCalled()
  })
})
