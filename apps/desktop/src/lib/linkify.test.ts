import { describe, expect, it } from 'vitest'

import { isSafeHttpUrl, splitTextWithUrls } from './linkify'

describe('splitTextWithUrls', () => {
  it('keeps plain text unchanged', () => {
    expect(splitTextWithUrls('no links here')).toEqual([{ type: 'text', value: 'no links here' }])
  })

  it('extracts localhost and https URLs and leaves surrounding punctuation', () => {
    expect(splitTextWithUrls('打开 http://localhost:5173，然后看 https://example.com/docs.')).toEqual([
      { type: 'text', value: '打开 ' },
      { type: 'url', value: 'http://localhost:5173' },
      { type: 'text', value: '，然后看 ' },
      { type: 'url', value: 'https://example.com/docs' },
      { type: 'text', value: '.' },
    ])
  })

  it('rejects non-http schemes', () => {
    expect(isSafeHttpUrl('file:///tmp/x')).toBe(false)
    expect(isSafeHttpUrl('javascript:alert(1)')).toBe(false)
    expect(isSafeHttpUrl('http://localhost:5173')).toBe(true)
  })
})
