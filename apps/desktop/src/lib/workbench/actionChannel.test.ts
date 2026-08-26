import { describe, expect, it } from 'vitest'

import {
  actionChannelSeparator,
  joinActionChannelMarkdown,
  serializeDocWithActionChannels,
} from './actionChannel'

describe('action channel markdown export', () => {
  it('renders action and default channels as readable dividers', () => {
    expect(actionChannelSeparator(2)).toBe(
      '------------------------ Action 2 ------------------------',
    )
    expect(actionChannelSeparator(null)).toBe(
      '------------------------------------------------',
    )
  })

  it('writes a divider only when the channel changes', () => {
    expect(
      joinActionChannelMarkdown([
        { actionIndex: null, markdown: '整体先说两句。' },
        { actionIndex: 2, markdown: '保存之后没有 toast。' },
        { actionIndex: 2, markdown: '截图也属于同一个 Action。' },
        { actionIndex: 3, markdown: '返回列表是好的。' },
        { actionIndex: null, markdown: '最后补充整体感受。' },
      ]),
    ).toBe(
      [
        '整体先说两句。',
        '------------------------ Action 2 ------------------------',
        '保存之后没有 toast。',
        '截图也属于同一个 Action。',
        '------------------------ Action 3 ------------------------',
        '返回列表是好的。',
        '------------------------------------------------',
        '最后补充整体感受。',
      ].join('\n\n'),
    )
  })

  it('exports node attrs without defining a reverse Markdown protocol', () => {
    expect(
      serializeDocWithActionChannels(
        {
          type: 'doc',
          content: [
            {
              type: 'paragraph',
              attrs: { actionIndex: 2 },
              content: [{ type: 'text', text: '保存失败。' }],
            },
          ],
        },
        (doc) => doc.content?.[0].content?.[0].text ?? '',
      ),
    ).toBe(
      '------------------------ Action 2 ------------------------\n\n保存失败。',
    )
  })
})
