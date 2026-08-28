import { describe, expect, it } from 'vitest'

import { formatTidyPrompt, tidySpeechSegments } from './lightCleanup'

describe('tidySpeechSegments', () => {
  it('formats every input block with a [n] label', () => {
    expect(
      formatTidyPrompt([
        { segmentId: 'a', text: '第一段' },
        { segmentId: 'b', text: '第二段' },
      ]),
    ).toBe('[1] 第一段\n\n[2] 第二段')
  })

  it('returns null when the model skips labels', async () => {
    await expect(
      tidySpeechSegments([{ segmentId: 'a', text: '按钮太小' }], {
        provider: 'openai',
        apiKey: 'k',
        baseUrl: '',
        model: 'm',
        reasoningEffort: 'low',
        locale: 'en',
      }, async () => ({ text: '按钮太小了。', model: 'test' })),
    ).resolves.toBeNull()
  })

  it('accepts a strict one-to-one labeled result', async () => {
    await expect(
      tidySpeechSegments(
        [
          { segmentId: 'a', text: '按钮太小' },
          { segmentId: 'b', text: '没有 toast' },
        ],
        {
          provider: 'openai',
          apiKey: 'k',
          baseUrl: '',
          model: 'm',
          reasoningEffort: 'low',
          locale: 'en',
        },
        async () => ({ text: '[1] 按钮太小了。\n\n[2] 没有 toast。', model: 'test' }),
      ),
    ).resolves.toEqual(['按钮太小了。', '没有 toast。'])
  })
})
