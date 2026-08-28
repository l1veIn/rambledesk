import { describe, expect, it } from 'vitest'
import type { JSONContent } from '@tiptap/core'

import { actionBlockquoteNode } from './actionBlockquote'
import { collectActionGroupContent } from './actionGroupContent'

function paragraph(text = ''): JSONContent {
  return {
    type: 'paragraph',
    ...(text ? { content: [{ type: 'text', text }] } : {}),
  }
}

function group(actionId: string, actionIndex: number, title: string, text = ''): JSONContent {
  return actionBlockquoteNode(
    { actionId, actionIndex, title },
    text ? [paragraph(text)] : [],
  )
}

function documentText(document: JSONContent): string[] {
  return (document.content ?? []).map((node) =>
    (node.content ?? []).map((child) => child.text ?? '').join(''),
  )
}

describe('collectActionGroupContent', () => {
  it('collects repeated non-consecutive groups for the same Action in document order', () => {
    const collected = collectActionGroupContent({
      type: 'doc',
      content: [
        group('action-1', 0, 'First', 'First note'),
        paragraph('General feedback'),
        group('action-2', 1, 'Second', 'Second note'),
        group('action-1', 0, 'First', 'Later note'),
      ],
    })

    expect(collected.get('action-1')?.groupCount).toBe(2)
    expect(documentText(collected.get('action-1')!.document)).toEqual([
      'First note',
      'Later note',
    ])
    expect(documentText(collected.get('action-2')!.document)).toEqual(['Second note'])
  })

  it('omits Action titles and empty groups', () => {
    const collected = collectActionGroupContent({
      type: 'doc',
      content: [
        group('action-1', 0, 'Long instruction'),
        group('action-2', 1, 'Other instruction', 'Visible note'),
      ],
    })

    expect(collected.has('action-1')).toBe(false)
    expect(documentText(collected.get('action-2')!.document)).toEqual(['Visible note'])
  })

  it('keeps rich block nodes and attachment nodes intact', () => {
    const list: JSONContent = {
      type: 'bulletList',
      content: [
        {
          type: 'listItem',
          content: [paragraph('A list item')],
        },
      ],
    }
    const attachment: JSONContent = {
      type: 'image',
      attrs: { src: 'attachment://image-1', attachmentId: 'image-1' },
    }
    const collected = collectActionGroupContent({
      type: 'doc',
      content: [
        actionBlockquoteNode(
          { actionId: 'action-1', actionIndex: 0, title: 'First' },
          [list, attachment],
        ),
      ],
    })

    expect(collected.get('action-1')?.document.content).toEqual([list, attachment])
  })
})
