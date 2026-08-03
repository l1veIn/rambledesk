import { describe, expect, it } from 'vitest'

import {
  speechModelDescription,
  speechModelDisplayName,
  speechModelLanguages,
} from './speechModelLabels'

const senseVoice = 'sense-voice-zh-en-ja-ko-yue-2024-07-17' as const

describe('speech model labels', () => {
  it('uses localized English copy for known models', () => {
    expect(speechModelDisplayName('en', senseVoice, 'SenseVoice 多语言')).toBe(
      'SenseVoice multilingual',
    )
    expect(
      speechModelDescription('en', senseVoice, 'VAD 自动分段后整段识别，兼顾多语言准确率'),
    ).toContain('multilingual accuracy')
    expect(speechModelLanguages('en', senseVoice, ['中文', 'English', '日本語'])).toEqual([
      'Chinese',
      'English',
      'Japanese',
    ])
  })

  it('keeps the backend-provided Chinese copy in Chinese mode', () => {
    expect(speechModelDisplayName('zh-CN', senseVoice, 'SenseVoice 多语言')).toBe(
      'SenseVoice 多语言',
    )
  })
})
