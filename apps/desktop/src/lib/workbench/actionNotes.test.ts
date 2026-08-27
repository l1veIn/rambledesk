import { describe, expect, it } from 'vitest'

import { actionNotesFromDocument } from './actionNotes'

describe('actionNotesFromDocument', () => {
  it('groups stamped blocks per Action and renders them as plain Markdown', () => {
    const markdown = [
      '先说整体。',
      '------------------------ Action 2 ------------------------',
      '保存之后没有 toast。',
      '',
      '------------------------ Action 1 ------------------------',
      '按钮太小了。',
      '',
      '最后补充。',
    ].join('\n\n')
    const notes = actionNotesFromDocument(null, markdown)

    expect(notes[1]).toContain('按钮太小了。')
    expect(notes[2]).toContain('保存之后没有 toast。')
    expect(notes[1]).not.toContain('Action 1')
    expect(notes[2]).not.toContain('Action 2')
    expect(notes[1]).not.toContain('先说整体。')
  })

  it('returns an empty record when nothing is stamped', () => {
    expect(actionNotesFromDocument(null, '只有普通文本。')).toEqual({})
  })
})
