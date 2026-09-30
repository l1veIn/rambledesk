import { describe, expect, it } from 'vitest'
import type { WebReviewAnnotation, WebReviewData } from '../../generated/feedback'
import { capturedElementRect, commentPosition, emptyWebReviewState, hasWebReviewInput, prepareElementAnnotation, validateWebReviewState, validReviewUrl } from './reviewModel'

const data: WebReviewData = { title: 'Landing page', url: 'http://localhost:5173/', source_version: 'v1', viewport: { width: 1440, height: 900 } }
const note: WebReviewAnnotation = {
  id: 'note-1', page_url: data.url, viewport: data.viewport,
  element: { selector: '#hero button', tag_name: 'button', text: 'Try it 😀', rect: { x: 230, y: 400, width: 120, height: 42 } },
  body: 'Make this the primary action.',
}

describe('web review submission context', () => {
  it('allows overall feedback without element notes and blocks unfinished element comments', () => {
    expect(validateWebReviewState(data, null)).toBeNull()
    expect(validateWebReviewState(data, emptyWebReviewState())).toBeNull()
    expect(hasWebReviewInput(data, emptyWebReviewState())).toBe(false)
    const state = { ...emptyWebReviewState(), annotations: [note] }
    expect(hasWebReviewInput(data, state)).toBe(true)
    expect(validateWebReviewState(data, state)).toBeNull()
    expect(validateWebReviewState(data, { ...state, annotations: [{ ...note, body: ' \n ' }] })).toBe('Finish or remove empty comments.')
  })
  it('validates identifiers, bounded viewport geometry and recorded element metadata', () => {
    const invalid = [
      { ...note, id: 'Bad ID' }, { ...note, page_url: 'javascript:alert(1)' },
      { ...note, viewport: { width: 239, height: 900 } }, { ...note, viewport: { width: 1440, height: 4321 } },
      { ...note, element: { ...note.element, selector: '' } }, { ...note, element: { ...note.element, tag_name: 'BUTTON' } },
      { ...note, element: { ...note.element, rect: { ...note.element.rect, x: 0.5 } } },
      { ...note, element: { ...note.element, rect: { ...note.element.rect, width: 0 } } },
      { ...note, screenshot_attachment_id: '../attachment' },
    ]
    for (const item of invalid) expect(validateWebReviewState(data, { ...emptyWebReviewState(), annotations: [item] })).not.toBeNull()
    expect(validateWebReviewState(data, { ...emptyWebReviewState(), annotations: [note, note] })).not.toBeNull()
    expect(validateWebReviewState(data, { ...emptyWebReviewState(), annotations: [{ ...note, screenshot_attachment_id: 'image-1' }] })).toBeNull()
  })
  it('counts Unicode scalar characters and rejects malformed strings', () => {
    const state = { ...emptyWebReviewState(), annotations: [{ ...note, body: '😀'.repeat(4000) }] }
    expect(validateWebReviewState(data, state)).toBeNull()
    for (const body of ['😀'.repeat(4001), 'Invalid\0text', '\ud800']) {
      expect(validateWebReviewState(data, { ...state, annotations: [{ ...note, body }] })).not.toBeNull()
    }
  })
  it('accepts local development URLs and rejects credentials, ambiguous schemes and controls', () => {
    for (const url of ['http://localhost:5173/', 'http://127.0.0.1:3000/about?test=1#hero', 'https://example.com/']) expect(validReviewUrl(url)).toBe(true)
    for (const url of ['https://user:pass@example.com/', 'file:///page.html', '//example.com', 'https://exa mple.com', 'https://example.com/\n', 'https:\\example.com']) expect(validReviewUrl(url)).toBe(false)
  })
})

describe('ongoing element comments', () => {
  it('creates an immutable captured anchor and reopens the same element without losing prose or attachments', () => {
    const { body: _, id: __, ...anchor } = note
    const first = prepareElementAnnotation(emptyWebReviewState(), anchor, () => 'first')!
    expect(first.created).toBe(true)
    expect(first.annotation.body).toBe('')
    const written = { ...first.annotation, body: 'Original thought\n[image](attachment://image-1)' }
    const state = { ...first.state, annotations: [written] }
    const moved = { ...anchor, element: { ...anchor.element, text: 'Updated text', rect: { x: 20, y: 40, width: 100, height: 50 } } }
    const next = prepareElementAnnotation(state, moved, () => { throw new Error('must reuse ID') })!
    expect(next.state).toBe(state)
    expect(next.annotation).toBe(written)
    expect(next.annotation.element).toEqual(anchor.element)
    expect(note.body).toBe('Make this the primary action.')
  })
  it('keeps separate notes for different pages and responsive viewports', () => {
    const state = { ...emptyWebReviewState(), annotations: [note] }
    const mobile = prepareElementAnnotation(state, { ...note, viewport: { width: 390, height: 844 } }, () => 'mobile')!
    expect(mobile.created).toBe(true)
    expect(mobile.state.annotations).toHaveLength(2)
    const anotherPage = prepareElementAnnotation(state, { ...note, page_url: `${data.url}about` }, () => 'about')!
    expect(anotherPage.created).toBe(true)
    expect(anotherPage.annotation.page_url).toBe(`${data.url}about`)
  })
  it('continues selected historic notes at the limit without allowing another element', () => {
    const annotations = Array.from({ length: 500 }, (_, index) => ({ ...note, id: `note-${index}` }))
    const state = { ...emptyWebReviewState(), annotations }
    const reused = prepareElementAnnotation(state, note, () => 'unused', 'note-42')!
    expect(reused.annotation.id).toBe('note-42')
    expect(reused.state).toBe(state)
    expect(prepareElementAnnotation(state, { ...note, element: { ...note.element, selector: '#footer' } }, () => 'overflow')).toBeNull()
  })
  it('places inline editors within the visible stage while leaving the reviewed viewport unchanged', () => {
    expect(commentPosition({ x: 30, y: 60, width: 80, height: 40 }, 1000, 600)).toEqual({ left: 122, top: 60 })
    expect(commentPosition({ x: 950, y: 590, width: 80, height: 40 }, 1000, 600)).toEqual({ left: 594, top: 400 })
    expect(commentPosition(null, 0, 0)).toEqual({ left: 12, top: 12 })
    expect(data.viewport).toEqual({ width: 1440, height: 900 })
  })
  it('captures document coordinates independently of the scrolling inline editor anchor', () => {
    const rect = { x: 170.25, y: 940.5, width: 120.4, height: 50.3 }
    expect(capturedElementRect(rect)).toEqual({ x: 170, y: 941, width: 120, height: 50 })
    expect(rect).toEqual({ x: 170.25, y: 940.5, width: 120.4, height: 50.3 })
  })
})
