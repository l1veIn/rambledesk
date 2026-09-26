// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import { previewFixtures } from '../preview/previewFixtures'
import { locale, speechAutoTidy, speechConfirmBeforeWrite, speechOverlayEnabled } from '../preferences'
import type { SpeechRecognitionListener } from '../speech/speech'
import type { SpeechTarget } from '../speech/speechTargets'
import type { RambleSessionControllerHandle } from '../speech/rambleSessionControllerHandle'
import type { SpeechWriteInput } from '../speech/speechWriteback'
import RambleSessionController from './RambleSessionController.svelte'

const requestId = previewFixtures.workspace.request.request_id
const comment: SpeechTarget = { requestId, requestTitle: 'Review', destination: {
  kind: 'review_annotation', annotationId: 'annotation-1', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening',
} }
const replacement: SpeechTarget = { ...comment, destination: { ...comment.destination, field: 'replacement' } as SpeechTarget['destination'] }
let view: ReturnType<typeof mount> | undefined
beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} unobserve() {} })
  localStorage.clear(); locale.set('en'); speechConfirmBeforeWrite.set(false); speechAutoTidy.set(false); speechOverlayEnabled.set(true)
})
afterEach(async () => { if (view) await unmount(view); document.body.replaceChildren(); vi.unstubAllGlobals() })

function setup(embeddedConsole = false) {
  const unavailable = createUnavailableWorkbenchCapabilities()
  const capabilities = { ...unavailable, clipboardCapture: {
    ...unavailable.clipboardCapture, implementation: { ...unavailable.clipboardCapture.implementation },
  } }
  let listener: SpeechRecognitionListener
  let selected = comment
  const stop = vi.fn(async () => listener.onEvent({ type: 'stopped', sessionId: 'recording', reason: 'stopped' }))
  const start = vi.fn((_options: unknown, next: SpeechRecognitionListener) => {
    listener = next
    return { id: 'recording', ready: Promise.resolve(), stop, cancel: vi.fn(async () => {}) }
  })
  capabilities.speech = { status: { availability: 'available', source: 'native' }, implementation: { ...capabilities.speech.implementation, start } }
  const onWriteSpeech = vi.fn(async (_input: SpeechWriteInput) => {})
  const onInputText = vi.fn(async (_target: SpeechTarget, _text: string, _id?: string) => {})
  const onRouteDraftOperation = vi.fn(async () => {})
  view = mount(RambleSessionController, { target: document.body, props: {
    capabilities, workspace: previewFixtures.workspace, getNextSpeechTarget: () => selected, embeddedConsole,
    onWriteSpeech, onInputText, onRouteDraftOperation,
  } })
  return { handle: view as unknown as RambleSessionControllerHandle, capabilities, start, stop, onWriteSpeech, onInputText, onRouteDraftOperation,
    select: (target: SpeechTarget) => { selected = target },
    emit: (event: Parameters<SpeechRecognitionListener['onEvent']>[0]) => listener.onEvent(event),
  }
}

describe('shared request voice input', () => {
  it('keeps one microphone and pins late results to their original annotation field', async () => {
    const app = setup()
    await app.handle.startInput()
    app.emit({ type: 'speech-started', sessionId: 'recording', segmentIndex: 0 })
    app.emit({ type: 'processing', sessionId: 'recording', segmentIndex: 0 })
    app.select(replacement)
    await app.handle.startInput()
    app.emit({ type: 'speech-started', sessionId: 'recording', segmentIndex: 1 })
    app.emit({ type: 'stable', sessionId: 'recording', segmentIndex: 0, text: 'Explain the audience.' })
    app.emit({ type: 'stable', sessionId: 'recording', segmentIndex: 1, text: 'Welcome, everyone.' })
    await app.handle.prepareFeedback(requestId)
    expect(app.start).toHaveBeenCalledTimes(1)
    expect(app.stop).toHaveBeenCalledTimes(1)
    expect(app.onWriteSpeech.mock.calls).toMatchObject([
      [{ ...comment, text: 'Explain the audience.' }], [{ ...replacement, text: 'Welcome, everyone.' }],
    ])
    expect(app.onRouteDraftOperation).not.toHaveBeenCalled()
  })

  it('captures the clipboard text destination before the asynchronous read completes', async () => {
    const app = setup()
    let release!: (result: { kind: 'text'; text: string; capturedAtMs: number; truncated: boolean }) => void
    app.capabilities.clipboardCapture.implementation.captureOnce = () => new Promise((resolve) => { release = resolve })
    const importing = app.handle.importClipboardNow()
    await tick()
    app.select(replacement)
    release({ kind: 'text', text: 'Clipboard note', capturedAtMs: 1, truncated: false })
    await importing
    expect(app.onInputText).toHaveBeenCalledWith(expect.objectContaining(comment), 'Clipboard note', expect.any(String))
    expect(app.onWriteSpeech).not.toHaveBeenCalled()
    expect(app.onRouteDraftOperation).not.toHaveBeenCalled()
  })

  it('retains a failed annotation transcript and blocks submission preparation', async () => {
    const app = setup()
    app.onWriteSpeech.mockRejectedValue(new Error('Annotation was deleted'))
    await app.handle.startInput()
    app.emit({ type: 'speech-started', sessionId: 'recording', segmentIndex: 0 })
    app.emit({ type: 'stable', sessionId: 'recording', segmentIndex: 0, text: 'Keep this transcript' })
    await expect(app.handle.prepareFeedback(requestId)).resolves.toEqual({ kind: 'pending-speech' })
    expect(localStorage.getItem('rambledesk.speech.pending-drafts')).toContain('Keep this transcript')
    expect(app.onRouteDraftOperation).not.toHaveBeenCalled()
  })

  it('keeps the embedded workbench free of live transcripts and success receipts', async () => {
    const app = setup(true)
    await app.handle.startInput()
    app.emit({ type: 'partial', sessionId: 'recording', text: 'Words still being spoken' })
    await tick()
    expect(document.querySelector('.speech-capsule-host')).toBeNull()
    app.emit({ type: 'stable', sessionId: 'recording', segmentIndex: 0, text: 'Written directly' })
    await vi.waitFor(() => expect(app.onWriteSpeech).toHaveBeenCalledOnce())
    await tick()
    expect(document.querySelector('.speech-capsule-host')).toBeNull()
  })

  it('opens only pending speech for confirmation and preserves editing without a footer toggle', async () => {
    speechConfirmBeforeWrite.set(true)
    speechOverlayEnabled.set(false)
    const app = setup(true)
    await app.handle.startInput()
    app.emit({ type: 'stable', sessionId: 'recording', segmentIndex: 0, text: 'Review these words' })
    app.emit({ type: 'partial', sessionId: 'recording', text: 'Live words should stay hidden' })
    await tick()
    const review = document.querySelector('.speech-capsule-host')!
    expect(review.textContent).toContain('Review these words')
    expect(review.textContent).not.toContain('Live words should stay hidden')
    expect(document.querySelector('.review-toggle')).toBeNull()
    expect(document.querySelector('[aria-label="Pause recording"]')).toBeNull()
    const button = (text: string) => [...document.querySelectorAll('button')].find((item) => item.textContent?.trim() === text)!
    button('Edit').click()
    await tick()
    const editor = document.querySelector<HTMLTextAreaElement>('[aria-label="Edit speech"]')!
    editor.value = 'Reviewed words'
    editor.dispatchEvent(new Event('input', { bubbles: true }))
    await tick()
    button('Save changes').click()
    await tick()
    button('Write to selected input').click()
    await vi.waitFor(() => expect(app.onWriteSpeech).toHaveBeenCalledWith(expect.objectContaining({ ...comment, text: 'Reviewed words' })))
    await tick()
    expect(document.querySelector('.speech-capsule-host')).toBeNull()
  })

  it('recovers a failed field write through the conditional review overlay', async () => {
    const app = setup(true)
    app.onWriteSpeech.mockRejectedValueOnce(new Error('Draft write failed'))
    await app.handle.startInput()
    app.emit({ type: 'stable', sessionId: 'recording', segmentIndex: 0, text: 'Preserve these words' })
    await vi.waitFor(() => expect(document.querySelector('[role="alert"]')?.textContent).toContain('Draft write failed'))
    const retry = [...document.querySelectorAll('button')].find((item) => item.textContent?.trim() === 'Retry writing')!
    retry.click()
    await vi.waitFor(() => expect(app.onWriteSpeech).toHaveBeenCalledTimes(2))
    expect(app.onWriteSpeech.mock.calls[1][0]).toEqual(app.onWriteSpeech.mock.calls[0][0])
    await tick()
    expect(document.querySelector('.speech-capsule-host')).toBeNull()
    expect(JSON.parse(localStorage.getItem('rambledesk.speech.pending-drafts')!)).toEqual([])
  })
})
