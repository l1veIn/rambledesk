// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fromStore, writable } from 'svelte/store'
import type { DocumentReviewData, ReviewAnnotation } from '../../generated/feedback'
import { locale } from '../../preferences'
import { emptyReviewState, validateReviewState, type DocumentReviewState } from './reviewModel'
import DocumentReviewWorkbench from './DocumentReviewWorkbench.svelte'
import { VOICE_INPUT_CONTEXT, type VoiceInputContext, type VoiceInputState } from '../../speech/voiceInputContext'
import type { SpeechTarget } from '../../speech/speechDraftQueue'
import { inputText, replaceInputText, tiptapEditor } from '../../../test/tiptap'

const data: DocumentReviewData = { title: 'Opening speech', source_version: 'v1', markdown: 'Hello 😀 everyone. This is our promise.\n\nThank you for listening.' }
let view: ReturnType<typeof mount> | undefined
let latest: DocumentReviewState
const button = (label: string) => Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.trim() === label)!
const byLabel = <T extends HTMLElement>(label: string) => document.querySelector<T>(`[aria-label="${label}"]${label === 'Your comment' ? ', [aria-label="Reason (optional)"]' : ''}`)!
const deleteDialog = () => document.querySelector<HTMLElement>('[data-review-delete-dialog]')
const preview = (id: string) => document.querySelector<HTMLButtonElement>(`[data-review-note-preview="${id}"]`)!
const note = (id = 'note-1', patch: Partial<ReviewAnnotation> = {}): ReviewAnnotation => ({ id, start: 6, end: 16, quote: '😀 everyone', kind: 'comment', body: 'A warmer greeting.', replacement: null, ...patch })
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
function open(state = emptyReviewState(), disabled = false, voice?: VoiceInputContext, input = data) {
  latest = structuredClone(state)
  view = mount(DocumentReviewWorkbench, { target: document.body, context: voice ? new Map([[VOICE_INPUT_CONTEXT, voice]]) : undefined,
    props: { data: input, state, disabled, onChange: (value) => latest = value } })
}
function selectSourceText(first: string, last = first) {
  const elements = [...document.querySelectorAll<HTMLElement>('[data-review-leaf]')]
  const start = elements.find((element) => element.textContent!.includes(first))!
  const end = elements.find((element) => element.textContent!.includes(last))!
  const position = (element: HTMLElement, offset: number): [Node, number] => {
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (offset <= node.textContent!.length) return [node, offset]
      offset -= node.textContent!.length
    }
    throw new Error('Text position not found')
  }
  const range = document.createRange()
  range.setStart(...position(start, start.textContent!.indexOf(first)))
  range.setEnd(...position(end, end.textContent!.indexOf(last) + last.length))
  document.getSelection()!.removeAllRanges(); document.getSelection()!.addRange(range)
  document.dispatchEvent(new Event('selectionchange'))
}
const documentTarget: SpeechTarget = { requestId: 'request-1', requestTitle: 'Review draft', destination: { kind: 'document', action: null } }
function voiceInput() {
  const state = writable<VoiceInputState>({ requestId: documentTarget.requestId, documentTarget, nextTarget: documentTarget, recording: false, disabled: false })
  return { state, selectTarget: vi.fn(), start: vi.fn(), stop: vi.fn() }
}

describe('Markdown document review interaction', () => {
  it.each(['Replacement document.', ''])('previews a persisted whole-document suggestion against the complete source: %j', async (replacement) => {
    const annotation = note('whole-change', { start: null, end: null, quote: null, kind: 'suggestion', body: '', replacement })
    open({ ...emptyReviewState(), verdict: 'changes_requested', annotations: [annotation] })
    preview(annotation.id).click()
    await vi.waitFor(() => expect(byLabel('Suggested wording')).not.toBeNull())
    const difference = document.querySelector('[data-review-suggestion-diff]')!
    expect(difference.querySelector('del')?.textContent).toContain('Hello 😀 everyone')
    expect(difference.querySelector('ins')?.textContent ?? '').toBe(replacement ? 'Replacement document' : '')
    expect(validateReviewState(data, latest)).toBeNull()
    button('Done').click()
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    expect(latest.annotations[0]).toEqual(annotation)
  })
  it('positions the same editor beside a selected line in a long single block instead of its end', async () => {
    const nativeRect = HTMLElement.prototype.getBoundingClientRect
    const geometry = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function(this: HTMLElement) {
      if (this.matches('[data-annotation-ids]')) return new DOMRect(20, 180, 500, 20)
      if (this.matches('.review-source-body')) return new DOMRect(20, 100, 700, 3000)
      if (this.matches('.review-source-layout')) return new DOMRect(20, 100, 700, 3000)
      return nativeRect.call(this)
    })
    try {
      open(emptyReviewState(), false, undefined, { ...data, markdown: '```text\n' + Array.from({ length: 120 }, (_, index) => `line ${index}`).join('\n') + '\n```' })
      selectSourceText('line 35')
      await vi.waitFor(() => expect(button('Comment')).toBeDefined())
      button('Comment').click()
      await vi.waitFor(() => expect(document.querySelector('[data-review-card-placement="floating"]')).not.toBeNull())
      expect((document.querySelector('.review-active-card') as HTMLElement).style.top).toBe('112px')
      expect(latest.annotations[0].quote).toBe('line 35')
      expect(HTMLElement.prototype.scrollIntoView).not.toHaveBeenCalled()
    } finally { geometry.mockRestore() }
  })
  it('discards only untouched new suggestions, while preserving externally echoed speech or attachment contents', async () => {
    const echoed = fromStore(writable(emptyReviewState()))
    view = mount(DocumentReviewWorkbench, { target: document.body, props: { data,
      get state() { return echoed.current }, onChange: (value) => { latest = value; echoed.current = value } } })
    selectSourceText('everyone')
    await vi.waitFor(() => expect(button('Suggest change')).toBeDefined())
    button('Suggest change').click()
    await vi.waitFor(() => expect(byLabel('Suggested wording')).not.toBeNull())
    button('Done').click()
    await vi.waitFor(() => expect(document.body.textContent).toContain('Change the wording or add a reason'))
    byLabel('Suggested wording').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(latest.annotations).toHaveLength(0))
    expect(document.activeElement?.hasAttribute('data-review-leaf')).toBe(true)
    selectSourceText('everyone')
    await vi.waitFor(() => expect(button('Suggest change')).toBeDefined())
    button('Suggest change').click()
    await vi.waitFor(() => expect(echoed.current.annotations).toHaveLength(1))
    const id = echoed.current.annotations[0].id
    echoed.current = { ...echoed.current, annotations: [{ ...echoed.current.annotations[0], body: 'Spoken reason\n[Image](attachment://image-1)' }] }
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toContain('Spoken reason'))
    byLabel('Suggested wording').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    expect(echoed.current.annotations).toHaveLength(1)
    expect(echoed.current.annotations[0]).toMatchObject({ id, body: 'Spoken reason\n[Image](attachment://image-1)' })
  })
  it('keeps a newly selected note beside its source block instead of jumping to the document end', async () => {
    open(); selectSourceText('everyone')
    await vi.waitFor(() => expect(button('Comment')).toBeDefined())
    button('Comment').focus()
    document.getSelection()!.removeAllRanges(); document.dispatchEvent(new Event('selectionchange'))
    button('Comment').click()
    await vi.waitFor(() => expect(document.activeElement).toBe(byLabel('Your comment')))
    const card = document.querySelector('[data-comment-id]')!
    expect(card.closest('[data-review-block]')?.querySelector('[data-review-text]')?.textContent).toContain('Hello 😀 everyone')
    expect(card.closest('[data-review-block]')?.nextElementSibling?.textContent).toContain('Thank you for listening')
    expect(latest.annotations[0].quote).toBe('everyone')
    expect(HTMLElement.prototype.scrollIntoView).not.toHaveBeenCalled()
  })
  it('maps a selection across an open card to source only, and ignores selections inside its editor', async () => {
    open({ ...emptyReviewState(), annotations: [note()] }); preview('note-1').click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    const field = byLabel('Your comment'), range = document.createRange()
    range.selectNodeContents(field); document.getSelection()!.removeAllRanges(); document.getSelection()!.addRange(range)
    document.dispatchEvent(new Event('selectionchange'))
    expect(button('Comment')).toBeUndefined()
    selectSourceText('Hello', 'Thank you')
    await vi.waitFor(() => expect(button('Comment')).toBeDefined())
    button('Comment').click()
    await vi.waitFor(() => expect(latest.annotations).toHaveLength(2))
    expect(latest.annotations[1].quote).toBe('Hello 😀 everyone. This is our promise.\n\nThank you')
    expect(latest.annotations[1].quote).not.toContain('A warmer greeting')
  })
  it.each(['Control', 'Meta'])('finishes only the current valid note with %s+Enter and preserves drafts on Escape', async (modifier) => {
    open(); button('Comment on document').click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    const command = () => new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: modifier === 'Control', metaKey: modifier === 'Meta', bubbles: true, cancelable: true })
    expect(byLabel('Your comment').dispatchEvent(command())).toBe(false)
    expect(document.querySelector('[data-comment-id]')).not.toBeNull()
    expect(latest.annotations[0].body).toBe('')
    replaceInputText(byLabel('Your comment'), 'Keep this exact draft.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Keep this exact draft.'))
    byLabel('Your comment').dispatchEvent(command())
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    expect(latest.verdict).toBeNull()
    const id = latest.annotations[0].id; preview(id).click()
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toBe('Keep this exact draft.'))
    byLabel('Your comment').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    expect(latest.annotations[0].body).toBe('Keep this exact draft.')
  })
  it('preserves empty comment drafts on Escape and does not handle IME commands', async () => {
    open(); button('Comment on document').click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    const composing = new KeyboardEvent('keydown', { key: 'Escape', isComposing: true, bubbles: true, cancelable: true })
    byLabel('Your comment').dispatchEvent(composing)
    expect(composing.defaultPrevented).toBe(false)
    expect(document.querySelector('[data-comment-id]')).not.toBeNull()
    byLabel('Your comment').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    expect(latest.annotations).toHaveLength(1)
    button('Ready as written').click()
    await vi.waitFor(() => expect(validateReviewState(data, latest)).toBe('Finish or remove empty comments.'))
  })
  it('renders a whole Markdown document in one immutable view with headings, lists, table and code', () => {
    open(emptyReviewState(), false, undefined, { ...data, markdown: '# Heading\n\n- Item\n\n| A | B |\n|---|---|\n| One | Two |\n\n```ts\nconst x = 1\n```' })
    expect(document.querySelector('[data-review-text] h1')?.textContent).toBe('Heading')
    expect(document.querySelector('[data-review-text] li')?.textContent).toBe('Item')
    expect(document.querySelectorAll('[data-review-text] td')).toHaveLength(2)
    expect(document.querySelector('[data-review-text] pre')?.textContent).toBe('const x = 1')
    expect(document.querySelector('[role="tablist"]')).toBeNull()
    expect(document.querySelector('[data-review-text] [contenteditable]')).toBeNull()
    expect(document.querySelector('[data-tour="review-comment"]')).not.toBeNull()
    expect(document.querySelector('[data-tour="review-verdict"]')).not.toBeNull()
  })
  it('creates an exact cross-paragraph anchor and saves suggested wording separately', async () => {
    open()
    selectSourceText('😀 everyone', 'Thank you')
    await vi.waitFor(() => expect(button('Suggest change')).toBeDefined())
    button('Suggest change').click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    expect(latest.annotations[0]).toMatchObject({ start: 6, quote: '😀 everyone. This is our promise.\n\nThank you', kind: 'suggestion' })
    replaceInputText(byLabel('Your comment'), 'Use a direct greeting.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Use a direct greeting.'))
    replaceInputText(byLabel('Suggested wording'), 'Hello friends.')
    await vi.waitFor(() => expect(latest.annotations[0].replacement).toBe('Hello friends.'))
    button('Changes requested').click()
    await vi.waitFor(() => expect(validateReviewState(data, latest)).toBeNull())
    expect(latest.annotations[0].replacement).toBe('Hello friends.')
    expect(document.querySelector('[data-review-text]')?.textContent).toContain('This is our promise.')
  })
  it('creates multiple independent notes in one paragraph even when an earlier note is selected', async () => {
    const original = note()
    open({ ...emptyReviewState(), annotations: [original] })
    preview(original.id).click()
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toBe(original.body))
    // Opening the card focuses its editor after positioning; wait for that before selecting new source text.
    await vi.waitFor(() => expect(document.activeElement).toBe(byLabel('Your comment')))
    selectSourceText('This is our promise.')
    await vi.waitFor(() => expect(button('Comment')).toBeDefined())
    expect(byLabel('Selected passage').textContent).toContain('This is our promise.')
    button('Comment').click()
    await vi.waitFor(() => expect(latest.annotations).toHaveLength(2))
    expect(latest.annotations[0]).toEqual(original)
    expect(latest.annotations[1]).toMatchObject({ quote: 'This is our promise.', body: '', kind: 'comment' })
    expect(latest.annotations[1].id).not.toBe(original.id)
  })
  it('suggests deletion from the change card without requiring a reason', async () => {
    open(); selectSourceText('everyone')
    await vi.waitFor(() => expect(button('Suggest change')).toBeDefined())
    button('Suggest change').click()
    await vi.waitFor(() => expect(button('Suggest deletion')).toBeDefined())
    button('Suggest deletion').click()
    await vi.waitFor(() => expect(latest.annotations).toHaveLength(1))
    expect(latest.annotations[0]).toMatchObject({ quote: 'everyone', kind: 'suggestion', replacement: '', body: '' })
    button('Changes requested').click()
    await vi.waitFor(() => expect(validateReviewState(data, latest)).toBeNull())
    expect(document.querySelector('.review-deletion')?.textContent).toBe('everyone')
  })
  it('disables oversized rewrite prefill while keeping comments available', async () => {
    open(emptyReviewState(), false, undefined, { ...data, markdown: 'x'.repeat(8001) })
    selectSourceText('x'.repeat(8001))
    await vi.waitFor(() => expect(button('Suggest change')?.disabled).toBe(true))
    expect(button('Comment').disabled).toBe(false)
    expect(button('Suggest deletion')).toBeUndefined()
  })
  it('clears an expired selection without creating a comment', async () => {
    open(); selectSourceText('everyone')
    await vi.waitFor(() => expect(button('Comment')).toBeDefined())
    document.getSelection()!.removeAllRanges(); document.dispatchEvent(new Event('selectionchange'))
    await vi.waitFor(() => expect(button('Comment')).toBeUndefined())
    expect(latest.annotations).toEqual([])
  })
  it('routes a whole-document comment voice toolbar to its stable annotation target', async () => {
    const voice = voiceInput(); open(emptyReviewState(), false, voice)
    button('Comment on document').click()
    await vi.waitFor(() => expect(document.querySelector('[data-input-toolbar]')).not.toBeNull())
    byLabel<HTMLButtonElement>('Speak comment').click()
    await vi.waitFor(() => expect(voice.start).toHaveBeenCalledOnce())
    expect(voice.start).toHaveBeenCalledWith({ ...documentTarget, destination: { kind: 'review_annotation', annotationId: latest.annotations[0].id,
      field: 'body', sourceVersion: 'v1', sourceLabel: 'Whole document' } })
    expect(latest.annotations[0]).toMatchObject({ start: null, end: null, quote: null })
  })
  it.each(['body', 'replacement'] as const)('focus selects the %s voice target and reveal reopens the same card', async (field) => {
    const voice = voiceInput(), annotation = note('voice-note', { kind: 'suggestion', replacement: 'friends' })
    open({ ...emptyReviewState(), annotations: [annotation] }, false, voice)
    preview(annotation.id).click()
    await vi.waitFor(() => expect(byLabel('Suggested wording')).not.toBeNull())
    expect(document.querySelectorAll('[data-input-toolbar]')).toHaveLength(2)
    expect(document.querySelectorAll('[aria-label="Capture"]')).toHaveLength(2)
    byLabel(field === 'body' ? 'Your comment' : 'Suggested wording').focus()
    const target: SpeechTarget = { ...documentTarget, destination: { kind: 'review_annotation', annotationId: annotation.id, field, sourceVersion: 'v1', sourceLabel: 'Selected passage' } }
    expect(voice.selectTarget).toHaveBeenLastCalledWith(target)
    byLabel<HTMLButtonElement>('Collapse comment').click()
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).toBeNull())
    voice.state.update((state) => ({ ...state, revealTarget: target, revealSequence: 1 }))
    await vi.waitFor(() => expect(document.activeElement?.getAttribute('data-review-field')).toBe(field))
    expect(inputText(byLabel('Your comment'))).toBe(annotation.body)
    expect(voice.start).not.toHaveBeenCalled()
  })
  it('reopens the same collapsed note with the editor caret at the end', async () => {
    const annotation = note(); open({ ...emptyReviewState(), annotations: [annotation] })
    preview(annotation.id).click()
    await vi.waitFor(() => expect(document.activeElement).toBe(byLabel('Your comment')))
    const editor = tiptapEditor(byLabel('Your comment'))
    expect(editor.state.selection.from).toBe(editor.state.doc.content.size - 1)
    editor.commands.insertContent(' More feedback.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe(`${annotation.body} More feedback.`))
    byLabel<HTMLButtonElement>('Collapse comment').click()
    await vi.waitFor(() => expect(preview(annotation.id).textContent).toContain('More feedback.'))
    preview(annotation.id).click()
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toContain('More feedback.'))
    expect(latest.annotations).toHaveLength(1)
  })
  it('moves focus and the reading position from an edited comment back to its source highlight', async () => {
    const annotation = note(); open({ ...emptyReviewState(), annotations: [annotation] })
    preview(annotation.id).click()
    await vi.waitFor(() => expect(document.activeElement).toBe(byLabel('Your comment')))
    replaceInputText(byLabel('Your comment'), 'Updated reason.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Updated reason.'))
    const target = document.querySelector<HTMLElement>('[data-annotation-ids="note-1"]')!
    vi.mocked(HTMLElement.prototype.scrollIntoView).mockClear()
    button('Locate in original').click()
    expect(document.activeElement).toBe(target)
    expect(HTMLElement.prototype.scrollIntoView).toHaveBeenLastCalledWith({ block: 'center', behavior: 'instant' })
    expect(vi.mocked(HTMLElement.prototype.scrollIntoView).mock.instances.at(-1)).toBe(target)
    expect(document.getSelection()?.toString()).toBe('')
    expect(latest.annotations[0].body).toBe('Updated reason.')
  })
  it('removes an empty comment immediately and restores focus to the document entry point', async () => {
    open(); button('Comment on document').click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(latest.annotations).toEqual([]))
    expect(deleteDialog()).toBeNull()
    await vi.waitFor(() => expect(document.activeElement).toBe(button('Comment on document')))
  })
  it.each(['Cancel', 'Escape'])('keeps all contents when deleting a written suggestion is dismissed by %s', async (dismissal) => {
    const annotation = note('protected', { kind: 'suggestion', body: 'Reason\n\n[Reference](attachment://reference-1)', replacement: 'friends' })
    const saved = { ...emptyReviewState(), annotations: [annotation] }; open(saved)
    preview(annotation.id).click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(deleteDialog()).not.toBeNull())
    await vi.waitFor(() => expect(document.activeElement).toBe(button('Cancel')))
    if (dismissal === 'Cancel') button('Cancel').click()
    else document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(deleteDialog()).toBeNull())
    expect(latest).toEqual(saved)
    expect(inputText(byLabel('Your comment'))).toBe(annotation.body)
    expect(inputText(byLabel('Suggested wording'))).toBe(annotation.replacement)
  })
  it.each([
    { kind: 'comment' as const, body: '[File](attachment://file-1)', replacement: null },
    { kind: 'suggestion' as const, body: '', replacement: '' },
    { kind: 'suggestion' as const, body: '', replacement: 'Friends' },
  ])('confirms deletion of attachment/suggestion content: %j', async (patch) => {
    const annotation = note('protected', patch); open({ ...emptyReviewState(), annotations: [annotation] })
    preview(annotation.id).click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(deleteDialog()).not.toBeNull())
    expect(latest.annotations).toEqual([annotation])
    button('Delete comment').click()
    await vi.waitFor(() => expect(latest.annotations).toEqual([]))
  })
  it('cancels a pending deletion when the request becomes read-only', async () => {
    const annotation = note(), saved = { ...emptyReviewState(), annotations: [annotation] }
    const locked = fromStore(writable(false)); latest = structuredClone(saved)
    view = mount(DocumentReviewWorkbench, { target: document.body, props: { data, state: saved,
      get disabled() { return locked.current }, onChange: (value) => latest = value } })
    preview(annotation.id).click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(deleteDialog()).not.toBeNull())
    locked.current = true
    await vi.waitFor(() => expect(deleteDialog()).toBeNull())
    expect(latest).toEqual(saved)
    expect(byLabel<HTMLButtonElement>('Delete comment').disabled).toBe(true)
  })
  it('allows inspecting submitted notes but never changes state or offers editable source', async () => {
    const annotation = note(), saved = { ...emptyReviewState(), verdict: 'ready' as const, annotations: [annotation] }
    open(saved, true); preview(annotation.id).click()
    await vi.waitFor(() => expect(document.querySelector('[data-comment-id]')).not.toBeNull())
    expect(byLabel<HTMLButtonElement>('Delete comment').disabled).toBe(true)
    expect(button('Comment on document').disabled).toBe(true)
    selectSourceText('everyone')
    expect(button('Comment')).toBeUndefined()
    expect(document.querySelector('[contenteditable="true"]')).toBeNull()
    expect(latest).toEqual(saved)
  })
  it('preserves overlapping notes and focuses only the latest card opened in quick succession', async () => {
    const first = note('first'), second = note('second', { start: 8, end: 20, quote: 'everyone. Th', body: 'Second note' })
    open({ ...emptyReviewState(), annotations: [first, second] })
    expect(document.querySelector('[data-annotation-ids="first second"]')).not.toBeNull()
    preview(first.id).click(); preview(second.id).click()
    await vi.waitFor(() => expect(document.activeElement?.closest('[data-comment-id]')?.getAttribute('data-comment-id')).toBe('second'))
    expect(inputText(byLabel('Your comment'))).toBe('Second note')
    expect(latest.annotations).toEqual([first, second])
  })
})
