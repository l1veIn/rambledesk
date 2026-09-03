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
  it('restores overlay visibility and opacity independently from confirmation', async () => {
    const defaults = await loadPreferences()
    expect(get(defaults.speechOverlayEnabled)).toBe(true)
    expect(get(defaults.speechOverlayOpacity)).toBe(95)
    const restored = await loadPreferences({
      'rambledesk.speech.overlay-enabled': 'false',
      'rambledesk.speech.overlay-opacity': '55',
      'rambledesk.speech.confirm-before-write': 'true',
    })
    expect(get(restored.speechOverlayEnabled)).toBe(false)
    expect(get(restored.speechOverlayOpacity)).toBe(55)
    expect(get(restored.speechConfirmBeforeWrite)).toBe(true)
  })

  it('keeps opacity readable and recovers from invalid saved values', async () => {
    const preferences = await loadPreferences({ 'rambledesk.speech.overlay-opacity': 'invalid' })
    expect(get(preferences.speechOverlayOpacity)).toBe(95)
    preferences.setSpeechOverlayOpacity(0)
    expect(get(preferences.speechOverlayOpacity)).toBe(30)
    preferences.setSpeechOverlayOpacity(150)
    expect(get(preferences.speechOverlayOpacity)).toBe(100)
    preferences.setSpeechOverlayOpacity(Number.NaN)
    expect(get(preferences.speechOverlayOpacity)).toBe(95)
  })
  it('requires an explicit opt-in for speech confirmation and restores it', async () => {
    const defaults = await loadPreferences()
    expect(get(defaults.speechConfirmBeforeWrite)).toBe(false)
    defaults.setSpeechConfirmBeforeWrite(true)
    expect(get(defaults.speechConfirmBeforeWrite)).toBe(true)
    const restored = await loadPreferences({ 'rambledesk.speech.confirm-before-write': 'true' })
    expect(get(restored.speechConfirmBeforeWrite)).toBe(true)
  })
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

describe('post-processing configuration', () => {
  it('distinguishes untidied speech by default and preserves an explicit opt-out', async () => {
    const defaults = await loadPreferences()
    expect(get(defaults.distinguishUntidiedText)).toBe(true)

    const optedOut = await loadPreferences({
      'rambledesk.tidy.distinguish-untidied-text': 'false',
    })
    expect(get(optedOut.distinguishUntidiedText)).toBe(false)
  })

  it('disables automatic Tidy by default and restores a saved threshold', async () => {
    const defaults = await loadPreferences()
    expect(get(defaults.tidyAutoThreshold)).toBe(0)

    const configured = await loadPreferences({
      'rambledesk.tidy.auto-threshold': '4',
    })
    expect(get(configured.tidyAutoThreshold)).toBe(4)
  })

  it('normalizes automatic Tidy thresholds set by the user', async () => {
    const preferences = await loadPreferences()
    preferences.setTidyAutoThreshold(-3)
    expect(get(preferences.tidyAutoThreshold)).toBe(0)
    preferences.setTidyAutoThreshold(5.7)
    expect(get(preferences.tidyAutoThreshold)).toBe(6)
  })

  it('keeps Tidy credentials independent from Cooking', async () => {
    const preferences = await loadPreferences({
      'rambledesk.cooking.api-key': 'cook-secret',
      'rambledesk.cooking.model': 'cook-model',
    })
    expect(get(preferences.cookingApiKey)).toBe('cook-secret')
    expect(get(preferences.cookingModel)).toBe('cook-model')
    expect(get(preferences.tidyApiKey)).toBe('')
    expect(get(preferences.tidyModel)).toBe('deepseek-v4-flash')
  })

  it('preserves the RC light-cleanup keys as the Tidy namespace', async () => {
    const preferences = await loadPreferences({
      'rambledesk.light-cleanup.provider': 'openai',
      'rambledesk.light-cleanup.api-key': 'tidy-secret',
      'rambledesk.light-cleanup.model': 'tidy-model',
    })
    expect(get(preferences.tidyProvider)).toBe('openai')
    expect(get(preferences.tidyApiKey)).toBe('tidy-secret')
    expect(get(preferences.tidyModel)).toBe('tidy-model')
    expect(get(preferences.cookingApiKey)).toBe('')
    expect(get(preferences.cookingModel)).toBe('deepseek-v4-flash')
  })

  it('changing one post-processing store does not mutate the other', async () => {
    const preferences = await loadPreferences()
    preferences.setTidyApiKey('tidy-only')
    preferences.setTidyModel('tidy-model')
    expect(get(preferences.cookingApiKey)).toBe('')
    expect(get(preferences.cookingModel)).toBe('deepseek-v4-flash')

    preferences.setCookingApiKey('cook-only')
    preferences.setCookingModel('cook-model')
    expect(get(preferences.tidyApiKey)).toBe('tidy-only')
    expect(get(preferences.tidyModel)).toBe('tidy-model')
  })
})
