import { describe, expect, it } from 'vitest'

import { DEFAULT_COOKING_SYSTEM_PROMPT, resolveCookingSystemPrompt } from './cooking'

describe('resolveCookingSystemPrompt', () => {
  it('uses the built-in prompt when the custom one is empty', () => {
    expect(resolveCookingSystemPrompt('')).toBe(DEFAULT_COOKING_SYSTEM_PROMPT)
    expect(resolveCookingSystemPrompt('   ')).toBe(DEFAULT_COOKING_SYSTEM_PROMPT)
    expect(resolveCookingSystemPrompt(undefined)).toBe(DEFAULT_COOKING_SYSTEM_PROMPT)
  })

  it('asks cooking to keep the operator language', () => {
    expect(DEFAULT_COOKING_SYSTEM_PROMPT).toContain("Preserve the operator's language")
    expect(DEFAULT_COOKING_SYSTEM_PROMPT).toContain('write the cooked Markdown in Chinese')
  })

  it('keeps a custom prompt so operators can stop cooking from dropping images', () => {
    const custom = 'Keep every ![shot](attachment://abc) line. Do not invent facts.'
    expect(resolveCookingSystemPrompt(custom)).toBe(custom)
  })
})
