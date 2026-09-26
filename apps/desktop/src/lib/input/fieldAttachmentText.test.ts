import { describe, expect, it } from 'vitest'
import { fieldAttachmentText, mapFieldAttachmentTextRanges, removeFieldAttachment, removeWorkbenchAttachmentReferences, replaceFieldAttachmentText } from './fieldAttachmentText'
import { snapshotFeedbackDraftMarkdown } from '../feedbackDraftDocument'
import { readWorkbenchState, withWorkbenchState } from '../workbenchState'

const known = [{ attachment_id: 'screen' }, { attachment_id: 'notes' }]
const screenshot = '![Screenshot](attachment://screen)'
const notes = '[Notes](attachment://notes)'

describe('field attachment text', () => {
  it('maps voice offsets after hidden chips with scalar precision and omits partially hidden speech', () => {
    const prefix = `😀Typed\n\n${screenshot}\n\n${notes}\n`
    const text = 'Spoken😀', value = prefix + text
    const start = [...prefix].length
    const mapped = mapFieldAttachmentTextRanges(value, known, [
      { id: 'before', start: 0, end: 6 },
      { id: 'speech', start, end: start + [...text].length },
      { id: 'hidden', start: 8, end: 10 },
      { id: 'overlap', start: 0, end: start + 1 },
    ])
    expect(mapped).toEqual([{ id: 'before', start: 0, end: 6 }, { id: 'speech', start: 7, end: 14 }])
    expect([...fieldAttachmentText(value, known).text].slice(mapped[1].start, mapped[1].end).join('')).toBe(text)
  })

  it('shows real references as chips while preserving unknown links and code verbatim', () => {
    const text = 'Observe this.\n\n`' + screenshot + '`\n\n```md\n' + notes + '\n```\n\n[Unknown](attachment://missing)'
    expect(fieldAttachmentText(`${text}\n\n${screenshot}\n\n${notes}`, known)).toEqual({
      text, attachmentIds: ['screen', 'notes'],
    })
    expect(fieldAttachmentText('Plain text  \n\n', known)).toEqual({ text: 'Plain text  \n\n', attachmentIds: [] })
  })

  it('retains references when editing or emptying text and preserves user newlines', () => {
    const original = `Original\n\n${screenshot}\n\n${notes}`
    expect(replaceFieldAttachmentText(original, 'Edited\n\n', known, 4000)).toBe(`Edited\n\n\n\n${screenshot}\n\n${notes}`)
    expect(fieldAttachmentText(replaceFieldAttachmentText(original, 'Edited\n\n', known, 4000), known).text).toBe('Edited\n\n')
    expect(replaceFieldAttachmentText(original, '', known, 4000)).toBe(`${screenshot}\n\n${notes}`)
    expect(replaceFieldAttachmentText(original, 'Original', known, 4000)).toBe(original)
  })

  it('removes only the selected reference, leaving code and other field text untouched', () => {
    const value = `Read \`${screenshot}\`.\n\n${screenshot}\n\n${notes}`
    expect(removeFieldAttachment(value, 'screen', known)).toBe(`Read \`${screenshot}\`.\n\n${notes}`)
    expect(removeFieldAttachment(value, 'missing', known)).toBe(value)
  })

  it('counts attachment references in the Unicode scalar field limit without truncating them', () => {
    const value = `Original\n\n${screenshot}`
    const result = replaceFieldAttachmentText(value, '😀'.repeat(4001), known, 4000)
    expect([...result]).toHaveLength(4000)
    expect(result.endsWith(screenshot)).toBe(true)
    expect(fieldAttachmentText(result, known).text).toBe('😀'.repeat(4000 - screenshot.length - 2))
  })

  it('recognizes escaped file names and preserves surrounding inline text', () => {
    const value = 'Before [notes \\[v2\\]](attachment://notes) after.'
    expect(fieldAttachmentText(value, known)).toEqual({ text: 'Before  after.', attachmentIds: ['notes'] })
    expect(removeFieldAttachment(value, 'notes', known)).toBe('Before  after.')
  })

  it('unlinks a deleted file across custom answers while preserving fixed answers and opaque envelope metadata', () => {
    const original = withWorkbenchState(snapshotFeedbackDraftMarkdown('Default notes'), { type: 'questions', answers: [
      { id: 'one', value: `First\n\n${screenshot}`, label: `First\n\n${screenshot}`, wasCustom: true },
      { id: 'two', value: `Second\n\n${screenshot}\n\n${notes}`, label: `Second\n\n${screenshot}\n\n${notes}`, wasCustom: true },
      { id: 'fixed', value: screenshot, label: screenshot, wasCustom: false },
    ] })
    const envelope = { ...JSON.parse(original.documentJson), fieldSpeechSegments: { version: 1, segments: [{ segmentId: 'speech-1' }] }, futureData: { keep: true } }
    original.documentJson = JSON.stringify(envelope)
    const next = removeWorkbenchAttachmentReferences(original, 'screen')
    expect(readWorkbenchState(next.documentJson)).toMatchObject({ type: 'questions', answers: [
      { value: 'First', label: 'First' }, { value: `Second\n\n${notes}`, label: `Second\n\n${notes}` }, { value: screenshot, label: screenshot },
    ] })
    expect(JSON.parse(next.documentJson)).toMatchObject({ fieldSpeechSegments: envelope.fieldSpeechSegments, futureData: envelope.futureData, doc: envelope.doc })
    expect(next.bodyMarkdown).toBe(original.bodyMarkdown)
    expect(removeWorkbenchAttachmentReferences(next, 'missing')).toBe(next)
  })

  it('unlinks a shared file across comment and suggestion fields without changing review anchors', () => {
    const annotation = { id: 'one', paragraph_id: 'opening', start: null, end: null, quote: null, kind: 'suggestion' as const,
      body: `Comment\n\n${screenshot}`, replacement: `Rewrite\n\n${screenshot}` }
    const original = withWorkbenchState(snapshotFeedbackDraftMarkdown('Notes'), { type: 'document_review', verdict: 'changes_requested', paragraph_marks: [],
      annotations: [annotation, { ...annotation, id: 'two', kind: 'comment', replacement: null }],
    })
    expect(readWorkbenchState(removeWorkbenchAttachmentReferences(original, 'screen').documentJson)).toMatchObject({ annotations: [
      { ...annotation, body: 'Comment', replacement: 'Rewrite' },
      { ...annotation, id: 'two', kind: 'comment', body: 'Comment', replacement: null },
    ] })
  })
})
