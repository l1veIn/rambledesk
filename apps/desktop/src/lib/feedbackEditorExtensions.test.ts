import { describe, expect, it } from 'vitest'
import { MarkdownManager } from '@tiptap/markdown'
import type { JSONContent } from '@tiptap/core'

import { feedbackEditorExtensions } from './feedbackEditorExtensions'
import { PENDING_SPEECH_NODE } from './pendingSpeech'

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

describe('pending speech markdown', () => {
  it('serializes pending speech as an ordinary paragraph', () => {
    const serialized = markdown().serialize({
      type: 'doc',
      content: [
        {
          type: PENDING_SPEECH_NODE,
          attrs: { status: 'pending' },
          content: [{ type: 'text', text: '啊那个按钮太小了' }],
        },
      ],
    })
    expect(serialized).toContain('啊那个按钮太小了')
    expect(serialized).not.toContain('pendingSpeech')
    expect(serialized).not.toContain('data-speech-status')
  })
})
