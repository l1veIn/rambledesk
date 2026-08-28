import { describe, expect, it } from 'vitest'

import {
  DEFAULT_TIDY_SYSTEM_PROMPT,
  formatTidyPrompt,
  tidySpeechSegments,
} from './lightCleanup'

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

  it('accepts empty labeled results for filler-only segments', async () => {
    await expect(
      tidySpeechSegments(
        [
          { segmentId: 'a', text: '嗯，嗯嗯。' },
          { segmentId: 'b', text: '按钮没有反应' },
        ],
        {
          provider: 'openai',
          apiKey: 'k',
          baseUrl: '',
          model: 'm',
          reasoningEffort: 'low',
          locale: 'en',
        },
        async () => ({ text: '[1]\n\n[2] 按钮没有反应。', model: 'test' }),
      ),
    ).resolves.toEqual(['', '按钮没有反应。'])
  })

  it('tells the model not to delete meaningful short blocks', () => {
    expect(DEFAULT_TIDY_SYSTEM_PROMPT).toContain('Never delete a block merely because it is short')
    expect(DEFAULT_TIDY_SYSTEM_PROMPT).toContain('return its [n] label with an empty body')
  })

  it('uses the Tidy system prompt supplied by the independent configuration', async () => {
    let system = ''
    await tidySpeechSegments(
      [{ segmentId: 'a', text: '嗯按钮太小' }],
      {
        provider: 'openai',
        apiKey: 'tidy-key',
        baseUrl: 'https://tidy.example/v1',
        model: 'tidy-model',
        reasoningEffort: 'low',
        locale: 'en',
        systemPrompt: 'Tidy only; do not cook.',
      },
      async (request) => {
        system = request.system
        return { text: '[1] 按钮太小', model: 'test' }
      },
    )
    expect(system).toBe('Tidy only; do not cook.')
  })
})
