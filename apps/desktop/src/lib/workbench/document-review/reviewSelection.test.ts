// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { selectionAnchor } from './reviewSelection'

afterEach(() => { document.getSelection()?.removeAllRanges(); document.body.replaceChildren() })

describe('review selection anchors', () => {
  it('uses exact Unicode offsets through overlapping highlight elements', () => {
    document.body.innerHTML = '<section><p data-review-text="p1">开场<mark>😀很</mark>好。</p></section>'
    const root = document.querySelector('section')!
    const range = document.createRange()
    range.setStart(document.querySelector('mark')!.firstChild!, 0)
    range.setEnd(document.querySelector('p')!.lastChild!, 1)
    document.getSelection()!.addRange(range)
    expect(selectionAnchor(root, document.getSelection())).toEqual({ paragraph_id: 'p1', start: 2, end: 5, quote: '😀很好' })
  })
  it('explicitly rejects a range crossing paragraphs rather than attaching to the wrong one', () => {
    document.body.innerHTML = '<section><p data-review-text="p1">First</p><p data-review-text="p2">Second</p></section>'
    const range = document.createRange()
    range.setStart(document.querySelectorAll('p')[0].firstChild!, 0)
    range.setEnd(document.querySelectorAll('p')[1].firstChild!, 3)
    document.getSelection()!.addRange(range)
    expect(selectionAnchor(document.querySelector('section')!, document.getSelection())).toBe('cross_paragraph')
  })
})
