// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { writable } from 'svelte/store'
import type { WebReviewAnnotation, WebReviewData } from '../../generated/feedback'
import { locale } from '../../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputContext, type VoiceInputState } from '../../speech/voiceInputContext'
import type { SpeechTarget } from '../../speech/speechDraftQueue'
import { inputText, replaceInputText } from '../../../test/tiptap'
import type { WebReviewFrameState, WebReviewRect, WebReviewSelection } from './webReviewProtocol'
import { emptyWebReviewState, type WebReviewState } from './reviewModel'
import WebReviewWorkbench from './WebReviewWorkbench.svelte'

type FrameHooks = { url: string; onState: (state: WebReviewFrameState) => void; onSelection: (selection: WebReviewSelection) => void; onAnchor?: (rect: WebReviewRect | null) => void }
const bridge = vi.hoisted(() => ({ options: null as FrameHooks | null, mode: vi.fn(), focus: vi.fn(), connect: vi.fn() }))
vi.mock('./webReviewFrame', () => ({ createWebReviewFrame: (options: FrameHooks) => {
  bridge.options = options
  return { setMode: bridge.mode, setAnnotations: vi.fn(), focus: bridge.focus, connect: bridge.connect, dispose: vi.fn() }
} }))

const data: WebReviewData = { title: 'Landing page', url: 'https://example.com/', source_version: 'v1', viewport: { width: 1440, height: 900 } }
const note: WebReviewAnnotation = { id: 'note-1', page_url: data.url, viewport: data.viewport,
  element: { selector: '#start', tag_name: 'button', text: 'Get started', rect: { x: 20, y: 40, width: 120, height: 50 } }, body: 'Increase contrast.' }
const selection: WebReviewSelection = { page_url: data.url, page_title: 'Landing', selector: '#start', tag_name: 'button', text: 'Get started', attributes: {},
  rect: { x: 20, y: 40, width: 120, height: 50 }, viewport: { ...data.viewport, scroll_x: 0, scroll_y: 0 }, captured_at: '2026-09-30T12:00:00Z' }
const documentTarget: SpeechTarget = { requestId: 'request-1', requestTitle: 'Review page', destination: { kind: 'document', action: null } }
let view: ReturnType<typeof mount> | undefined
let latest: WebReviewState
const byLabel = <T extends HTMLElement>(label: string) => document.querySelector<T>(`[aria-label="${label}"]`)!

beforeEach(() => {
  locale.set('en'); latest = emptyWebReviewState(); bridge.options = null; vi.clearAllMocks()
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  Element.prototype.getAnimations = (() => []) as never
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.body.replaceChildren(); vi.unstubAllGlobals() })
function open(state = emptyWebReviewState(), readOnly = false, voice?: VoiceInputContext) {
  view = mount(WebReviewWorkbench, { target: document.body, context: voice ? new Map([[VOICE_INPUT_CONTEXT, voice]]) : undefined,
    props: { data, state, readOnly, onChange: (value) => latest = value } })
}
async function connect() {
  await vi.waitFor(() => expect(bridge.options).not.toBeNull())
  bridge.options!.onState({ status: 'ready', page_url: data.url, page_title: 'Landing', viewport: selection.viewport })
  await vi.waitFor(() => expect(byLabel<HTMLButtonElement>('Select elements').disabled).toBe(false))
}
function voiceInput() {
  return { state: writable<VoiceInputState>({ requestId: documentTarget.requestId, documentTarget, nextTarget: documentTarget, recording: false, disabled: false }),
    selectTarget: vi.fn(), start: vi.fn(), stop: vi.fn() }
}

describe('web review workbench', () => {
  it('gates selection on a real bridge connection and explains the preview fallback', async () => {
    open()
    expect(byLabel<HTMLButtonElement>('Select elements').disabled).toBe(true)
    await vi.waitFor(() => expect(bridge.options).not.toBeNull())
    bridge.options!.onState({ status: 'unavailable', reason: 'bridge_missing', page_url: data.url, page_title: '', viewport: null })
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-status]')?.textContent).toContain('review bridge'))
    expect(byLabel<HTMLButtonElement>('Select elements').disabled).toBe(true)
    await connect()
    byLabel<HTMLButtonElement>('Select elements').click()
    await vi.waitFor(() => expect(bridge.mode).toHaveBeenLastCalledWith('select'))
  })
  it('creates an inline note, routes typing and voice to it, and reopens the same comment from the list', async () => {
    const voice = voiceInput()
    open(emptyWebReviewState(), false, voice); await connect()
    byLabel<HTMLButtonElement>('Select elements').click()
    await vi.waitFor(() => expect(bridge.mode).toHaveBeenLastCalledWith('select'))
    bridge.options!.onSelection(selection)
    bridge.options!.onAnchor?.(selection.rect)
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    expect(latest.annotations).toHaveLength(1)
    expect(latest.annotations[0]).toMatchObject({ page_url: data.url, viewport: data.viewport, element: note.element, body: '' })
    replaceInputText(byLabel('Your comment'), 'Increase contrast.')
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Increase contrast.'))
    const target: SpeechTarget = { ...documentTarget, destination: { kind: 'web_review_annotation', annotationId: latest.annotations[0].id, elementLabel: 'Get started' } }
    byLabel('Your comment').focus()
    expect(voice.selectTarget).toHaveBeenLastCalledWith(target)
    byLabel<HTMLButtonElement>('Collapse comment').click()
    await vi.waitFor(() => expect(document.querySelector('[data-web-comment-id]')).toBeNull())
    const list = Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.includes('Review comments'))!
    list.click()
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-note-preview]')).not.toBeNull())
    document.querySelector<HTMLButtonElement>('[data-web-review-note-preview]')!.click()
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toBe('Increase contrast.'))
    expect(latest.annotations).toHaveLength(1)
    byLabel<HTMLButtonElement>('Collapse comment').click()
    voice.state.update((state) => ({ ...state, revealTarget: target, revealSequence: 1 }))
    await vi.waitFor(() => expect(document.activeElement?.getAttribute('data-web-review-field')).toBe('body'))
    expect(voice.start).not.toHaveBeenCalled()
  })
  it('keeps an unfinished note removable from the comment list', async () => {
    open({ ...emptyWebReviewState(), annotations: [{ ...note, body: '' }] })
    Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.includes('Review comments'))!.click()
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-note-preview]')).not.toBeNull())
    document.querySelector<HTMLButtonElement>('[data-web-review-note-preview]')!.click()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(latest.annotations).toEqual([]))
    expect(document.querySelector('[data-web-review-delete-dialog]')).toBeNull()
  })
  it('navigates back to a saved comment after SPA navigation even when the iframe src is unchanged', async () => {
    open({ ...emptyWebReviewState(), annotations: [note] }); await connect()
    const originalFrame = bridge.options!
    originalFrame.onState({ status: 'ready', page_url: `${data.url}about`, page_title: 'About', viewport: selection.viewport })
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-status]')?.getAttribute('data-web-review-status')).toBe('ready'))
    Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.includes('Review comments'))!.click()
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-note-preview]')).not.toBeNull())
    document.querySelector<HTMLButtonElement>('[data-web-review-note-preview]')!.click()
    await vi.waitFor(() => expect(bridge.options).not.toBe(originalFrame))
    expect(bridge.options!.url).toBe(note.page_url)
    expect(bridge.focus).toHaveBeenLastCalledWith({ selector: note.element.selector, page_url: note.page_url, sequence: 1 })
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toBe(note.body))
  })
  it('reopens a comment on the current page without reloading the iframe', async () => {
    open({ ...emptyWebReviewState(), annotations: [note] }); await connect()
    const originalFrame = bridge.options!
    Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.includes('Review comments'))!.click()
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-note-preview]')).not.toBeNull())
    document.querySelector<HTMLButtonElement>('[data-web-review-note-preview]')!.click()
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toBe(note.body))
    expect(bridge.options).toBe(originalFrame)
    expect(bridge.focus).toHaveBeenLastCalledWith({ selector: note.element.selector, page_url: note.page_url, sequence: 1 })
  })
  it('shows submitted comments and their saved context without mounting an executable live page', async () => {
    open({ ...emptyWebReviewState(), annotations: [note] }, true)
    expect(document.querySelector('iframe')).toBeNull()
    expect(bridge.connect).not.toHaveBeenCalled()
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toBe(note.body))
    expect(document.body.textContent).toContain(note.page_url)
    expect(document.body.textContent).toContain(note.element.selector)
    expect(document.querySelector('[contenteditable="true"]')).toBeNull()
    expect(byLabel('Delete comment')).toBeNull()
  })
})
