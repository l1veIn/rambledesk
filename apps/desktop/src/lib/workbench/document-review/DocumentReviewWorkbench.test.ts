// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { DocumentReviewData } from '../../generated/feedback'
import { locale } from '../../preferences'
import { emptyReviewState, validateReviewState, type DocumentReviewState } from './reviewModel'
import DocumentReviewWorkbench from './DocumentReviewWorkbench.svelte'

const data: DocumentReviewData = { title: 'Opening speech', source_version: 'v1', paragraphs: [
  { id: 'opening', label: 'Opening', text: 'Hello 😀 everyone. This is our promise.' },
  { id: 'closing', label: 'Closing', text: 'Thank you for listening.' },
] }
let view: ReturnType<typeof mount> | undefined
let latest: DocumentReviewState
const button = (label: string) => Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.trim() === label)!
const byLabel = <T extends HTMLElement>(label: string) => document.querySelector<T>(`[aria-label="${label}"]`)!
beforeEach(() => {
  locale.set('en'); latest = emptyReviewState()
  HTMLElement.prototype.scrollIntoView = vi.fn()
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.getSelection()?.removeAllRanges(); document.body.replaceChildren() })
function open(state = emptyReviewState(), disabled = false) {
  view = mount(DocumentReviewWorkbench, { target: document.body, props: { data, state, disabled, onChange: (value) => latest = value } })
}

describe('document review interaction', () => {
  it('clears an expired selection and supports keyboard tab navigation', async () => {
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
    document.querySelector('#review-original-tab')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
    await vi.waitFor(() => expect(document.activeElement?.id).toBe('review-comments-tab'))
    expect(document.querySelector('#review-comments-tab')?.getAttribute('tabindex')).toBe('0')
    expect(document.querySelector('#review-original-tab')?.getAttribute('tabindex')).toBe('-1')
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
    await vi.waitFor(() => expect(byLabel<HTMLTextAreaElement>('Your comment')).not.toBeNull())
    expect(latest.annotations[0]).toMatchObject({ paragraph_id: 'opening', start: 6, end: 16, quote: '😀 everyone', kind: 'suggestion' })
    const comment = byLabel<HTMLTextAreaElement>('Your comment')
    comment.value = 'Use a more direct greeting.'; comment.dispatchEvent(new Event('input', { bubbles: true }))
    await vi.waitFor(() => expect(latest.annotations[0].body).toBe('Use a more direct greeting.'))
    const replacement = byLabel<HTMLTextAreaElement>('Suggested wording')
    replacement.value = 'friends'; replacement.dispatchEvent(new Event('input', { bubbles: true }))
    button('Changes requested').click()
    await vi.waitFor(() => expect(latest.verdict).toBe('changes_requested'))
    expect(validateReviewState(data, latest)).toBeNull()
    expect(document.querySelector('[data-review-text="opening"]')?.textContent).toBe(data.paragraphs[0].text)
    expect(document.querySelectorAll('[contenteditable="true"]')).toHaveLength(0)
  })
  it('restores paragraph marks and comments, locates the source, resolves and reopens an opinion', async () => {
    open()
    const mark = byLabel<HTMLSelectElement>('Mark paragraph 2')
    mark.value = 'revise'; mark.dispatchEvent(new Event('change', { bubbles: true }))
    byLabel<HTMLButtonElement>('Comment on paragraph 2').click()
    await vi.waitFor(() => expect(byLabel<HTMLTextAreaElement>('Your comment')).not.toBeNull())
    const input = byLabel<HTMLTextAreaElement>('Your comment')
    input.value = 'End with a concrete next step.'; input.dispatchEvent(new Event('input', { bubbles: true }))
    await vi.waitFor(() => expect(latest.annotations[0].body).toContain('next step'))
    const saved = structuredClone(latest)
    await unmount(view!); view = undefined; document.body.replaceChildren(); open(saved)
    expect(byLabel<HTMLSelectElement>('Mark paragraph 2').value).toBe('revise')
    document.querySelector<HTMLButtonElement>('[role="tab"][id="review-comments-tab"]')!.click()
    await vi.waitFor(() => expect(byLabel<HTMLTextAreaElement>('Your comment')?.value).toContain('next step'))
    button('Resolve').click()
    await vi.waitFor(() => expect(latest.annotations[0].status).toBe('resolved'))
    button('Resolved').click()
    await vi.waitFor(() => expect(button('Reopen')).toBeDefined())
    button('Reopen').click()
    button('All').click()
    await vi.waitFor(() => expect(button('Locate in original')).toBeDefined())
    button('Locate in original').click()
    await vi.waitFor(() => expect(document.querySelector('[data-review-text="closing"]')).not.toBeNull())
    expect(HTMLElement.prototype.scrollIntoView).toHaveBeenCalled()
  })
  it('locks all mutations on a submitted request while preserving review navigation', async () => {
    open({ ...emptyReviewState(), verdict: 'ready' }, true)
    expect(byLabel<HTMLSelectElement>('Mark paragraph 1').disabled).toBe(true)
    expect(byLabel<HTMLButtonElement>('Comment on paragraph 1').disabled).toBe(true)
    expect(button('Changes requested').disabled).toBe(true)
    document.querySelector<HTMLButtonElement>('#review-comments-tab')!.click()
    await vi.waitFor(() => expect(document.querySelector('#review-comments')).not.toBeNull())
    expect(latest).toEqual(emptyReviewState())
  })
})
