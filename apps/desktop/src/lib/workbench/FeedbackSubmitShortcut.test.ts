// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { FeedbackWorkspaceView } from '../feedback'
import { locale } from '../preferences'
import { canSubmitWorkbench } from '../workbenchState'
import FeedbackColumn from './FeedbackColumn.svelte'

let view: ReturnType<typeof mount> | undefined
const rangeRects = Object.getOwnPropertyDescriptor(Range.prototype, 'getClientRects')
const rangeBounds = Object.getOwnPropertyDescriptor(Range.prototype, 'getBoundingClientRect')

beforeEach(() => {
  locale.set('en')
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})

afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
  for (const [name, descriptor] of [['getClientRects', rangeRects], ['getBoundingClientRect', rangeBounds]] as const) {
    if (descriptor) Object.defineProperty(Range.prototype, name, descriptor)
    else Reflect.deleteProperty(Range.prototype, name)
  }
})

function workspace(): FeedbackWorkspaceView {
  return {
    request: {
      request_id: 'request-1', host_id: 'codex', host_session_id: 'session-1',
      source_hint: null, title: 'Review the work', what_happened: 'Need human feedback.',
      status: 'in_progress', resolution: null, allow_finish: false, final_summary: null,
      revision: 1, created_at: '2026-10-11T00:00:00Z', updated_at: '2026-10-11T00:00:00Z',
    },
    workbench: { type: 'ramble', version: 1, data: { actions: [{ id: 'try', instruction: 'Try the app.' }] } },
    actions: [], context_refs: [], request_attachments: [], attachments: [], feedback: null,
    draft: { document_json: null, body_markdown: 'Useful feedback.', saved_revision: 1, updated_at: '2026-10-11T00:00:00Z' },
  }
}

async function openFeedback(overrides: Partial<{
  workspace: FeedbackWorkspaceView, draftBody: string, canSubmit: boolean, locked: boolean,
  cooking: boolean, submitting: boolean, cancelling: boolean, approving: boolean,
  attachmentBusy: boolean, cookingEnabled: boolean,
}> = {}) {
  const state = writable({
    workspace: workspace(), draftBody: 'Useful feedback.', canSubmit: true, locked: false,
    cooking: false, submitting: false, cancelling: false, approving: false,
    attachmentBusy: false, cookingEnabled: false, ...overrides,
  })
  const props = fromStore(state)
  const onSubmit = vi.fn(() => state.update((current) => ({ ...current, submitting: true })))
  const onCookPreview = vi.fn()
  const onChange = vi.fn()
  view = mount(FeedbackColumn, { target: document.body, props: {
    get workspace() { return props.current.workspace },
    get draftBody() { return props.current.draftBody },
    get canSubmit() { return props.current.canSubmit },
    get locked() { return props.current.locked },
    get cooking() { return props.current.cooking },
    get submitting() { return props.current.submitting },
    get cancelling() { return props.current.cancelling },
    get approving() { return props.current.approving },
    get attachmentBusy() { return props.current.attachmentBusy },
    get cookingEnabled() { return props.current.cookingEnabled },
    formatTime: () => '10:00', onSubmit, onCookPreview, onChange,
  } })
  await vi.waitFor(() => expect(document.querySelector('.feedback-prose')).not.toBeNull())
  await tick()
  return { state, onSubmit, onCookPreview, onChange, editor: document.querySelector<HTMLElement>('.feedback-prose')! }
}

function key(target: HTMLElement, options: KeyboardEventInit = {}) {
  const event = new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', keyCode: 13, ctrlKey: true, bubbles: true, cancelable: true, ...options })
  target.dispatchEvent(event)
  return event
}

async function placeCaret(editor: HTMLElement) {
  editor.focus()
  const text = editor.querySelector('p')!.firstChild!
  window.getSelection()!.collapse(text, text.textContent!.length)
  document.dispatchEvent(new Event('selectionchange'))
  await tick()
}

describe('free feedback submission shortcut', () => {
  it.each(['ctrlKey', 'metaKey'] as const)('submits once with %s+Enter, ignoring held-key repeats and the subsequent busy state', async (modifier) => {
    const { editor, onSubmit } = await openFeedback()
    const before = editor.innerHTML
    expect(key(editor, { ctrlKey: false, [modifier]: true }).defaultPrevented).toBe(true)
    key(editor, { ctrlKey: false, [modifier]: true, repeat: true })
    await tick()
    key(editor, { ctrlKey: false, [modifier]: true })
    expect(onSubmit).toHaveBeenCalledTimes(1)
    expect(editor.innerHTML).toBe(before)
  })

  it.each(['locked', 'cooking', 'submitting', 'cancelling', 'approving', 'attachmentBusy'] as const)('does not submit while %s', async (flag) => {
    const { editor, onSubmit } = await openFeedback({ [flag]: true })
    const before = editor.innerHTML
    key(editor)
    expect(onSubmit).not.toHaveBeenCalled()
    expect(editor.innerHTML).toBe(before)
  })

  it('respects the existing empty-draft eligibility and legacy ramble requests', async () => {
    const request = workspace()
    delete request.workbench
    const { editor, onSubmit, onChange } = await openFeedback({ workspace: request, draftBody: '', canSubmit: canSubmitWorkbench(undefined, null, '') })
    const before = editor.innerHTML
    expect(key(editor).defaultPrevented).toBe(true)
    expect(onSubmit).not.toHaveBeenCalled()
    expect(onChange).not.toHaveBeenCalled()
    expect(editor.innerHTML).toBe(before)
    expect(document.querySelector('[title="Submit feedback · Ctrl/⌘+Enter"]')).not.toBeNull()
  })

  it.each(['completed', 'cancelled'] as const)('does not submit a %s request even with stale eligibility', async (status) => {
    const request = workspace()
    request.request.status = status
    const { editor, onSubmit } = await openFeedback({ workspace: request })
    key(editor)
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it.each([{ isComposing: true }, { keyCode: 229 }, { key: 'Process' }])('does not submit an IME key %j', async (options) => {
    const { editor, onSubmit } = await openFeedback()
    key(editor, options)
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('respects the editor composition state when the key omits the IME flags', async () => {
    const { editor, onSubmit } = await openFeedback()
    editor.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }))
    key(editor)
    expect(onSubmit).not.toHaveBeenCalled()
    editor.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }))
  })

  it('preserves Enter paragraphs and Shift+Enter line breaks', async () => {
    const { editor, onSubmit } = await openFeedback()
    await placeCaret(editor)
    key(editor, { ctrlKey: false })
    await tick()
    expect(editor.querySelectorAll('p')).toHaveLength(2)
    const afterParagraph = editor.innerHTML
    key(editor, { ctrlKey: false, shiftKey: true })
    await tick()
    expect(editor.innerHTML).not.toBe(afterParagraph)
    expect(editor.querySelectorAll('p')).toHaveLength(2)
    expect(editor.querySelectorAll('br').length).toBeGreaterThan(0)
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('leaves a typed workbench notes editor and other input targets alone', async () => {
    const request = workspace()
    request.workbench = { type: 'questions', version: 1, data: { questions: [] } }
    const { editor, onSubmit } = await openFeedback({ workspace: request })
    await placeCaret(editor)
    key(editor)
    await tick()
    expect(onSubmit).not.toHaveBeenCalled()
    expect(editor.querySelector('br')).not.toBeNull()
    expect(document.querySelector('[title*="Ctrl/⌘+Enter"]')).toBeNull()
  })

  it('does not register a global shortcut for inputs outside the feedback body', async () => {
    const { onSubmit } = await openFeedback()
    const input = document.createElement('input')
    document.querySelector('.feedback-column')!.append(input)
    expect(key(input).defaultPrevented).toBe(false)
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('uses the existing cook-and-submit action and localized button hint', async () => {
    const { editor, onSubmit, onCookPreview } = await openFeedback({ cookingEnabled: true })
    expect(document.querySelector('[title="Cook and submit · Ctrl/⌘+Enter"]')).not.toBeNull()
    locale.set('zh-CN')
    await tick()
    expect(document.querySelector('[title="Cooking 并提交 · Ctrl/⌘+Enter"]')).not.toBeNull()
    key(editor)
    expect(onSubmit).toHaveBeenCalledTimes(1)
    expect(onCookPreview).not.toHaveBeenCalled()
  })
})
