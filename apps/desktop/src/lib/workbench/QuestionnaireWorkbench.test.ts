// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { QuestionAnswer, QuestionsData } from '../generated/feedback'
import { locale } from '../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../speech/voiceInputContext'
import type { SpeechTarget } from '../speech/speechTargets'
import { INPUT_TOOLS_CONTEXT, type InputToolsState } from '../input/inputToolsContext'
import QuestionnaireWorkbench from './QuestionnaireWorkbench.svelte'
import { inputText, replaceInputText, tiptapEditor } from '../../test/tiptap'

const data: QuestionsData = { questions: [
  { id: 'layout', label: 'Layout', prompt: 'Which layout?', allowOther: true, options: [{ value: 'compact', label: 'Compact' }, { value: 'roomy', label: 'Roomy' }] },
  { id: 'delivery', label: 'Delivery', prompt: 'When to ship?', allowOther: false, options: [{ value: 'now', label: 'Now' }, { value: 'later', label: 'Later' }] },
] }
const documentTarget: SpeechTarget = { requestId: 'request-1', requestTitle: 'Product choices', destination: { kind: 'document', action: null } }
const answerTarget: SpeechTarget = { ...documentTarget, destination: { kind: 'question_answer', questionId: 'layout', questionLabel: 'Layout' } }
const custom: QuestionAnswer = { id: 'layout', value: 'My layout', label: 'My layout', wasCustom: true }
let view: ReturnType<typeof mount> | undefined
const button = (text: string) => Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.trim() === text || item.getAttribute('aria-label') === text)!
const answerInput = () => document.querySelector<HTMLElement>('[data-question-answer="layout"]')
beforeEach(() => {
  locale.set('en')
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.body.replaceChildren() })
function open(answers: QuestionAnswer[] = [], disabled = false) {
  const state = writable<VoiceInputState>({ requestId: documentTarget.requestId, documentTarget, nextTarget: documentTarget, recording: false, disabled: false })
  const voice = { state, start: vi.fn(), stop: vi.fn(), selectTarget: vi.fn() }
  const tools = { state: writable<InputToolsState>({ requestId: documentTarget.requestId, disabled, busy: false, canCapture: true, canPaste: true, attachments: [] }), capture: vi.fn(), paste: vi.fn(), files: vi.fn(), preview: vi.fn(), reportError: vi.fn() }
  const onChange = vi.fn()
  view = mount(QuestionnaireWorkbench, { target: document.body, context: new Map<symbol, unknown>([[VOICE_INPUT_CONTEXT, voice], [INPUT_TOOLS_CONTEXT, tools]]), props: { data, answers, disabled, onChange } })
  return { voice, tools, onChange }
}

describe('question custom answer voice input', () => {
  it('keeps input tools and attachment chips bound to this answer while typing and removing a reference', async () => {
    const reference = '[Screenshot](attachment://screen)'
    const { tools, onChange } = open([{ ...custom, value: `My layout\n\n${reference}`, label: `My layout\n\n${reference}` }])
    tools.state.update((state) => ({ ...state, attachments: [{ attachment_id: 'screen', file_name: 'Screenshot', media_type: 'image/png', byte_size: 1, sha256: '', position: 0 }] }))
    await vi.waitFor(() => expect(inputText(answerInput())).toBe('My layout'))
    expect(document.querySelector('[data-input-toolbar]')?.contains(button('Capture'))).toBe(true)
    button('Capture').click()
    await vi.waitFor(() => expect(tools.capture).toHaveBeenCalledWith(answerTarget))
    button('Preview Screenshot').click()
    expect(tools.preview).toHaveBeenCalledWith('screen')
    replaceInputText(answerInput(), 'Adjusted layout')
    await vi.waitFor(() => expect(onChange.mock.lastCall?.[0][0].value).toBe(`Adjusted layout\n\n${reference}`))
    button('Remove attachment reference Screenshot').click()
    await vi.waitFor(() => expect(onChange.mock.lastCall?.[0][0].value).toBe('Adjusted layout'))
    expect(document.querySelector('[data-field-attachments]')).toBeNull()
  })
  it('offers voice only after choosing Other and focusing selects the question without starting or advancing', async () => {
    const { voice, onChange } = open()
    expect(document.querySelector('[data-voice-input]')).toBeNull()
    button('Other — write your answer').click()
    await vi.waitFor(() => expect(answerInput()).not.toBeNull())
    answerInput()!.focus()
    expect(voice.selectTarget).toHaveBeenLastCalledWith(answerTarget)
    expect(voice.start).not.toHaveBeenCalled()
    button('Speak answer').click()
    expect(voice.start).toHaveBeenCalledWith(answerTarget)
    replaceInputText(answerInput(), 'Typed custom answer')
    expect(onChange).toHaveBeenLastCalledWith([{ id: 'layout', value: 'Typed custom answer', label: 'Typed custom answer', wasCustom: true }])
    expect(document.querySelector('h3')?.textContent).toBe('Which layout?')
  })

  it('uses the scalar character limit and preserves an empty custom draft until explicitly cleared', async () => {
    const { onChange } = open([custom])
    await vi.waitFor(() => expect(answerInput()).not.toBeNull())
    const input = answerInput()!
    replaceInputText(input, '😀'.repeat(4001))
    await vi.waitFor(() => expect([...inputText(input)]).toHaveLength(4000))
    expect(onChange.mock.lastCall?.[0][0].label).toBe(inputText(input))
    replaceInputText(input, '')
    expect(onChange.mock.lastCall?.[0]).toEqual([{ id: 'layout', value: '', label: '', wasCustom: true }])
    button('Clear answer').click()
    expect(onChange).toHaveBeenLastCalledWith([])
    await vi.waitFor(() => expect(answerInput()).toBeNull())
  })

  it('reveals an existing custom answer by question identity and never recreates a cleared answer', async () => {
    const { voice, onChange } = open([custom])
    button('2 Delivery').click()
    await vi.waitFor(() => expect(document.querySelector('h3')?.textContent).toBe('When to ship?'))
    expect(document.querySelector('[data-voice-input]')).toBeNull()
    voice.state.update((state) => ({ ...state, revealTarget: answerTarget, revealSequence: 1 }))
    await vi.waitFor(() => expect(document.activeElement).toBe(answerInput()))
    expect(voice.start).not.toHaveBeenCalled()
    expect(onChange).not.toHaveBeenCalled()
    button('Clear answer').click()
    button('2 Delivery').click()
    voice.state.update((state) => ({ ...state, revealTarget: answerTarget, revealSequence: 2 }))
    await vi.waitFor(() => expect(document.querySelector('h3')?.textContent).toBe('Which layout?'))
    expect(answerInput()).toBeNull()
    expect(onChange).toHaveBeenCalledTimes(1)
  })

  it('does not start recording when switching to an option and disables voice on read-only answers', async () => {
    const { voice } = open([custom], true)
    await vi.waitFor(() => expect(answerInput()).not.toBeNull())
    expect(answerInput()!.getAttribute('contenteditable')).toBe('false')
    expect(answerInput()!.getAttribute('aria-disabled')).toBe('true')
    expect(button('Speak answer').disabled).toBe(true)
    answerInput()!.dispatchEvent(new FocusEvent('focus'))
    button('Speak answer').click()
    expect(voice.selectTarget).not.toHaveBeenCalled()
    expect(voice.start).not.toHaveBeenCalled()
    await unmount(view!); view = undefined; document.body.replaceChildren()
    const editable = open([custom])
    const compact = Array.from(document.querySelectorAll<HTMLButtonElement>('[data-answer-option]')).find((item) => item.textContent?.includes('Compact'))!
    compact.click()
    await vi.waitFor(() => expect(document.querySelector('h3')?.textContent).toBe('When to ship?'))
    expect(editable.voice.start).not.toHaveBeenCalled()
    expect(editable.onChange.mock.lastCall?.[0][0]).toMatchObject({ value: 'compact', wasCustom: false })
  })

  it('uses TipTap history for custom edits without changing the answer mode or feedback notes', async () => {
    const { onChange } = open([custom])
    await vi.waitFor(() => expect(answerInput()).not.toBeNull())
    expect(answerInput()!.getAttribute('role')).toBe('textbox')
    expect(document.querySelector('textarea')).toBeNull()
    replaceInputText(answerInput(), 'Revised layout')
    expect(onChange.mock.lastCall?.[0][0]).toMatchObject({ value: 'Revised layout', wasCustom: true })
    expect(tiptapEditor(answerInput()).commands.undo()).toBe(true)
    expect(inputText(answerInput())).toBe('My layout')
    expect(onChange.mock.lastCall?.[0][0]).toMatchObject({ value: 'My layout', wasCustom: true })
    expect(tiptapEditor(answerInput()).commands.redo()).toBe(true)
    expect(onChange.mock.lastCall?.[0][0].value).toBe('Revised layout')
  })
})
