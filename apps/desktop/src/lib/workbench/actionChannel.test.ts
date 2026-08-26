import { describe, expect, it } from 'vitest'

import {
  appendActionChannelBlock,
  joinActionChannelMarkdown,
  parseActionChannelLine,
  parseDocWithActionChannels,
  serializeDocWithActionChannels,
  splitActionChannelMarkdown,
} from './actionChannel'

describe('action channel markdown', () => {
  it('treats only exact marker lines as channel switches', () => {
    expect(parseActionChannelLine('@ Action 2')).toBe(2)
    expect(parseActionChannelLine(' @ ')).toBe(null)
    expect(parseActionChannelLine('@ Action 2 保存之后没有 toast。')).toBeUndefined()
    expect(parseActionChannelLine('email @ Action 2')).toBeUndefined()
  })

  it('round-trips default, action, and return-to-default runs', () => {
    const source = [
      '整体先说两句。',
      '',
      '@ Action 2',
      '保存之后没有 toast。',
      '',
      '![shot.png](attachment://abc-123)',
      '',
      '@ Action 3',
      '返回列表是好的。',
      '',
      '@',
      '其实 toast 在右下角。',
    ].join('\n')

    expect(splitActionChannelMarkdown(source)).toEqual([
      { actionIndex: null, markdown: '整体先说两句。' },
      { actionIndex: 2, markdown: '保存之后没有 toast。\n\n![shot.png](attachment://abc-123)' },
      { actionIndex: 3, markdown: '返回列表是好的。' },
      { actionIndex: null, markdown: '其实 toast 在右下角。' },
    ])
    expect(joinActionChannelMarkdown(splitActionChannelMarkdown(source))).toBe(
      [
        '整体先说两句。',
        '@ Action 2',
        '保存之后没有 toast。\n\n![shot.png](attachment://abc-123)',
        '@ Action 3',
        '返回列表是好的。',
        '@',
        '其实 toast 在右下角。',
      ].join('\n\n'),
    )
  })

  it('does not emit a marker while staying on the same action', () => {
    expect(
      appendActionChannelBlock('Hello\n\n@ Action 2\n\n第一句。', '第二句。', 2),
    ).toBe('Hello\n\n@ Action 2\n\n第一句。\n\n第二句。')
  })

  it('stamps parsed blocks and serializes markers back out', () => {
    const markdown = '开场。\n\n@ Action 2\n\n保存失败。'
    const parsed = parseDocWithActionChannels(markdown, (source) => ({
      type: 'doc',
      content: source.split(/\n{2,}/).filter(Boolean).map((text) => ({
        type: 'paragraph',
        content: [{ type: 'text', text }],
      })),
    }))
    expect(parsed.content?.map((node) => node.attrs?.actionIndex ?? null)).toEqual([null, 2])
    expect(
      serializeDocWithActionChannels(parsed, (doc) =>
        (doc.content ?? [])
          .map((node) => node.content?.[0]?.text ?? '')
          .join('\n\n'),
      ),
    ).toBe('开场。\n\n@ Action 2\n\n保存失败。')
  })
})
