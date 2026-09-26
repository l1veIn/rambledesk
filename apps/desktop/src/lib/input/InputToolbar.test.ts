// @vitest-environment jsdom
import { tick } from 'svelte'
import { createClassComponent } from 'svelte/legacy'
import { writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '../preferences'
import { INPUT_TOOLS_CONTEXT, type InputToolsState } from './inputToolsContext'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../speech/voiceInputContext'
import type { SpeechTarget } from '../speech/speechTargets'
import InputToolbar from './InputToolbar.svelte'

const documentTarget: SpeechTarget = {
  requestId: 'request-1', requestTitle: 'Script review', destination: { kind: 'document', action: null },
}
const commentTarget: SpeechTarget = {
  ...documentTarget,
  destination: { kind: 'review_annotation', annotationId: 'opening', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening' },
}
let view: ReturnType<typeof createClassComponent> | undefined
beforeEach(() => locale.set('en'))
afterEach(async () => {
  view?.$destroy(); view = undefined
  await tick()
  vi.restoreAllMocks()
  document.body.replaceChildren()
})

function setup(target = documentTarget, disabled = false) {
  const voiceState = writable<VoiceInputState>({
    requestId: documentTarget.requestId, documentTarget, nextTarget: documentTarget, recording: true, disabled: false,
  })
  const toolsState = writable<InputToolsState>({
    requestId: documentTarget.requestId, disabled: false, busy: false, canCapture: true, canPaste: true, attachments: [],
  })
  const voice = { state: voiceState, start: vi.fn(), stop: vi.fn(), selectTarget: vi.fn() }
  const tools = { state: toolsState, capture: vi.fn(), paste: vi.fn(), files: vi.fn(), preview: vi.fn(), reportError: vi.fn() }
  view = createClassComponent({
    component: InputToolbar, target: document.body, props: { target, disabled },
    context: new Map<symbol, unknown>([[VOICE_INPUT_CONTEXT, voice], [INPUT_TOOLS_CONTEXT, tools]]),
  })
  return { voice, voiceState, tools, toolsState, setTarget: (next: SpeechTarget) => view!.$set({ target: next }) }
}

function button(name: string): HTMLButtonElement {
  const result = [...document.querySelectorAll<HTMLButtonElement>('button')].find(item => item.getAttribute('aria-label') === name)
  expect(result, `button ${name}`).toBeDefined()
  return result!
}

describe('input toolbar destination ownership', () => {
  it('pauses the active input microphone and resumes the same target after stopping', async () => {
    const app = setup()
    button('Pause recording').click()
    expect(app.voice.stop).toHaveBeenCalledOnce()
    expect(app.voice.start).not.toHaveBeenCalled()
    app.voiceState.update(state => ({ ...state, recording: false }))
    await tick()
    button('Speak here').click()
    expect(app.voice.start).toHaveBeenCalledWith(documentTarget)
    expect(app.voice.stop).toHaveBeenCalledOnce()
  })

  it('selects another input through the shared recording without stopping it', () => {
    const app = setup(commentTarget)
    button('Speak here').click()
    expect(app.voice.start).toHaveBeenCalledWith(commentTarget)
    expect(app.voice.stop).not.toHaveBeenCalled()
  })

  it.each([['Capture', 'capture'], ['Clipboard', 'paste']] as const)(
    '%s fixes the target at the click before asynchronous acquisition starts', async (label, method) => {
      const app = setup(commentTarget)
      button(label).click()
      app.setTarget(documentTarget)
      await tick()
      expect(app.voice.selectTarget).toHaveBeenCalledWith(commentTarget)
      expect(app.tools[method]).toHaveBeenCalledWith(commentTarget)
      expect(app.tools[method].mock.calls[0]?.[0]).not.toBe(commentTarget)
      expect(app.voice.start).not.toHaveBeenCalled()
      expect(app.voice.stop).not.toHaveBeenCalled()
    },
  )

  it('keeps the original file destination while the picker is open and the input changes', async () => {
    const app = setup(commentTarget)
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!
    const openPicker = vi.spyOn(input, 'click').mockImplementation(() => {})
    button('Choose files').click()
    expect(openPicker).toHaveBeenCalledOnce()
    expect(app.voice.selectTarget).toHaveBeenCalledWith(commentTarget)
    app.setTarget(documentTarget)
    await tick()
    const file = new File(['reference'], 'reference.txt', { type: 'text/plain' })
    Object.defineProperty(input, 'files', { configurable: true, value: [file] })
    input.dispatchEvent(new Event('change', { bubbles: true }))
    await tick()
    expect(app.tools.files).toHaveBeenCalledWith(commentTarget, [file])
    expect(app.tools.files.mock.calls[0]?.[0]).not.toBe(commentTarget)
    expect(input.value).toBe('')
    // A repeated browser change event cannot reuse a consumed destination.
    input.dispatchEvent(new Event('change', { bubbles: true }))
    await tick()
    expect(app.tools.files).toHaveBeenCalledOnce()
  })

  it('does not invoke any input action when the input is locked', () => {
    const app = setup(commentTarget, true)
    for (const name of ['Speak here', 'Capture', 'Clipboard', 'Choose files']) {
      expect(button(name).disabled).toBe(true)
      button(name).click()
    }
    expect(app.voice.start).not.toHaveBeenCalled()
    expect(app.voice.stop).not.toHaveBeenCalled()
    expect(app.voice.selectTarget).not.toHaveBeenCalled()
    expect(app.tools.capture).not.toHaveBeenCalled()
    expect(app.tools.paste).not.toHaveBeenCalled()
    expect(app.tools.files).not.toHaveBeenCalled()
  })

  it.each(['disabled', 'busy', 'different request'] as const)('blocks acquisition when shared tools are %s', async (reason) => {
    const app = setup(commentTarget)
    app.toolsState.update(state => ({
      ...state, disabled: reason === 'disabled', busy: reason === 'busy',
      requestId: reason === 'different request' ? 'request-2' : state.requestId,
    }))
    await tick()
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!
    const openPicker = vi.spyOn(input, 'click').mockImplementation(() => {})
    for (const name of ['Capture', 'Clipboard', 'Choose files']) {
      expect(button(name).disabled).toBe(true)
      button(name).click()
    }
    expect(app.voice.selectTarget).not.toHaveBeenCalled()
    expect(openPicker).not.toHaveBeenCalled()
    expect(app.tools.capture).not.toHaveBeenCalled()
    expect(app.tools.paste).not.toHaveBeenCalled()
    expect(app.tools.files).not.toHaveBeenCalled()
  })
})
