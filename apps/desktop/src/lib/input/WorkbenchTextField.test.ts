// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { AttachmentView } from '../feedback'
import type { FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { locale } from '../preferences'
import { fieldDocumentText } from '../editor/fieldTextDocument'
import { inputText, tiptapEditor } from '../../test/tiptap'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../speech/voiceInputContext'
import { INPUT_TOOLS_CONTEXT } from './inputToolsContext'
import WorkbenchTextField from './WorkbenchTextField.svelte'

const target = { requestId: 'r', requestTitle: 'Questions', destination: {
  kind: 'question_answer' as const, questionId: 'q', questionLabel: 'Question',
} }
let view: ReturnType<typeof mount> | undefined
beforeEach(() => locale.set('en'))
afterEach(async () => { if (view) await unmount(view); view = undefined; document.body.replaceChildren() })

async function setup(initial: string, maxLength = 4000, attachments: AttachmentView[] = []) {
  const state = writable({ value: initial, disabled: false })
  const props = fromStore(state)
  const voiceState = writable<VoiceInputState>({ requestId: 'r', documentTarget: null, nextTarget: target,
    disabled: false, recording: false })
  const selectTarget = vi.fn()
  const onChange = vi.fn((value: string) => state.update((current) => ({ ...current, value })))
  view = mount(WorkbenchTextField, { target: document.body, props: {
    get value() { return props.current.value }, get disabled() { return props.current.disabled },
    label: 'Your answer', voiceLabel: 'Speak answer', target, maxLength, onChange,
    'data-question-answer': 'q',
  }, context: new Map<symbol, unknown>([
    [VOICE_INPUT_CONTEXT, { state: voiceState, selectTarget, start: vi.fn(), stop: vi.fn() }],
    [INPUT_TOOLS_CONTEXT, { state: writable({ requestId: 'r', disabled: false, busy: false,
      canCapture: false, canPaste: false, attachments }), preview: vi.fn(), reportError: vi.fn() }],
  ]) })
  await tick()
  const element = document.querySelector<HTMLElement>('[data-question-answer]')!
  const editor = tiptapEditor(element)
  return { state, voiceState, selectTarget, onChange, element, editor }
}

function speechSnapshot(value: string, start: number, text: string, state = 'pending'): FeedbackDraftSnapshot {
  return { bodyMarkdown: '', documentJson: JSON.stringify({ schemaVersion: 2, doc: { type: 'doc', content: [] },
    workbenchState: { type: 'questions', answers: [{ id: 'q', value, label: value, wasCustom: true }] },
    fieldSpeechSegments: { version: 1, segments: [{ segmentId: 's', target, start, end: start + [...text].length,
      text, state, contract: 'q-v1', identity: 'q' }] },
  }) }
}

describe('shared TipTap workbench fields', () => {
  it('keeps literal multiline text and preserves caret/history across a background voice append', async () => {
    const initial = '**literal**\n\n😀 Last\n'
    const app = await setup(initial)
    expect(document.querySelector('textarea')).toBeNull()
    expect(inputText(app.element)).toBe(initial)
    expect(app.editor.getJSON().content?.[0].content?.[0].marks).toBeUndefined()
    app.editor.commands.setTextSelection(2)
    app.editor.view.dispatch(app.editor.state.tr.insertText('手'))
    await tick()
    const typed = fieldDocumentText(app.editor.getJSON())
    const caret = app.editor.state.selection.from
    app.state.update((value) => ({ ...value, value: typed + 'Voice' }))
    await tick()
    expect(app.editor.state.selection.from).toBe(caret)
    expect(inputText(app.element)).toBe(typed + 'Voice')
    app.editor.commands.undo()
    await tick()
    expect(inputText(app.element)).toBe(initial + 'Voice')
    expect(app.onChange.mock.lastCall?.[0]).toBe(initial + 'Voice')
  })

  it('shows a microphone at the spoken text after hidden attachments, then clears it after tidy', async () => {
    const attachment = { attachment_id: 'file-1', file_name: 'notes.pdf', media_type: 'application/pdf' } as AttachmentView
    const prefix = 'Typed😀\n\n[notes](attachment://file-1)\n'
    const spoken = '嗯，这里需要修改'
    const value = prefix + spoken
    const app = await setup(value, 4000, [attachment])
    app.voiceState.update((state) => ({ ...state, draftSnapshot: speechSnapshot(value, [...prefix].length, spoken) }))
    await tick()
    const marker = app.element.querySelector('[data-speech-segment-id="s"]')!
    expect(marker).not.toBeNull()
    expect(marker.nextSibling?.textContent).toBe(spoken)
    expect(app.element.textContent).not.toContain('attachment://')
    expect(app.editor.getJSON()).not.toHaveProperty('speechSegmentId')
    expect(JSON.stringify(app.editor.getJSON())).not.toContain('speech-origin-marker')
    app.voiceState.update((state) => ({ ...state, draftSnapshot: speechSnapshot(value, [...prefix].length, spoken, 'cleaned') }))
    await tick()
    expect(app.element.querySelector('.speech-origin-marker')).toBeNull()
    expect(app.onChange).not.toHaveBeenCalled()
  })

  it('keeps attachment references and counts Unicode scalars when input reaches its limit', async () => {
    const attachment = { attachment_id: 'file-1', file_name: 'notes.pdf', media_type: 'application/pdf' } as AttachmentView
    const suffix = '\n\n[notes](attachment://file-1)'
    const app = await setup('A' + suffix, [...suffix].length + 4, [attachment])
    app.editor.view.dispatch(app.editor.state.tr.insertText('😀😀😀😀😀', 2))
    await tick()
    expect(app.onChange.mock.lastCall?.[0]).toBe('A😀😀😀' + suffix)
    expect(inputText(app.element)).toBe('A😀😀😀')
    app.editor.commands.undo()
    await tick()
    expect(inputText(app.element)).toBe('A')
    expect(app.onChange.mock.lastCall?.[0]).toBe('A' + suffix)
  })

  it('defers external replacement during IME composition and retains both typing and arriving speech', async () => {
    const app = await setup('Draft')
    app.element.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }))
    expect(app.editor.view.composing).toBe(true)
    app.state.update((state) => ({ ...state, value: 'Draft\nVoice' }))
    app.voiceState.update((state) => ({ ...state, draftSnapshot: speechSnapshot('Draft\nVoice', 6, 'Voice') }))
    await tick()
    expect(inputText(app.element)).toBe('Draft')
    app.editor.view.dispatch(app.editor.state.tr.insertText('中文', 6))
    await tick()
    expect(app.onChange.mock.lastCall?.[0]).toBe('Draft中文\nVoice')
    app.voiceState.update((state) => ({ ...state, draftSnapshot: speechSnapshot('Draft中文\nVoice', 8, 'Voice') }))
    app.element.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '中文' }))
    await vi.waitFor(() => expect(inputText(app.element)).toBe('Draft中文\nVoice'))
    expect(app.element.querySelector('[data-speech-segment-id="s"]')?.nextSibling?.textContent).toBe('Voice')
  })

  it('retains Shift+Enter hard breaks through controlled value echoes and undo', async () => {
    const app = await setup('First')
    app.editor.commands.setTextSelection(6)
    app.editor.commands.setHardBreak()
    await tick()
    expect(app.editor.getJSON().content).toHaveLength(1)
    expect(app.editor.getJSON().content?.[0].content?.[1].type).toBe('hardBreak')
    expect(app.onChange.mock.lastCall?.[0]).toBe('First\n')
    app.editor.commands.undo()
    await tick()
    expect(inputText(app.element)).toBe('First')
  })
})
