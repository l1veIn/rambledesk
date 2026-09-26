// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { DocumentReviewData, ReviewAnnotation } from '../../generated/feedback'
import { locale } from '../../preferences'
import { emptyReviewState, validateReviewState, type DocumentReviewState } from './reviewModel'
import DocumentReviewWorkbench from './DocumentReviewWorkbench.svelte'
import { fromStore, writable } from 'svelte/store'
import { VOICE_INPUT_CONTEXT, type VoiceInputContext, type VoiceInputState } from '../../speech/voiceInputContext'
import type { SpeechTarget } from '../../speech/speechDraftQueue'
import { inputText, replaceInputText, tiptapEditor } from '../../../test/tiptap'

const data: DocumentReviewData = { title: 'Opening speech', source_version: 'v1', paragraphs: [
  { id: 'opening', label: 'Opening', text: 'Hello 😀 everyone. This is our promise.' },
  { id: 'closing', label: 'Closing', text: 'Thank you for listening.' },
] }
let view: ReturnType<typeof mount> | undefined
let latest: DocumentReviewState
const button = (label: string) => Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.trim() === label)!
const byLabel = <T extends HTMLElement>(label: string) => document.querySelector<T>(`[aria-label="${label}"]`)!
const deleteDialog = () => document.querySelector<HTMLElement>('[data-review-delete-dialog]')
beforeEach(() => {
  locale.set('en'); latest = emptyReviewState()
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  Element.prototype.getAnimations = (() => []) as never
  HTMLElement.prototype.scrollIntoView = vi.fn()
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.getSelection()?.removeAllRanges(); document.body.replaceChildren(); vi.unstubAllGlobals() })
function open(state = emptyReviewState(), disabled = false, voice?: VoiceInputContext) {
  const instance = mount(DocumentReviewWorkbench, { target: document.body, context: voice ? new Map([[VOICE_INPUT_CONTEXT, voice]]) : undefined, props: { data, state, disabled, onChange: (value) => latest = value } })
  view = instance
  return instance
}
const documentTarget: SpeechTarget = { requestId: 'request-1', requestTitle: 'Review draft', destination: { kind: 'document', action: null } }
function voiceInput() {
  const state = writable<VoiceInputState>({ requestId: documentTarget.requestId, documentTarget, nextTarget: documentTarget, recording: false, disabled: false })
  return { state, selectTarget: vi.fn(), start: vi.fn(), stop: vi.fn() }
}

describe('document review interaction', () => {
  it('keeps the source and guide landmarks in one view without view-switching tabs', () => {
    open()
    expect(document.querySelector('[role="tablist"]')).toBeNull()
    expect(document.querySelector('[role="tab"]')).toBeNull()
    expect(document.querySelector('[role="region"][aria-label="Original"]')?.querySelectorAll('[data-review-text]')).toHaveLength(data.paragraphs.length)
    expect(document.querySelector('[data-tour="review-comment"]')).not.toBeNull()
    expect(document.querySelector('[data-tour="review-delete"]')).not.toBeNull()
    expect(document.querySelector('[data-tour="review-verdict"]')).not.toBeNull()
    expect(latest).toEqual(emptyReviewState())
  })

  it('creates a spoken paragraph comment and selects the stable annotation field before recording', async () => {
    const voice = voiceInput()
    open(emptyReviewState(), false, voice)
    byLabel<HTMLButtonElement>('Speak comment on paragraph 1').click()
    await vi.waitFor(() => expect(voice.start).toHaveBeenCalledOnce())
    expect(voice.start).toHaveBeenCalledWith({ ...documentTarget, destination: {
      kind: 'review_annotation', annotationId: latest.annotations[0].id, field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening',
    } })
    expect(latest.annotations[0]).toMatchObject({ paragraph_id: 'opening', start: null, end: null, quote: null, body: '' })
    expect(latest.annotations[0]).not.toHaveProperty('status')
  })

  it.each(['body', 'replacement'] as const)('field focus selects the next destination and a reveal reopens the collapsed %s field', async (field) => {
    const voice = voiceInput()
    const annotation = { id: 'note-1', paragraph_id: 'opening', start: null, end: null, quote: null, kind: 'suggestion' as const, body: 'Comment', replacement: 'Hello' }
    open({ ...emptyReviewState(), annotations: [annotation] }, false, voice)
    document.querySelector<HTMLButtonElement>('[data-review-note-preview="note-1"]')!.click()
    await vi.waitFor(() => expect(byLabel<HTMLElement>('Suggested wording')).not.toBeNull())
    expect(document.querySelectorAll('[data-input-toolbar]')).toHaveLength(2)
    expect(document.querySelectorAll('[aria-label="Capture"]')).toHaveLength(2)
    byLabel<HTMLElement>(field === 'body' ? 'Your comment' : 'Suggested wording').focus()
    const target: SpeechTarget = { ...documentTarget, destination: { kind: 'review_annotation', annotationId: 'note-1', field, sourceVersion: 'v1', paragraphLabel: 'Opening' } }
    expect(voice.selectTarget).toHaveBeenLastCalledWith(target)
    expect(voice.start).not.toHaveBeenCalled()
    byLabel<HTMLButtonElement>('Collapse comment').click()
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    voice.state.update((state) => ({ ...state, revealTarget: target, revealSequence: 1 }))
    await vi.waitFor(() => expect(document.querySelector('#review-original')).not.toBeNull())
    await vi.waitFor(() => expect(document.activeElement?.getAttribute('data-review-field')).toBe(field))
    expect(inputText(byLabel('Your comment'))).toBe(annotation.body)
    expect(inputText(byLabel('Suggested wording'))).toBe(annotation.replacement)
    expect(voice.start).not.toHaveBeenCalled()
  })

  it('clears an expired source-text selection', async () => {
    open()
    const paragraph = document.querySelector<HTMLElement>('[data-review-text="opening"]')!
    const range = document.createRange()
    range.selectNodeContents(paragraph)
    document.getSelection()!.addRange(range)
    document.dispatchEvent(new Event('selectionchange'))
    await vi.waitFor(() => expect(button('Suggest rewrite')).toBeDefined())
    document.getSelection()!.removeAllRanges()
    document.dispatchEvent(new Event('selectionchange'))
    await vi.waitFor(() => expect(button('Suggest rewrite')).toBeUndefined())
  })
  it('anchors an exact passage, persists a rewrite separately, and keeps the source untouched', async () => {
    open()
    const paragraph = document.querySelector<HTMLElement>('[data-review-text="opening"]')!
    const node = document.createTreeWalker(paragraph, NodeFilter.SHOW_TEXT).nextNode()!
    const range = document.createRange()
    range.setStart(node, 6); range.setEnd(node, 17)
    document.getSelection()!.addRange(range)
    document.dispatchEvent(new Event('selectionchange'))
    await vi.waitFor(() => expect(button('Suggest rewrite')).toBeDefined())
    button('Suggest rewrite').click()
    await vi.waitFor(() => expect(byLabel<HTMLElement>('Your comment')).not.toBeNull())
    expect(latest.annotations[0]).toMatchObject({ paragraph_id: 'opening', start: 6, end: 16, quote: '😀 everyone', kind: 'suggestion' })
    const comment = byLabel<HTMLElement>('Your comment')
    replaceInputText(comment, 'Use a more direct greeting.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Use a more direct greeting.'))
    const replacement = byLabel<HTMLElement>('Suggested wording')
    replaceInputText(replacement, 'friends')
    button('Changes requested').click()
    await vi.waitFor(() => expect(latest.verdict).toBe('changes_requested'))
    expect(validateReviewState(data, latest)).toBeNull()
    expect(document.querySelector('[data-review-text="opening"]')?.textContent).toBe(data.paragraphs[0].text)
    expect(document.querySelectorAll('[data-review-text] [contenteditable="true"]')).toHaveLength(0)
    expect(document.querySelectorAll('[data-review-field][contenteditable="true"]')).toHaveLength(2)
    expect(latest.annotations[0].replacement).toBe('friends')
  })

  it('strikes through and restores a paragraph without removing its source or comment', async () => {
    const annotation = { id: 'keep-note', paragraph_id: 'opening', start: null, end: null, quote: null, kind: 'comment' as const, body: 'Keep this reasoning for later.', replacement: null }
    open({ ...emptyReviewState(), annotations: [annotation] })
    expect(document.querySelector('select')).toBeNull()
    byLabel<HTMLButtonElement>('Delete paragraph 1').click()
    await vi.waitFor(() => expect(latest.paragraph_marks).toEqual([{ paragraph_id: 'opening', decision: 'remove' }]))
    const source = document.querySelector<HTMLElement>('[data-review-text="opening"]')!
    expect(source.textContent).toBe(data.paragraphs[0].text)
    expect(source.classList.contains('line-through')).toBe(true)
    expect(latest.annotations).toEqual([annotation])
    byLabel<HTMLButtonElement>('Restore paragraph 1').click()
    await vi.waitFor(() => expect(latest.paragraph_marks).toEqual([]))
    expect(source.classList.contains('line-through')).toBe(false)
    expect(source.textContent).toBe(data.paragraphs[0].text)
    expect(latest.annotations).toEqual([annotation])
  })

  it('shows collapsed comment text and reuses the same field for continued typing and speech', async () => {
    const voice = voiceInput()
    open(emptyReviewState(), false, voice)
    byLabel<HTMLButtonElement>('Comment on paragraph 1').click()
    await vi.waitFor(() => expect(byLabel<HTMLElement>('Your comment')).not.toBeNull())
    replaceInputText(byLabel('Your comment'), 'Start with the benefit.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Start with the benefit.'))
    const id = latest.annotations[0].id
    byLabel<HTMLButtonElement>('Collapse comment').click()
    await vi.waitFor(() => expect(byLabel<HTMLElement>('Your comment')).toBeNull())
    const preview = document.querySelector<HTMLButtonElement>(`[data-review-note-preview="${id}"]`)!
    expect(preview.textContent).toContain('Start with the benefit.')
    expect(preview.getAttribute('aria-expanded')).toBe('false')
    preview.click()
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toBe('Start with the benefit.'))
    byLabel<HTMLButtonElement>('Comment on paragraph 1').click()
    await vi.waitFor(() => {
      expect(document.activeElement).toBe(byLabel('Your comment'))
      const editor = tiptapEditor(byLabel('Your comment'))
      expect(editor.state.selection.from).toBe(editor.state.doc.content.size - 1)
    })
    expect(latest.annotations).toHaveLength(1)
    expect(latest.annotations[0].id).toBe(id)
    tiptapEditor(byLabel('Your comment')).commands.insertContent(' Explain it plainly.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Start with the benefit. Explain it plainly.'))
    byLabel<HTMLButtonElement>('Speak comment on paragraph 1').click()
    await vi.waitFor(() => expect(voice.start).toHaveBeenCalledOnce())
    expect(latest.annotations).toHaveLength(1)
    expect(voice.start).toHaveBeenLastCalledWith(expect.objectContaining({ destination: expect.objectContaining({ annotationId: id, field: 'body' }) }))
  })

  it('adds suggested wording to the existing paragraph comment without duplicating or replacing it', async () => {
    const annotation = { id: 'existing-note', paragraph_id: 'opening', start: null, end: null, quote: null, kind: 'comment' as const, body: 'Make the greeting personal.', replacement: null }
    open({ ...emptyReviewState(), annotations: [annotation] })
    const paragraph = document.querySelector<HTMLElement>('[data-review-text="opening"]')!
    const range = document.createRange()
    range.selectNodeContents(paragraph)
    document.getSelection()!.addRange(range)
    document.dispatchEvent(new Event('selectionchange'))
    await vi.waitFor(() => expect(button('Suggest rewrite')).toBeDefined())
    button('Suggest rewrite').click()
    await vi.waitFor(() => expect(byLabel<HTMLElement>('Suggested wording')).not.toBeNull())
    expect(latest.annotations).toHaveLength(1)
    expect(latest.annotations[0]).toEqual({ ...annotation, kind: 'suggestion', replacement: data.paragraphs[0].text })
    replaceInputText(byLabel('Suggested wording'), 'Hello friends.')
    await vi.waitFor(() => expect(latest.annotations[0].replacement).toBe('Hello friends.'))
    byLabel<HTMLButtonElement>('Comment on paragraph 1').click()
    expect(latest.annotations).toHaveLength(1)
    expect(latest.annotations[0]).toMatchObject({ id: 'existing-note', body: annotation.body, kind: 'suggestion', replacement: 'Hello friends.' })
  })

  it('restores, edits and deletes comments without a lifecycle or source-location action', async () => {
    open()
    byLabel<HTMLButtonElement>('Delete paragraph 2').click()
    byLabel<HTMLButtonElement>('Comment on paragraph 2').click()
    await vi.waitFor(() => expect(byLabel<HTMLElement>('Your comment')).not.toBeNull())
    const input = byLabel<HTMLElement>('Your comment')
    replaceInputText(input, 'End with a concrete next step.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toContain('next step'))
    const saved = structuredClone(latest)
    await unmount(view!); view = undefined; document.body.replaceChildren(); open(saved)
    expect(byLabel<HTMLButtonElement>('Restore paragraph 2').getAttribute('aria-pressed')).toBe('true')
    expect(document.querySelector('[data-review-text="closing"]')?.classList.contains('line-through')).toBe(true)
    document.querySelector<HTMLButtonElement>(`[data-review-note-preview="${saved.annotations[0].id}"]`)!.click()
    await vi.waitFor(() => expect(inputText(byLabel<HTMLElement>('Your comment'))).toContain('next step'))
    for (const label of ['Open', 'Resolved', 'All', 'Resolve', 'Reopen', 'Locate in original']) {
      expect(button(label)).toBeUndefined()
    }
    replaceInputText(byLabel('Your comment'), 'End with a specific invitation.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('End with a specific invitation.'))
    expect(latest.annotations).toHaveLength(1)
    expect(latest.annotations[0].id).toBe(saved.annotations[0].id)
    expect(latest.annotations[0]).not.toHaveProperty('status')
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(deleteDialog()).not.toBeNull())
    expect(latest.annotations[0].body).toBe('End with a specific invitation.')
    button('Delete comment').click()
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    expect(document.querySelector('[data-review-note-preview]')).toBeNull()
    expect(document.querySelector('[data-review-text="closing"]')?.textContent).toBe(data.paragraphs[1].text)
    expect(latest.annotations).toEqual([])
    expect(latest.paragraph_marks).toEqual(saved.paragraph_marks)
    await vi.waitFor(() => expect(document.activeElement).toBe(byLabel('Comment on paragraph 2')))
  })

  it('continues a restored comment at the model selection end before DOM selectionchange arrives', async () => {
    // Browsers deliver selectionchange asynchronously. Commands must see the new
    // editor selection even when that DOM notification has not arrived yet.
    const holdSelectionChange = (event: Event) => event.stopImmediatePropagation()
    document.addEventListener('selectionchange', holdSelectionChange, true)
    try {
      const annotation: ReviewAnnotation = { id: 'restored-note', paragraph_id: 'opening', start: null, end: null,
        quote: null, kind: 'comment', body: 'Existing feedback.', replacement: null }
      open({ ...emptyReviewState(), annotations: [annotation] })
      byLabel<HTMLButtonElement>('Comment on paragraph 1').click()
      await vi.waitFor(() => expect(document.activeElement).toBe(byLabel('Your comment')))
      const editor = tiptapEditor(byLabel('Your comment'))
      expect(editor.state.selection.from).toBe(editor.state.doc.content.size - 1)
      editor.commands.setTextSelection(1)
      byLabel<HTMLButtonElement>('Comment on paragraph 1').click()
      await vi.waitFor(() => expect(editor.state.selection.from).toBe(editor.state.doc.content.size - 1))
      editor.commands.insertContent(' More feedback.')
      await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Existing feedback. More feedback.'))
    } finally {
      document.removeEventListener('selectionchange', holdSelectionChange, true)
    }
  })

  it('focuses only the latest comment when paragraph comments are opened in quick succession', async () => {
    const note = (id: string, paragraph_id: string): ReviewAnnotation => ({ id, paragraph_id, start: null, end: null,
      quote: null, kind: 'comment', body: `${id} feedback.`, replacement: null })
    open({ ...emptyReviewState(), annotations: [note('first', 'opening'), note('second', 'closing')] })
    byLabel<HTMLButtonElement>('Comment on paragraph 1').click()
    byLabel<HTMLButtonElement>('Comment on paragraph 2').click()
    await vi.waitFor(() => {
      expect(document.activeElement?.closest('[data-comment-id]')?.getAttribute('data-comment-id')).toBe('second')
      const editor = tiptapEditor(byLabel('Your comment'))
      expect(editor.state.selection.from).toBe(editor.state.doc.content.size - 1)
    })
    expect(document.querySelector('[data-comment-id="first"]')).toBeNull()
    expect(inputText(byLabel('Your comment'))).toBe('second feedback.')
  })

  it('removes an empty ordinary comment without opening a confirmation', async () => {
    open()
    byLabel<HTMLButtonElement>('Comment on paragraph 1').click()
    await vi.waitFor(() => expect(byLabel<HTMLElement>('Your comment')).not.toBeNull())
    replaceInputText(byLabel('Your comment'), '   ')
    await vi.waitFor(() => expect(latest.annotations[0].body.trim()).toBe(''))
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(latest.annotations).toEqual([]))
    expect(deleteDialog()).toBeNull()
    expect(document.querySelector('[data-comment-id]')).toBeNull()
    expect(document.querySelector('[data-review-text="opening"]')?.textContent).toBe(data.paragraphs[0].text)
  })

  it.each(['Cancel', 'Escape'])('preserves the complete comment when deletion is dismissed with %s', async (dismissal) => {
    // Hold editor focus frames until the modal is already open, as on a busy UI.
    const frames = new Map<number, FrameRequestCallback>()
    const requestFrame = globalThis.requestAnimationFrame
    const cancelFrame = globalThis.cancelAnimationFrame
    let frameId = 0
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frames.set(++frameId, callback); return frameId })
    vi.stubGlobal('cancelAnimationFrame', (id: number) => { frames.delete(id) })
    const annotation: ReviewAnnotation = {
      id: 'protected-note', paragraph_id: 'opening', start: 6, end: 7, quote: '😀', kind: 'suggestion',
      body: 'Use a warmer greeting.\n\n[Reference](attachment://reference-1)', replacement: 'friends',
    }
    const saved: DocumentReviewState = { ...emptyReviewState(), verdict: 'changes_requested', annotations: [annotation], paragraph_marks: [{ paragraph_id: 'closing', decision: 'remove' }] }
    open(saved)
    latest = structuredClone(saved)
    document.querySelector<HTMLButtonElement>('[data-review-note-preview="protected-note"]')!.click()
    await vi.waitFor(() => expect(byLabel<HTMLElement>('Suggested wording')).not.toBeNull())
    const deleteButton = byLabel<HTMLButtonElement>('Delete comment')
    deleteButton.click()
    await vi.waitFor(() => expect(deleteDialog()?.textContent).toContain('Delete this comment?'))
    expect(latest).toEqual(saved)
    await vi.waitFor(() => expect(document.activeElement).toBe(button('Cancel')))
    const pendingFrames = [...frames.values()]
    frames.clear()
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    for (const callback of pendingFrames) callback(performance.now())
    expect(document.activeElement).toBe(button('Cancel'))
    if (dismissal === 'Cancel') button('Cancel').click()
    else document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(deleteDialog()).toBeNull())
    expect(latest).toEqual(saved)
    expect(inputText(byLabel('Your comment'))).toBe(annotation.body)
    expect(inputText(byLabel('Suggested wording'))).toBe(annotation.replacement)
    await vi.waitFor(() => expect(document.activeElement).toBe(deleteButton))
    expect(document.querySelector('[data-review-text="opening"]')?.textContent).toBe(data.paragraphs[0].text)
  })

  it.each([
    { name: 'replacement-only suggestion', kind: 'suggestion', body: '', replacement: 'Hello friends.' },
    { name: 'attachment-only comment', kind: 'comment', body: '[Reference](attachment://reference-1)', replacement: null },
    { name: 'empty replacement deletion suggestion', kind: 'suggestion', body: '', replacement: '' },
  ] as const)('requires confirmation before deleting a $name', async ({ kind, body, replacement }) => {
    const annotation: ReviewAnnotation = { id: 'protected-note', paragraph_id: 'opening', start: null, end: null, quote: null, kind, body, replacement }
    const saved = { ...emptyReviewState(), annotations: [annotation] }
    open(saved)
    latest = structuredClone(saved)
    document.querySelector<HTMLButtonElement>('[data-review-note-preview="protected-note"]')!.click()
    await vi.waitFor(() => expect(byLabel<HTMLButtonElement>('Delete comment')).not.toBeNull())
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(deleteDialog()).not.toBeNull())
    expect(latest).toEqual(saved)
    expect(document.querySelector('[data-comment-id="protected-note"]')).not.toBeNull()
    button('Delete comment').click()
    await vi.waitFor(() => expect(latest.annotations).toEqual([]))
    expect(deleteDialog()).toBeNull()
    expect(document.querySelector('[data-review-note-preview]')).toBeNull()
    expect(document.querySelector('[data-review-text="opening"]')?.textContent).toBe(data.paragraphs[0].text)
  })

  it('dismisses a pending deletion without changing the note when the request becomes read-only', async () => {
    const annotation: ReviewAnnotation = { id: 'locking-note', paragraph_id: 'opening', start: null, end: null, quote: null, kind: 'comment', body: 'Keep this opinion.', replacement: null }
    const saved = { ...emptyReviewState(), annotations: [annotation] }
    const locked = fromStore(writable(false))
    latest = structuredClone(saved)
    view = mount(DocumentReviewWorkbench, { target: document.body, props: {
      data, state: saved, get disabled() { return locked.current }, onChange: (value) => latest = value,
    } })
    document.querySelector<HTMLButtonElement>('[data-review-note-preview="locking-note"]')!.click()
    await vi.waitFor(() => expect(byLabel<HTMLButtonElement>('Delete comment')).not.toBeNull())
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(deleteDialog()).not.toBeNull())
    locked.current = true
    await vi.waitFor(() => expect(deleteDialog()).toBeNull())
    expect(latest).toEqual(saved)
    expect(byLabel<HTMLButtonElement>('Delete comment').disabled).toBe(true)
    expect(document.querySelector('[data-comment-id="locking-note"]')?.textContent).toContain(annotation.body)
    locked.current = false
    await vi.waitFor(() => expect(byLabel<HTMLButtonElement>('Delete comment').disabled).toBe(false))
    expect(deleteDialog()).toBeNull()
  })

  it('locks submitted edits while allowing existing inline comments to expand and collapse', async () => {
    const annotation = { id: 'submitted-note', paragraph_id: 'opening', start: null, end: null, quote: null, kind: 'comment' as const, body: 'Keep the opening concise.', replacement: null }
    open({ ...emptyReviewState(), verdict: 'ready', annotations: [annotation] }, true)
    expect(byLabel<HTMLButtonElement>('Delete paragraph 1').disabled).toBe(true)
    byLabel<HTMLButtonElement>('Delete paragraph 1').click()
    expect(byLabel<HTMLButtonElement>('Comment on paragraph 1').disabled).toBe(true)
    expect(button('Changes requested').disabled).toBe(true)
    document.querySelector<HTMLButtonElement>('[data-review-note-preview="submitted-note"]')!.click()
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id="submitted-note"]')?.textContent).toContain(annotation.body))
    expect(document.querySelector('[contenteditable="true"]')).toBeNull()
    expect(byLabel<HTMLButtonElement>('Delete comment').disabled).toBe(true)
    byLabel<HTMLButtonElement>('Delete comment').click()
    expect(deleteDialog()).toBeNull()
    byLabel<HTMLButtonElement>('Collapse comment').click()
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    expect(document.querySelector('[data-review-note-preview="submitted-note"]')?.textContent).toContain(annotation.body)
    expect(document.querySelector('[data-review-text="opening"]')?.textContent).toBe(data.paragraphs[0].text)
    expect(latest).toEqual(emptyReviewState())
  })

  it('isolates editor history when opening another comment on the same paragraph', async () => {
    const annotation = { paragraph_id: 'opening', start: null, end: null, quote: null, kind: 'comment' as const, replacement: null }
    open({ ...emptyReviewState(), annotations: [
      { ...annotation, id: 'first', body: 'First comment' },
      { ...annotation, id: 'second', body: 'Second comment' },
    ] })
    const preview = (id: string) => document.querySelector<HTMLButtonElement>(`[data-review-note-preview="${id}"]`)!
    preview('first').click()
    await vi.waitFor(() => expect(inputText(byLabel<HTMLElement>('Your comment'))).toBe('First comment'))
    const firstEditor = tiptapEditor(byLabel('Your comment'))
    replaceInputText(byLabel('Your comment'), 'Edited first comment')
    expect(firstEditor.can().undo()).toBe(true)
    preview('second').click()
    await vi.waitFor(() => expect(inputText(byLabel<HTMLElement>('Your comment'))).toBe('Second comment'))
    expect(firstEditor.isDestroyed).toBe(true)
    expect(tiptapEditor(byLabel('Your comment')).commands.undo()).toBe(false)
    expect(inputText(byLabel('Your comment'))).toBe('Second comment')
    preview('first').click()
    await vi.waitFor(() => expect(inputText(byLabel<HTMLElement>('Your comment'))).toBe('Edited first comment'))
    expect(latest.annotations[1].body).toBe('Second comment')
    expect(document.querySelector('[data-review-text="opening"]')?.textContent).toBe(data.paragraphs[0].text)
  })
})
