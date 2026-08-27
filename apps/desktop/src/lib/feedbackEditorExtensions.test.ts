import { describe, expect, it } from 'vitest'
import { MarkdownManager } from '@tiptap/markdown'
import type { JSONContent } from '@tiptap/core'

import {
  feedbackEditorExtensions,
  parseFeedbackMarkdown,
  serializeFeedbackMarkdown,
} from './feedbackEditorExtensions'
import {
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
} from './speechBlockMetadata'

function markdown() {
  return new MarkdownManager({ extensions: feedbackEditorExtensions() })
}

/** Markdown → document → markdown → document; the second document must match. */
function stableRoundTrip(source: string): JSONContent {
  const manager = markdown()
  const parsed = manager.parse(source)
  expect(manager.parse(manager.serialize(parsed))).toEqual(parsed)
  return parsed
}

function blockTypes(doc: JSONContent): string[] {
  return (doc.content ?? []).map((node) => node.type ?? '')
}

function cellTexts(row: JSONContent): string[] {
  return (row.content ?? []).map(
    (cell) => cell.content?.[0]?.content?.[0]?.text ?? '',
  )
}

const TABLE = [
  '| Area | Verdict |',
  '| --- | --- |',
  '| Ramble | smooth |',
  '| Table | readable |',
].join('\n')

describe('feedback editor markdown tables', () => {
  it('parses a markdown table into table nodes instead of dropping it', () => {
    const doc = stableRoundTrip(TABLE)

    expect(blockTypes(doc)).toEqual(['table'])
    const rows = doc.content?.[0]?.content ?? []
    expect(rows.map((row) => row.content?.[0]?.type)).toEqual([
      'tableHeader',
      'tableCell',
      'tableCell',
    ])
    expect(rows.map(cellTexts)).toEqual([
      ['Area', 'Verdict'],
      ['Ramble', 'smooth'],
      ['Table', 'readable'],
    ])
  })

  it('keeps the blocks around a table in order', () => {
    const doc = stableRoundTrip(
      ['Before the table.', '', TABLE, '', 'After the table.'].join('\n'),
    )

    expect(blockTypes(doc)).toEqual(['paragraph', 'table', 'paragraph'])
  })

  it('serializes a table back to a pipe table the host can read', () => {
    const manager = markdown()

    const serialized = manager.serialize(manager.parse(TABLE))

    expect(serialized).toContain('| Area')
    expect(serialized).toContain('| Ramble')
    expect(serialized).toContain('| readable')
  })

  it('keeps cell alignment', () => {
    const doc = stableRoundTrip(
      ['| Left | Right |', '| :--- | ---: |', '| a | b |'].join('\n'),
    )

    const headerCells = doc.content?.[0]?.content?.[0]?.content ?? []
    expect(headerCells.map((cell) => cell.attrs?.align)).toEqual(['left', 'right'])
  })
})

describe('feedback editor markdown task lists', () => {
  it('keeps checkbox state', () => {
    const doc = stableRoundTrip(['- [ ] open item', '- [x] done item'].join('\n'))

    expect(blockTypes(doc)).toEqual(['taskList'])
    const items = doc.content?.[0]?.content ?? []
    expect(items.map((item) => item.attrs?.checked)).toEqual([false, true])
  })
})

describe('feedback editor attachment markdown', () => {
  it('round-trips an attachment image', () => {
    const doc = stableRoundTrip('![shot.png](attachment://abc-123)')

    const image = doc.content?.[0]?.content?.[0] ?? doc.content?.[0]
    expect(image?.type).toBe('image')
    expect(image?.attrs?.src).toBe('attachment://abc-123')
    expect(image?.attrs?.alt).toBe('shot.png')
  })

  it('round-trips a non-image attachment chip', () => {
    const doc = stableRoundTrip('[notes.pdf](attachment://abc-123)')

    const chip = doc.content?.[0]?.content?.[0]
    expect(chip?.type).toBe('attachmentFile')
    expect(chip?.attrs).toMatchObject({
      attachmentId: 'abc-123',
      fileName: 'notes.pdf',
    })
  })
})

describe('ASR speech metadata in markdown', () => {
  it('serializes pending ASR speech as an ordinary paragraph', () => {
    const serialized = markdown().serialize({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: {
            [SPEECH_SEGMENT_ID_ATTR]: 'segment-1',
            [INPUT_SOURCE_ATTR]: 'asr',
            [CLEANUP_STATE_ATTR]: 'pending',
          },
          content: [{ type: 'text', text: '啊那个按钮太小了' }],
        },
      ],
    })
    expect(serialized).toContain('啊那个按钮太小了')
    expect(serialized).not.toContain(SPEECH_SEGMENT_ID_ATTR)
    expect(serialized).not.toContain(CLEANUP_STATE_ATTR)
  })

  it('serializes cleaned speech without the editor marker', () => {
    const serialized = markdown().serialize({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: {
            [SPEECH_SEGMENT_ID_ATTR]: 'segment-1',
            [INPUT_SOURCE_ATTR]: 'asr',
            [CLEANUP_STATE_ATTR]: 'pending',
          },
          content: [{ type: 'text', text: '啊那个按钮太小了' }],
        },
        {
          type: 'paragraph',
          attrs: {
            [SPEECH_SEGMENT_ID_ATTR]: 'segment-2',
            [INPUT_SOURCE_ATTR]: 'asr',
            [CLEANUP_STATE_ATTR]: 'cleaned',
          },
          content: [{ type: 'text', text: '按钮太小了。' }],
        },
        {
          type: 'paragraph',
          content: [{ type: 'text', text: '我手打的一句' }],
        },
      ],
    })
    expect(serialized).toContain('啊那个按钮太小了')
    expect(serialized).toContain('按钮太小了。')
    expect(serialized).toContain('我手打的一句')
    expect(serialized).not.toContain('✦')
    expect(serialized).not.toContain(CLEANUP_STATE_ATTR)
    expect(serialized).not.toContain('已整理')
    expect(serialized).not.toContain('data-cleanup-state')
  })
})

describe('action channel markdown', () => {
  it('exports node attrs as readable dividers', () => {
    const serialized = serializeFeedbackMarkdown({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: { actionIndex: 2 },
          content: [{ type: 'text', text: '保存之后没有 toast。' }],
        },
      ],
    })

    expect(serialized).toContain(
      '------------------------ Action 2 ------------------------',
    )
    expect(serialized).toContain('保存之后没有 toast。')
    expect(serialized).not.toContain('data-action-index')
  })

  it('rehydrates dividers as channel stamps instead of visible text', () => {
    const doc = parseFeedbackMarkdown(
      '------------------------ Action 2 ------------------------\n\n保存失败。',
    )

    expect((doc.content ?? []).map((node) => node.attrs?.actionIndex ?? null)).toEqual([
      2,
    ])
    expect(doc.content?.[0]?.content?.[0]?.text).toBe('保存失败。')
    expect(JSON.stringify(doc.content)).not.toContain('Action 2')
  })

  it('turns the default-channel separator into a channel reset with no visible rule', () => {
    const doc = parseFeedbackMarkdown(
      '------------------------ Action 2 ------------------------\n\n属于 Action 2。\n\n------------------------------------------------\n\n取消之后的新内容。',
    )

    expect(doc.content?.length).toBe(2)
    expect(doc.content?.[0]?.attrs?.actionIndex).toBe(2)
    expect(doc.content?.[0]?.content?.[0]?.text).toBe('属于 Action 2。')
    expect(doc.content?.[1]?.attrs?.actionIndex).toBeUndefined()
    expect(doc.content?.[1]?.content?.[0]?.text).toBe('取消之后的新内容。')
    expect(JSON.stringify(doc.content)).not.toContain('--------------------------------')
  })
})
