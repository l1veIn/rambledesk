import { describe, expect, it } from 'vitest'
import { rebaseFieldTextEdit } from './fieldTextEditing'

describe('field edits concurrent with external writeback', () => {
  it.each([
    ['A😀B', 'A😀中文B', 'A😀B\nVoice', 'A😀中文B\nVoice'],
    ['Old text', 'Old text!', 'New text', 'New text!'],
    ['Old text', 'My text', 'Your text', 'My text'],
    ['Draft', '', 'Draft\nVoice', '\nVoice'],
    ['', '中文', '\nVoice', '中文\nVoice'],
    ['a', 'ab', 'ab', 'ab'],
  ])('rebases %j → %j onto %j', (before, edited, latest, expected) => {
    expect(rebaseFieldTextEdit(before, edited, latest)).toBe(expected)
  })
})
