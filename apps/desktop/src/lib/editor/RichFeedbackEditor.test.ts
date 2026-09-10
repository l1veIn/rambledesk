// @vitest-environment jsdom
import type { JSONContent } from '@tiptap/core'
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { locale } from '../preferences'
import RichFeedbackEditor from './RichFeedbackEditor.svelte'

let view: ReturnType<typeof mount> | undefined
const rangeRects = Object.getOwnPropertyDescriptor(Range.prototype, 'getClientRects')
const rangeBounds = Object.getOwnPropertyDescriptor(Range.prototype, 'getBoundingClientRect')

beforeEach(() => {
  locale.set('en')
  // Real ProseMirror focus/history commands need geometry, which jsdom does not lay out.
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

function button(label: string) {
  const result = document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)
  expect(result).not.toBeNull()
  return result!
}

async function openEditor(initialDocument: JSONContent = { type: 'doc', content: [{ type: 'paragraph' }] }) {
  const state = writable({ document: initialDocument, editorEpoch: 0, disabled: false })
  const props = fromStore(state)
  const onChange = vi.fn<(snapshot: FeedbackDraftSnapshot) => void>()
  view = mount(RichFeedbackEditor, { target: document.body, props: {
    get document() { return props.current.document },
    get editorEpoch() { return props.current.editorEpoch },
    get disabled() { return props.current.disabled },
    onChange,
  } })
  await vi.waitFor(() => expect(document.querySelector('.feedback-prose')).not.toBeNull())
  await new Promise((resolve) => setTimeout(resolve, 0))
  await tick()
  onChange.mockClear()
  return { state, onChange, editor: document.querySelector<HTMLElement>('.feedback-prose')! }
}

async function typeText(editor: HTMLElement, text: string) {
  // Reproduce the browser's contenteditable mutation; the real ProseMirror observer
  // converts it into a transaction and emits the component's ordinary onChange.
  editor.focus()
  const paragraph = editor.querySelector('p')!
  paragraph.textContent = text
  window.getSelection()!.collapse(paragraph.firstChild, text.length)
  editor.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }))
  await tick()
}

async function moveCaret(editor: HTMLElement, node: Node) {
  editor.focus()
  window.getSelection()!.collapse(node, 1)
  document.dispatchEvent(new Event('selectionchange'))
  await tick()
}

describe('the real feedback editor toolbar', () => {
  it('enables Undo after input and reflects document/history changes through Undo and Redo clicks', async () => {
    const { editor, onChange } = await openEditor()
    expect(button('Undo').disabled).toBe(true)
    expect(button('Redo').disabled).toBe(true)

    await typeText(editor, 'A useful observation')
    await vi.waitFor(() => expect(onChange.mock.lastCall?.[0].bodyMarkdown).toBe('A useful observation'))
    expect(button('Undo').disabled).toBe(false)
    button('Undo').click()
    await vi.waitFor(() => expect(editor.textContent).toBe(''))
    expect(onChange.mock.lastCall?.[0].bodyMarkdown).toBe('')
    expect(button('Undo').disabled).toBe(true)
    expect(button('Redo').disabled).toBe(false)

    button('Redo').click()
    await vi.waitFor(() => expect(editor.textContent).toBe('A useful observation'))
    expect(onChange.mock.lastCall?.[0].bodyMarkdown).toBe('A useful observation')
    expect(button('Undo').disabled).toBe(false)
    expect(button('Redo').disabled).toBe(true)
  })

  it('updates active formatting for selection-only and stored-mark transactions without reporting a document edit', async () => {
    const { editor, onChange } = await openEditor({ type: 'doc', content: [
      { type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: 'Marked heading', marks: [{ type: 'bold' }] }] },
      { type: 'paragraph', content: [{ type: 'text', text: 'Plain paragraph' }] },
    ] })
    onChange.mockClear()
    await moveCaret(editor, editor.querySelector('h2 strong')!.firstChild!)
    await vi.waitFor(() => expect(button('Bold').getAttribute('aria-pressed')).toBe('true'))
    expect(button('Heading 2').getAttribute('aria-pressed')).toBe('true')

    await moveCaret(editor, editor.querySelector('p')!.firstChild!)
    await vi.waitFor(() => expect(button('Bold').getAttribute('aria-pressed')).toBe('false'))
    expect(button('Heading 2').getAttribute('aria-pressed')).toBe('false')
    button('Italic').click()
    await vi.waitFor(() => expect(button('Italic').getAttribute('aria-pressed')).toBe('true'))
    button('Italic').click()
    await vi.waitFor(() => expect(button('Italic').getAttribute('aria-pressed')).toBe('false'))
    expect(onChange).not.toHaveBeenCalled()

    for (const label of ['Bullet list', 'Quote']) {
      button(label).click()
      await vi.waitFor(() => expect(button(label).getAttribute('aria-pressed')).toBe('true'))
      button(label).click()
      await vi.waitFor(() => expect(button(label).getAttribute('aria-pressed')).toBe('false'))
    }
  })

  it('clears history availability when a new document epoch resets editor state', async () => {
    const { editor, state, onChange } = await openEditor()
    await typeText(editor, 'Previous request')
    await vi.waitFor(() => expect(onChange).toHaveBeenCalled())
    expect(button('Undo').disabled).toBe(false)

    state.set({ editorEpoch: 1, disabled: false, document: { type: 'doc', content: [
      { type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: 'Next request' }] },
    ] } })
    await vi.waitFor(() => expect(editor.textContent).toBe('Next request'))
    expect(button('Undo').disabled).toBe(true)
    expect(button('Redo').disabled).toBe(true)
    expect(button('Heading 2').getAttribute('aria-pressed')).toBe('true')
    expect(onChange).toHaveBeenCalledTimes(1)
  })
})
