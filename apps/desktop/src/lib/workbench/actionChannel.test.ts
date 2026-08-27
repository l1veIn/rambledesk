import { describe, expect, it } from 'vitest'

import {
  actionChannelSeparator,
  joinActionChannelMarkdown,
  migrateActionChannelSeparators,
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

describe('legacy markdown with action dividers', () => {
  const doc = (content: Parameters<typeof migrateActionChannelSeparators>[0]['content']) =>
    ({ type: 'doc', content }) as ReturnType<typeof migrateActionChannelSeparators>

  it('drops the divider and stamps the blocks that follow it', () => {
    const migrated = migrateActionChannelSeparators(
      doc([
        { type: 'paragraph', content: [{ type: 'text', text: '先说结论。' }] },
        {
          type: 'paragraph',
          content: [{ type: 'text', text: '------------------------ Action 2 ------------------------' }],
        },
        { type: 'paragraph', content: [{ type: 'text', text: '保存没有 toast。' }] },
        { type: 'paragraph', content: [{ type: 'text', text: '截图属于同一个 Action。' }] },
        {
          type: 'paragraph',
          content: [{ type: 'text', text: '------------------------ Action 3 ------------------------' }],
        },
        { type: 'paragraph', content: [{ type: 'text', text: '返回列表很好。' }] },
      ]),
    )
    expect(migrated.content?.length).toBe(4)
    expect(migrated.content?.[0]?.attrs?.actionIndex).toBeUndefined()
    expect(migrated.content?.[1]?.attrs?.actionIndex).toBe(2)
    expect(migrated.content?.[2]?.attrs?.actionIndex).toBe(2)
    expect(migrated.content?.[3]?.attrs?.actionIndex).toBe(3)
    expect(
      JSON.stringify(migrated.content?.[1]?.content?.[0]?.text),
    ).toContain('保存没有 toast')
  })

  it('keeps an existing stamp and resets the channel to the default on plain dashes', () => {
    const migrated = migrateActionChannelSeparators(
      doc([
        {
          type: 'paragraph',
          attrs: { actionIndex: 2 },
          content: [{ type: 'text', text: '已经盖章的内容。' }],
        },
        {
          type: 'paragraph',
          content: [{ type: 'text', text: '------------------------------------------------' }],
        },
        { type: 'paragraph', content: [{ type: 'text', text: '回到默认频道。' }] },
      ]),
    )
    expect(migrated.content?.length).toBe(2)
    expect(migrated.content?.[0]?.attrs?.actionIndex).toBe(2)
    expect(migrated.content?.[1]?.attrs?.actionIndex).toBeUndefined()
  })

  it('leaves normal paragraphs untouched', () => {
    const migrated = migrateActionChannelSeparators(
      doc([{ type: 'paragraph', content: [{ type: 'text', text: '普通内容。' }] }]),
    )
    expect(migrated.content?.length).toBe(1)
    expect(migrated.content?.[0]?.attrs?.actionIndex).toBeUndefined()
  })
})
