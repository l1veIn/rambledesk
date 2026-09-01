import { describe, expect, it } from 'vitest'
import type { SpeechModelInfo } from './workbenchCapabilities'
import { resolveSupportedSpeechModelId } from './speechModelSelection'

const xAsr = 'x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05' as const
const senseVoice = 'sense-voice-zh-en-ja-ko-yue-2024-07-17' as const
const model = (id: SpeechModelInfo['id']) => ({ id }) as SpeechModelInfo

describe('platform speech model selection', () => {
  it('migrates a stale Desktop default only when the platform has one explicit model', () => {
    expect(resolveSupportedSpeechModelId(senseVoice, [model(xAsr)])).toBe(xAsr)
  })

  it('never guesses when several platform models remain possible', () => {
    expect(resolveSupportedSpeechModelId(senseVoice, [model(xAsr), model(senseVoice)])).toBe(senseVoice)
    expect(resolveSupportedSpeechModelId(senseVoice, [model(xAsr), model('funasr-nano-int8-2025-12-30')])).toBeNull()
  })
})
