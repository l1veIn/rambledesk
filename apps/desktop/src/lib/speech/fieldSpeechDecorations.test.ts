// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import { fieldDocumentText, fieldTextDocument, fieldTextExtensions, fieldTextPosition } from '../editor/fieldTextDocument'
import { FIELD_SPEECH_PLUGIN_KEY, FieldSpeechDecorations, setFieldSpeechSegments } from './fieldSpeechDecorations'

const editors: Editor[] = []
function input() {
  const editor = new Editor({ extensions: [...fieldTextExtensions(), FieldSpeechDecorations], content: fieldTextDocument('Typed😀\nSpoken text') })
  editors.push(editor)
  return editor
}
afterEach(() => { editors.splice(0).forEach((editor) => editor.destroy()) })

describe('field speech origin decorations', () => {
  it('renders a microphone prefix without changing field data or clipboard content', () => {
    const editor = input(), before = editor.getJSON()
    const from = fieldTextPosition(editor.state.doc, 7), to = fieldTextPosition(editor.state.doc, 18)
    setFieldSpeechSegments(editor, [{ segmentId: 'speech', from, to }])
    const marker = editor.view.dom.querySelector('.speech-origin-marker')!
    expect(marker.getAttribute('data-speech-segment-id')).toBe('speech')
    expect(marker.getAttribute('data-cleanup-state')).toBe('pending')
    expect(marker.getAttribute('contenteditable')).toBe('false')
    expect(marker.textContent).toBe('')
    expect(editor.getJSON()).toEqual(before)
    expect(editor.getHTML()).not.toContain('speech-origin-marker')
    expect(fieldDocumentText(editor.getJSON())).toBe('Typed😀\nSpoken text')
    expect(editor.commands.undo()).toBe(false)
  })

  it('maps the marker through edits and replaces it with transient tidy state', () => {
    const editor = input()
    const range = { segmentId: 'speech', from: fieldTextPosition(editor.state.doc, 7), to: fieldTextPosition(editor.state.doc, 18) }
    setFieldSpeechSegments(editor, [range])
    editor.view.dispatch(editor.state.tr.insertText('prefix ', 1))
    expect(FIELD_SPEECH_PLUGIN_KEY.getState(editor.state)?.decorations.find()[0].from).toBe(range.from + 7)
    setFieldSpeechSegments(editor, [{ ...range, from: range.from + 7, to: range.to + 7 }], ['speech'])
    expect(editor.view.dom.querySelector('.speech-origin-marker.speech-segment-tidying')).not.toBeNull()
    setFieldSpeechSegments(editor, [{ ...range, state: 'cleaned' }])
    expect(editor.view.dom.querySelector('.speech-origin-marker')).toBeNull()
  })

  it('does not render empty or out-of-document provenance', () => {
    const editor = input()
    setFieldSpeechSegments(editor, [
      { segmentId: 'empty', from: 1, to: 1 }, { segmentId: 'outside', from: 1, to: 1000 },
      { segmentId: 'before', from: 0, to: 2 },
    ])
    expect(editor.view.dom.querySelector('.speech-origin-marker')).toBeNull()
  })
})
