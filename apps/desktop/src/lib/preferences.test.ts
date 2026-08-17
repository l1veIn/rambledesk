import { describe, expect, it } from 'vitest'

import { DEFAULT_SPEECH_HOTWORDS, mergeSpeechHotwords } from './speechHotwords'

describe('speech hotword defaults', () => {
  it('includes product terms used in rambles', () => {
    expect(DEFAULT_SPEECH_HOTWORDS).toEqual(
      expect.arrayContaining(['ramble', 'RambleDesk', 'Rambelle', 'Cooking']),
    )
  })

  it('merges missing defaults without duplicating case-insensitively', () => {
    expect(mergeSpeechHotwords(['Claude Code', 'ramble'], ['ramble', 'RambleDesk', 'Rambelle'])).toEqual(
      ['Claude Code', 'ramble', 'RambleDesk', 'Rambelle'],
    )
  })

  it('leaves an already complete list unchanged', () => {
    const current = ['ramble', 'RambleDesk']
    expect(mergeSpeechHotwords(current, ['ramble', 'RambleDesk'])).toBe(current)
  })
})
