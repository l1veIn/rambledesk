import { describe, expect, it } from 'vitest'
import { suggestionDiff } from './reviewSuggestion'

describe('literal suggestion preview', () => {
  it.each([['Hello 😀 friend', 'Hello 👋 friend'], ['中间文字', '中新文字'], ['remove', ''], ['', 'added'], ['unchanged', 'unchanged']])('reconstructs both immutable sides: %s', (before, after) => {
    const diff = suggestionDiff(before, after)
    expect(diff.prefix + diff.removed + diff.suffix).toBe(before)
    expect(diff.prefix + diff.added + diff.suffix).toBe(after)
    expect(diff.removed + diff.added).not.toMatch(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u)
  })
})
