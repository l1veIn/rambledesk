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

describe('light cleanup defaults', () => {
  it('defaults light cleanup to off so it can be enabled without cooking', async () => {
    const {
      lightCleanupCharThreshold,
      lightCleanupEnabled,
      lightCleanupBaseUrl,
      lightCleanupIdleMs,
      lightCleanupModel,
      lightCleanupProvider,
      lightCleanupReasoningEffort,
      lightCleanupSegmentThreshold,
      lightCleanupTimeoutMs,
    } = await loadPreferences()
    expect(get(lightCleanupEnabled)).toBe(false)
    expect(get(lightCleanupProvider)).toBe('deepseek')
    expect(get(lightCleanupBaseUrl)).toBe('https://api.deepseek.com/v1')
    expect(get(lightCleanupModel)).toBe('deepseek-v4-flash')
    expect(get(lightCleanupSegmentThreshold)).toBe(3)
    expect(get(lightCleanupCharThreshold)).toBe(500)
    expect(get(lightCleanupIdleMs)).toBe(30_000)
    expect(get(lightCleanupTimeoutMs)).toBe(30_000)
    expect(get(lightCleanupReasoningEffort)).toBe('none')
  })

  it('defaults Cooking to the same fast non-reasoning model independently', async () => {
    const { cookingModel, cookingProvider, cookingReasoningEffort } = await loadPreferences()
    expect(get(cookingProvider)).toBe('deepseek')
    expect(get(cookingModel)).toBe('deepseek-v4-flash')
    expect(get(cookingReasoningEffort)).toBe('none')
  })

  it('seeds cleanup from an existing Cooking connection once, then keeps both stores independent', async () => {
    const preferences = await loadPreferences({
      'rambledesk.cooking.provider': 'compatible',
      'rambledesk.cooking.api-key': 'legacy-key',
      'rambledesk.cooking.base-url': 'https://legacy.example/v1',
      'rambledesk.cooking.model': 'legacy-model',
    })

    expect(get(preferences.lightCleanupProvider)).toBe('compatible')
    expect(get(preferences.lightCleanupApiKey)).toBe('legacy-key')
    expect(get(preferences.lightCleanupBaseUrl)).toBe('https://legacy.example/v1')
    expect(get(preferences.lightCleanupModel)).toBe('legacy-model')

    preferences.setCookingProvider('openai')
    preferences.setCookingApiKey('cooking-key')
    preferences.setCookingModel('cooking-model')
    expect(get(preferences.lightCleanupProvider)).toBe('compatible')
    expect(get(preferences.lightCleanupApiKey)).toBe('legacy-key')
    expect(get(preferences.lightCleanupModel)).toBe('legacy-model')

    preferences.setLightCleanupProvider('deepseek')
    preferences.setLightCleanupApiKey('cleanup-key')
    preferences.setLightCleanupModel('cleanup-model')
    expect(get(preferences.cookingProvider)).toBe('openai')
    expect(get(preferences.cookingApiKey)).toBe('cooking-key')
    expect(get(preferences.cookingModel)).toBe('cooking-model')
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
