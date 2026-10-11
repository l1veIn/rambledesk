// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { selectionAnchor } from './reviewSelection'
import { parseReviewMarkdown } from './reviewMarkdown'
import ReviewMarkdown from './ReviewMarkdown.svelte'
import ReviewMarkdownNode from './ReviewMarkdownNode.svelte'

let view: ReturnType<typeof mount> | undefined
afterEach(async () => { if (view) await unmount(view); view = undefined; document.getSelection()?.removeAllRanges(); document.body.replaceChildren() })
function render(source: string) {
  const model = parseReviewMarkdown(source)
  view = mount(ReviewMarkdown, { target: document.body, props: { document: model, annotations: [], selectedId: null, onSelect: () => {} } })
  return model
}
function select(start: Node, from: number, end = start, to = start.textContent!.length) {
  const range = document.createRange(); range.setStart(start, from); range.setEnd(end, to)
  document.getSelection()!.removeAllRanges(); document.getSelection()!.addRange(range)
}
const leaf = (id: number) => document.querySelector(`[data-review-leaf="source-${id}"]`)!
const textNode = (id: number) => document.createTreeWalker(leaf(id), NodeFilter.SHOW_TEXT).nextNode()!

describe('rendered DOM to raw source anchors', () => {
  it.each([
    ['before **bold** after', 0, 0, 1, 4, 'before **bold**'],
    ['before **bold** after', 1, 0, 2, 6, '**bold** after'],
    ['before **bold** after', 1, 0, 1, 4, 'bold'],
    ['before [label](https://example.com) after', 0, 0, 1, 5, 'before [label](https://example.com)'],
    ['before **nested _word_** after', 0, 0, 2, 4, 'before **nested _word_**'],
    ['before `code` after', 0, 0, 1, 4, 'before `code`'],
    ['before **bold** after', 0, 0, 1, 2, 'before **bo'],
  ] as const)('completes only fully selected inline syntax in %s', (source, fromLeaf, from, toLeaf, to, quote) => {
    const model = render(source)
    select(textNode(fromLeaf), from, textNode(toLeaf), to)
    const result = selectionAnchor(document.body, document.getSelection(), model)
    expect(result).toMatchObject({ quote })
    if (result && result !== 'unmapped') expect([...source].slice(result.start!, result.end!).join('')).toBe(quote)
  })
  it('supports reverse cross-block selections containing markup, CRLF and emoji', () => {
    const source = '# 中文😀 **bold**\r\n\r\n- next **line**'
    const model = render(source)
    select(textNode(0), 2, textNode(3), 2)
    const expected = '😀 **bold**\r\n\r\n- next **li'
    expect(selectionAnchor(document.body, document.getSelection(), model)).toEqual({ start: 4, end: 4 + [...expected].length, quote: expected })
    document.getSelection()!.setBaseAndExtent(textNode(3), 2, textNode(0), 2)
    expect(selectionAnchor(document.body, document.getSelection(), model)).toEqual({ start: 4, end: 4 + [...expected].length, quote: expected })
  })
  it('uses exact entity/escape sources and rejects a surrogate half', () => {
    const model = render('中😀 &amp; \\*')
    select(textNode(0), 4, textNode(0), 7)
    expect(selectionAnchor(document.body, document.getSelection(), model)).toEqual({ start: 3, end: 11, quote: '&amp; \\*' })
    select(textNode(0), 2, textNode(0), 3)
    expect(selectionAnchor(document.body, document.getSelection(), model)).toBe('unmapped')
  })
  it('accepts container boundaries without including adjacent blocks', () => {
    const model = render('first\n\nsecond\n\nthird')
    const root = document.querySelector('[data-review-text]')!
    const range = document.createRange(); range.selectNodeContents(root.children[1])
    document.getSelection()!.addRange(range)
    expect(selectionAnchor(document.body, document.getSelection(), model)).toEqual({ start: 7, end: 13, quote: 'second' })
  })
  it('keeps safe table/code rendering and anchors across their block boundary', () => {
    const source = '| a | b |\n|---|---|\n| x | y |\n\n```ts\nconst value = 1\n```'
    const model = render(source)
    expect(document.querySelectorAll('th')).toHaveLength(2)
    expect(document.querySelectorAll('td')).toHaveLength(2)
    select(textNode(2), 0, textNode(4), 5)
    const quote = 'x | y |\n\n```ts\nconst'
    const start = [...source.slice(0, source.indexOf('x'))].length
    expect(selectionAnchor(document.body, document.getSelection(), model)).toEqual({ start, end: start + [...quote].length, quote })
  })
  it('does not execute HTML, load images or open unsafe URLs, and rejects placeholder selections', () => {
    const model = render('before ![diagram](https://example.com/image.png) after\n\n[unsafe](javascript:alert%281%29)\n\n<script>alert(1)</script>')
    expect(document.querySelector('img')).toBeNull()
    expect(document.querySelector('script')).toBeNull()
    expect(document.querySelector('a')?.hasAttribute('href')).toBe(false)
    expect(document.body.textContent).toContain('<script>alert(1)</script>')
    select(textNode(0), 0, textNode(1), 3)
    expect(selectionAnchor(document.body, document.getSelection(), model)).toBe('unmapped')
  })
  it('opens an annotated link as a comment without navigation, while ordinary links use the opener', () => {
    const model = parseReviewMarkdown('[noted](https://example.com) [plain](https://example.org)')
    const onSelect = vi.fn(), onOpenLink = vi.fn()
    view = mount(ReviewMarkdownNode, { target: document.body, props: { node: model.nodes[0], selectedId: null, onSelect, onOpenLink,
      annotations: [{ id: 'note', start: 1, end: 6, quote: 'noted', kind: 'comment', body: 'Review link', replacement: null }] } })
    expect(document.querySelector<HTMLElement>('[data-annotation-ids]')!.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))).toBe(false)
    expect(onSelect).toHaveBeenCalledWith('note')
    expect(onOpenLink).not.toHaveBeenCalled()
    document.querySelectorAll<HTMLAnchorElement>('a')[1].click()
    expect(onOpenLink).toHaveBeenCalledExactlyOnceWith('https://example.org')
    expect(document.querySelector('p')?.textContent).toBe('noted plain')
  })
})
