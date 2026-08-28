import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { DEFAULT_SPEECH_HOTWORDS, mergeSpeechHotwords } from './speechHotwords'

const xAsr = 'x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05'
const senseVoice = 'sense-voice-zh-en-ja-ko-yue-2024-07-17'

function memoryStorage(initial: Record<string, string> = {}): Storage {
  const values = new Map(Object.entries(initial))
  return {
    get length() {
      return values.size
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => values.delete(key),
    setItem: (key, value) => values.set(key, value),
  }
}

async function loadPreferences(initial: Record<string, string> = {}) {
  vi.resetModules()
  vi.stubGlobal('localStorage', memoryStorage(initial))
  vi.stubGlobal('navigator', { language: 'en-US' })
  return import('./preferences')
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('speech model defaults', () => {
  it('defaults fresh installs to SenseVoice', async () => {
    const { speechModelId } = await loadPreferences()
    expect(get(speechModelId)).toBe(senseVoice)
  })

  it('migrates the persisted rc.7 X-ASR default once', async () => {
    const { speechModelId } = await loadPreferences({
      'rambledesk.speech.model': xAsr,
    })
    expect(get(speechModelId)).toBe(senseVoice)
  })

  it('preserves X-ASR when selected after the default migration', async () => {
    const { speechModelId } = await loadPreferences({
      'rambledesk.speech.model': xAsr,
      'rambledesk.speech.model-default-revision': '1',
    })
    expect(get(speechModelId)).toBe(xAsr)
  })
})

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
