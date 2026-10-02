import { describe, expect, it } from 'vitest'
import type { DiffReviewAnchor, DiffReviewFile } from '../../generated/feedback'
import { anchorIncludesLine, diffAnchorOffset, diffFileLabel, diffFileMetadata, diffWindow, parseReviewDiff, validDiffAnchor } from './diffModel'

const diff = 'diff --git a/code.ts b/code.ts\n--- a/code.ts\n+++ b/code.ts\n@@ -10,3 +20,4 @@ function\n context\n---looks like a header\n+++looks like a header\n+inserted\n tail\n@@ -40 +51 @@\n-old\n+new\n'
const file: DiffReviewFile = { id: 'code', old_path: 'code.ts', new_path: 'code.ts', diff }
const anchor = (side: 'old' | 'new', start: number | null, end = start, hunk = 0): DiffReviewAnchor => ({ file_id: 'code', hunk_index: hunk, side, start_line: start, end_line: end })

describe('immutable unified diff model', () => {
  it('retains actual old/new coordinates, header-looking body lines and metadata', () => {
    const hunks = parseReviewDiff(diff)!
    expect(hunks.map((hunk) => [hunk.index, hunk.old, hunk.new])).toEqual([
      [0, { start: 10, count: 3 }, { start: 20, count: 4 }], [1, { start: 40, count: 1 }, { start: 51, count: 1 }],
    ])
    expect(hunks[0].lines.map((line) => [line.kind, line.oldLine, line.newLine])).toEqual([
      ['context', 10, 20], ['deletion', 11, null], ['addition', null, 21], ['addition', null, 22], ['context', 12, 23],
    ])
    expect(diffFileMetadata(diff).map((line) => line.text)).toEqual(['diff --git a/code.ts b/code.ts', '--- a/code.ts', '+++ b/code.ts'])
  })
  it('accepts CRLF and preserves no-newline metadata without moving line numbers', () => {
    const hunks = parseReviewDiff('@@ -1,2 +1,2 @@\r\n-old\r\n\\ No newline at end of file\r\n+new\r\n\\ No newline at end of file\r\n tail\r\n')!
    expect(hunks[0].lines.map((line) => [line.text, line.oldLine, line.newLine])).toEqual([
      ['-old', 1, null], ['\\ No newline at end of file', null, null], ['+new', null, 1],
      ['\\ No newline at end of file', null, null], [' tail', 2, 2],
    ])
  })
  it.each([
    '', 'Binary files differ', '@@ -1,2 +1 @@\n-old\n+new', '@@ -1 +1 @@\n-old\n+new\n+overflow',
    '@@ -1 +1 @@\nold\n+new', '@@ -0 +1 @@\n-old\n+new', '@@ -0,0 +0,0 @@',
    '@@ -4294967295 +1 @@\n-old\n+new', '@@ -1 +1 @@\n-old\n+new\n@@ -1 +2 @@\n-old\n+new',
    '@@ -2 +2 @@\n-old\n+new\n@@ -3 +2 @@\n-old\n+new',
    '\\ No newline at end of file\n@@ -1 +1 @@\n-old\n+new',
    '@@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n\\ No newline at end of file',
    '--- a\n--- b\n@@ -1 +1 @@\n-old\n+new',
    '@@ -1 +1 @@\n-old\n+new\ndiff --git a/next b/next\n@@ -5 +5 @@\n-old\n+new',
  ])('rejects malformed or multiple-file input %#', (value) => expect(parseReviewDiff(value)).toBeNull())
  it('limits line ranges to one side and hunk, while whole hunks can cover empty sides', () => {
    expect(validDiffAnchor(file, anchor('old', 10, 12))).toBe(true)
    expect(validDiffAnchor(file, anchor('new', 20, 23))).toBe(true)
    for (const bad of [anchor('old', 10, 13), anchor('new', 19, 23), anchor('old', 12, 10), anchor('new', null, 21), anchor('new', 51, 51, 0)]) {
      expect(validDiffAnchor(file, bad)).toBe(false)
    }
    const added = { ...file, old_path: '/dev/null', diff: '@@ -0,0 +1,2 @@\n+one\n+two' }
    expect(validDiffAnchor(added, anchor('old', 1))).toBe(false)
    expect(validDiffAnchor(added, anchor('old', null))).toBe(true)
    expect(validDiffAnchor(file, { ...anchor('new', 20), file_id: 'other' })).toBe(false)
    expect(diffFileLabel({ ...file, new_path: '/dev/null' })).toBe('code.ts')
    expect(anchorIncludesLine(anchor('old', null), 0, 'new', 21)).toBe(true)
    expect(anchorIncludesLine(anchor('old', 10, 12), 0, 'new', 21)).toBe(false)
  })
  it('finds stable source windows across hunk boundaries without renumbering anchors', () => {
    const hunks = parseReviewDiff(diff)!
    expect(diffAnchorOffset(hunks, anchor('new', 51, 51, 1))).toBe(6)
    const page = diffWindow(hunks, 4, 2)
    expect(page.map((hunk) => [hunk.index, hunk.lines.map((line) => line.text)])).toEqual([[0, [' tail']], [1, ['-old']]])
    expect(hunks[0].lines).toHaveLength(5)
  })
})
