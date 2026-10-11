import { describe, expect, it } from 'vitest'
import { highlightedLeaf, leafBoundary, parseReviewMarkdown } from './reviewMarkdown'
import type { ReviewAnnotation } from '../../generated/feedback'

describe('Markdown source maps', () => {
  it.each([
    ['# 标题 😀\r\n\r\n**bold** &amp; \\*', ['标题 😀', 'bold', ' & *']],
    ['> a\r\n> b\r\n\r\n- list\n  continued', ['a\r\nb', 'list\ncontinued']],
    ['| a | b |\n|---|---|\n| x\\|y | z |', ['a', 'b', 'x|y', 'z']],
    ['```js\r\na\r\nb\r\n```', ['a\r\nb']],
    ['` a\r\nb `', ['a b']],
    ['[link](https://example.com) and [link](https://example.org)', ['link', ' and ', 'link']],
    ['- [x] complete\n- [ ] pending', ['complete', 'pending']],
  ] as const)('maps rendered leaves from %s', (source, expected) => {
    const result = parseReviewMarkdown(source)
    expect([...result.leaves.values()].map((leaf) => leaf.text)).toEqual(expected)
    expect(result.fallback).toBe(false)
    for (const leaf of result.leaves.values()) for (const unit of leaf.units) {
      expect(unit.start).toBeGreaterThanOrEqual(0)
      expect(unit.end).toBeGreaterThan(unit.start)
      expect([...source].slice(unit.start, unit.end).join('')).not.toBe('')
    }
  })
  it('maps decoded entities and escapes to their complete raw spelling, refusing partial entities', () => {
    const source = '中😀 &amp; \\* &NotEqualTilde;'
    const leaf = [...parseReviewMarkdown(source).leaves.values()][0]
    expect(leaf.units.find((unit) => unit.text === '&')).toEqual({ text: '&', start: 3, end: 8 })
    expect(leaf.units.find((unit) => unit.text === '*')).toEqual({ text: '*', start: 9, end: 11 })
    expect(leafBoundary(leaf, 2, 'end')).toBeNull() // inside the emoji surrogate pair
    expect(leafBoundary(leaf, leaf.text.length - 1, 'end')).toBeNull() // inside a two-character entity
  })
  it('distinguishes repeated text by parser position and keeps overlap text exactly once', () => {
    const result = parseReviewMarkdown('repeat **repeat** repeat')
    const leaf = [...result.leaves.values()][1]
    expect(leaf.units[0].start).toBe(9)
    const note = (id: string, start: number, end: number): ReviewAnnotation => ({ id, start, end,
      quote: [...result.markdown].slice(start, end).join(''), kind: 'comment', body: 'Note', replacement: null })
    const segments = highlightedLeaf(leaf, [note('a', 9, 13), note('b', 11, 15)])
    expect(segments).toEqual([{ text: 're', annotationIds: ['a'] }, { text: 'pe', annotationIds: ['a', 'b'] }, { text: 'at', annotationIds: ['b'] }])
  })
  it('renders raw HTML and an unsupported mapping literally with exact source positions', () => {
    const source = '<script>alert(1)</script>\n\n| Code |\n|---|\n| `x\\|y` |'
    const result = parseReviewMarkdown(source)
    expect(result.nodes[0].tag).toBe('span')
    expect([...result.leaves.values()][0]).toMatchObject({ text: '<script>alert(1)</script>', fallback: true })
    for (const leaf of result.leaves.values()) if (leaf.fallback) {
      expect(leaf.text).toBe([...source].slice(leaf.units[0].start, leaf.units.at(-1)!.end).join(''))
    }
  })
  it('parses a substantial document below the render-node budget without dropping text', () => {
    const source = Array.from({ length: 1000 }, (_, i) => `**${i}** word\n\n`).join('')
    const result = parseReviewMarkdown(source)
    expect(result.fallback).toBe(false)
    expect(result.leaves.size).toBe(2000)
    expect(result.leaves.get('source-1998')?.text).toBe('999')
  })
  it('preserves the complete source with selectable scalar boundaries when formatting exceeds the render budget', () => {
    const source = '**a** *b* '.repeat(2_100)
    const result = parseReviewMarkdown(source)
    expect(result.fallback).toBe(true)
    expect(result.nodes).toHaveLength(1)
    expect(result.leaves.size).toBe(1)
    const leaf = result.leaves.get('source-0')!
    expect(leaf.text).toBe(source)
    expect(leafBoundary(leaf, 6, 'start')).toBe(6)
    expect(leafBoundary(leaf, source.length, 'end')).toBe([...source].length)
  })
})
