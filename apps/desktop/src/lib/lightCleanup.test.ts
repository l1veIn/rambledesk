import { describe, expect, it, vi } from 'vitest'

import {
  DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT,
  lightCleanupTranscript,
  resolveLightCleanupSystemPrompt,
  type LightCleanupConfig,
} from './lightCleanup'

const config: LightCleanupConfig = {
  provider: 'deepseek',
  apiKey: 'sk-test',
  baseUrl: 'https://api.deepseek.com/v1',
  model: 'deepseek-v4-flash',
  reasoningEffort: 'none',
  locale: 'en',
}

describe('resolveLightCleanupSystemPrompt', () => {
  it('uses the built-in prompt when the custom one is empty', () => {
    expect(resolveLightCleanupSystemPrompt('')).toBe(DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT)
    expect(resolveLightCleanupSystemPrompt('   ')).toBe(DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT)
    expect(resolveLightCleanupSystemPrompt(undefined)).toBe(DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT)
  })

  it('keeps a custom prompt', () => {
    expect(resolveLightCleanupSystemPrompt('Just fix punctuation.')).toBe('Just fix punctuation.')
  })

  it('asks only for light spoken-text cleanup, not cooking', () => {
    expect(DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT).toContain('filler')
    expect(DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT).toContain('比如说')
    expect(DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT).toMatch(/punctuation|sentence break/i)
    expect(DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT).not.toContain('headings')
    expect(DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT).toContain('Do not summarize')
  })
})

describe('lightCleanupTranscript', () => {
  it('returns empty input unchanged without calling the model', async () => {
    const generate = vi.fn()
    await expect(lightCleanupTranscript('  ', config, generate)).resolves.toBe('')
    expect(generate).not.toHaveBeenCalled()
  })

  it('sends the transcript to the model and returns the cleaned text', async () => {
    const generate = vi.fn(async () => ({
      text: 'The button is too small.',
      model: 'deepseek/deepseek-v4-flash',
    }))
    await expect(
      lightCleanupTranscript('啊那个 button 太小了', config, generate),
    ).resolves.toBe('The button is too small.')
    expect(generate).toHaveBeenCalledWith(
      expect.objectContaining({
        system: DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT,
        prompt: '啊那个 button 太小了',
      }),
    )
  })

  it('keeps the original transcript when the model returns empty text', async () => {
    const generate = vi.fn(async () => ({ text: '  ', model: 'deepseek/x' }))
    await expect(lightCleanupTranscript('keep me', config, generate)).resolves.toBe('keep me')
  })

  it('rejects when no API key is configured', async () => {
    await expect(
      lightCleanupTranscript('hello', { ...config, apiKey: '  ' }, vi.fn()),
    ).rejects.toThrow(/API key/)
  })
})
