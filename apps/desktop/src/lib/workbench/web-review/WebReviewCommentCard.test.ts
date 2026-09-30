// @vitest-environment jsdom
import { tick } from 'svelte'
import { createClassComponent } from 'svelte/legacy'
import { writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { AttachmentView, WebReviewAnnotation } from '../../generated/feedback'
import { INPUT_TOOLS_CONTEXT, type InputToolsState } from '../../input/inputToolsContext'
import { locale } from '../../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../../speech/voiceInputContext'
import type { SpeechTarget } from '../../speech/speechDraftQueue'
import { inputText, replaceInputText } from '../../../test/tiptap'
import WebReviewCommentCard from './WebReviewCommentCard.svelte'

const annotation: WebReviewAnnotation = { id: 'note-1', page_url: 'https://example.com/plans',
  viewport: { width: 1440, height: 900 }, element: { selector: '#signup', tag_name: 'button', text: 'Get started',
    rect: { x: 20, y: 40, width: 120, height: 50 } }, body: '', screenshot_attachment_id: 'capture-1' }
const documentTarget: SpeechTarget = { requestId: 'request-1', requestTitle: 'Review page', destination: { kind: 'document', action: null } }
const commentTarget: SpeechTarget = { ...documentTarget,
  destination: { kind: 'web_review_annotation', annotationId: annotation.id, elementLabel: annotation.element.text } }
const attachments: AttachmentView[] = [
  { attachment_id: 'capture-1', file_name: 'captured-element.png', media_type: 'image/png', byte_size: 1, sha256: '', position: 0 },
  { attachment_id: 'file-1', file_name: 'reference.txt', media_type: 'text/plain', byte_size: 1, sha256: '', position: 1 },
]
let view: ReturnType<typeof createClassComponent> | undefined
let latest: WebReviewAnnotation
const byLabel = <T extends HTMLElement>(label: string) => document.querySelector<T>(`[aria-label="${label}"]`)!

beforeEach(() => {
  locale.set('en'); latest = annotation
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  Element.prototype.getAnimations = (() => []) as never
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})
afterEach(async () => {
  view?.$destroy(); view = undefined; await tick()
  document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals()
})

function open(note = annotation, floating = true, disabled = false) {
  const voice = { state: writable<VoiceInputState>({ requestId: documentTarget.requestId, documentTarget,
    nextTarget: documentTarget, recording: false, disabled: false }), selectTarget: vi.fn(), start: vi.fn(), stop: vi.fn() }
  const tools = { state: writable<InputToolsState>({ requestId: documentTarget.requestId, disabled: false,
    busy: false, canCapture: true, canPaste: true, attachments }), capture: vi.fn(), paste: vi.fn(),
    files: vi.fn(), preview: vi.fn(), reportError: vi.fn() }
  const onDelete = vi.fn(), onCollapse = vi.fn()
  latest = note
  view = createClassComponent({ component: WebReviewCommentCard, target: document.body,
    context: new Map<symbol, unknown>([[VOICE_INPUT_CONTEXT, voice], [INPUT_TOOLS_CONTEXT, tools]]),
    props: { annotation: note, number: 2, floating, disabled, collapsible: !disabled,
      onUpdate: (value: WebReviewAnnotation) => { latest = value; view?.$set({ annotation: value }) }, onDelete, onCollapse } })
  return { voice, tools, onDelete, onCollapse }
}

describe('web review comment card', () => {
  it('shows a plain numbered title and comment field without captured element context in the floating card', async () => {
    locale.set('zh-CN')
    open({ ...annotation, body: '请调整颜色。\n[reference.txt](attachment://file-1)' })
    await vi.waitFor(() => expect(inputText(byLabel('批注意见'))).toBe('请调整颜色。\n'))
    expect(document.querySelector('h3')?.textContent).toBe('批注 2')
    expect(document.querySelector('blockquote')).toBeNull()
    expect(byLabel('评审时的页面信息')).toBeNull()
    for (const text of [annotation.element.text, annotation.page_url, annotation.element.selector,
      '1440 × 900', '<button>', 'captured-element.png']) expect(document.body.textContent).not.toContain(text)
    expect(document.body.textContent).toContain('reference.txt')
    expect(byLabel('删除批注')).not.toBeNull()
    expect(byLabel('收起批注')).not.toBeNull()
  })

  it('preserves captured metadata while editing and routes all input tools to the comment', async () => {
    const app = open()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    replaceInputText(byLabel('Your comment'), 'Increase contrast.')
    await vi.waitFor(() => expect(latest.body).toBe('Increase contrast.'))
    expect(latest).toEqual({ ...annotation, body: 'Increase contrast.' })
    expect(document.body.textContent).not.toContain('Finish or remove empty comments.')
    byLabel('Your comment').focus()
    expect(app.voice.selectTarget).toHaveBeenLastCalledWith(commentTarget)
    byLabel<HTMLButtonElement>('Speak comment').click()
    expect(app.voice.start).toHaveBeenCalledWith(commentTarget)
    byLabel<HTMLButtonElement>('Capture').click()
    byLabel<HTMLButtonElement>('Clipboard').click()
    await tick()
    expect(app.tools.capture).toHaveBeenCalledWith(commentTarget)
    expect(app.tools.paste).toHaveBeenCalledWith(commentTarget)
    const fileInput = document.querySelector<HTMLInputElement>('input[type="file"]')!
    vi.spyOn(fileInput, 'click').mockImplementation(() => {})
    byLabel<HTMLButtonElement>('Choose files').click()
    const file = new File(['reference'], 'reference.txt', { type: 'text/plain' })
    Object.defineProperty(fileInput, 'files', { configurable: true, value: [file] })
    fileInput.dispatchEvent(new Event('change', { bubbles: true }))
    await tick()
    expect(app.tools.files).toHaveBeenCalledWith(commentTarget, [file])
  })

  it('keeps empty comments removable and asks before deleting an existing opinion', async () => {
    const app = open()
    await vi.waitFor(() => expect(byLabel('Your comment')).not.toBeNull())
    expect(document.body.textContent).toContain('Finish or remove empty comments.')
    byLabel<HTMLButtonElement>('Collapse comment').click()
    expect(app.onCollapse).toHaveBeenCalledOnce()
    byLabel<HTMLButtonElement>('Delete comment').click()
    expect(app.onDelete).toHaveBeenCalledOnce()
    expect(document.querySelector('[data-web-review-delete-dialog]')).toBeNull()
    view!.$set({ annotation: { ...annotation, body: 'Keep this opinion.' } }); await tick()
    byLabel<HTMLButtonElement>('Delete comment').click()
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-delete-dialog]')).not.toBeNull())
    expect(app.onDelete).toHaveBeenCalledOnce()
    Array.from(document.querySelectorAll('button')).find((button) => button.textContent === 'Cancel')!.click()
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-delete-dialog]')).toBeNull())
    expect(app.onDelete).toHaveBeenCalledOnce()
  })

  it('retains captured context and screenshot for a read-only history card', async () => {
    open({ ...annotation, body: 'Increase contrast.' }, false, true)
    await vi.waitFor(() => expect(inputText(byLabel('Your comment'))).toBe('Increase contrast.'))
    expect(document.querySelector('h3')).toBeNull()
    expect(document.querySelector('blockquote')?.textContent).toBe(annotation.element.text)
    const context = byLabel('Saved page context').textContent
    expect(context).toContain(annotation.page_url)
    expect(context).toContain('1440 × 900 · <button>')
    expect(context).toContain(annotation.element.selector)
    expect(document.body.textContent).toContain('captured-element.png')
    expect(document.querySelector('[contenteditable="true"]')).toBeNull()
    expect(byLabel('Delete comment')).toBeNull()
    expect(byLabel('Collapse comment')).toBeNull()
  })
})
