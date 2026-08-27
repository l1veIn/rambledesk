import { describe, expect, it } from 'vitest'

import {
  appendMarkdownBlock,
  formatTime,
  messageFrom,
  operatorFeedbackBody,
  replaceLastOccurrence,
} from './feedbackText'

describe('appendMarkdownBlock', () => {
  it('appends a block after a blank line', () => {
    expect(appendMarkdownBlock('hello', 'world')).toBe('hello\n\nworld')
  })

  it('returns the block alone for an empty body', () => {
    expect(appendMarkdownBlock('', 'world')).toBe('world')
  })
})

describe('replaceLastOccurrence', () => {
  it('replaces the spoken tail in place after a clipboard block', () => {
    const body = '我试一下复制粘贴啊。\n\n> Clipboard import\n\n> pasted'
    expect(replaceLastOccurrence(body, '我试一下复制粘贴啊。', '我试一下复制粘贴。')).toBe(
      '我试一下复制粘贴。\n\n> Clipboard import\n\n> pasted',
    )
  })
})

describe('operatorFeedbackBody', () => {
  const cooked = `# Title

## Operator Feedback

The operator saw a problem.

## Attachments

- [file](attachments/1.png)`

  it('extracts the Operator Feedback section and drops attachments', () => {
    expect(operatorFeedbackBody(cooked)).toBe('The operator saw a problem.')
  })

  it('returns arbitrary text unchanged', () => {
    expect(operatorFeedbackBody('plain notes')).toBe('plain notes')
  })
})

describe('formatTime', () => {
  it('renders a valid date in the given locale', () => {
    const date = new Date(2026, 0, 2, 3, 4, 5)
    expect(formatTime(date.toISOString(), 'en', 'not saved')).toMatch(/2026/)
  })

  it('uses the not-saved label for null', () => {
    expect(formatTime(null, 'en', 'not saved')).toBe('not saved')
  })
})

describe('messageFrom', () => {
  it('extracts an Error message', () => {
    expect(messageFrom(new Error('boom'))).toBe('boom')
  })

  it('extracts a command error message', () => {
    expect(messageFrom({ message: 'denied' })).toBe('denied')
  })

  it('stringifies unknown values', () => {
    expect(messageFrom(42)).toBe('42')
  })
})
