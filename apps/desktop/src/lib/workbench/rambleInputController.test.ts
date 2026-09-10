// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import type { ClipboardCaptureResult } from '../capabilities/capturePlugin'
import { locale, speechConfirmBeforeWrite } from '../preferences'
import type { SpeechRecognitionListener } from '../speech/speech'
import { previewFixtures } from '../preview/previewFixtures'
import RambleSessionController from './RambleSessionController.svelte'
import { createRambleSession } from './rambleSession'

const request = previewFixtures.workspace.request
const views: Array<ReturnType<typeof mount>> = []
beforeEach(() => {
  localStorage.clear()
  locale.set('en')
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
})
afterEach(async () => {
  for (const view of views.splice(0)) await unmount(view)
  document.body.replaceChildren()
  vi.unstubAllGlobals()
})
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail })
  return { promise, resolve, reject }
}
function harness(extra: { waitForDocumentWrites?: () => Promise<void> } = {}) {
  const capture = deferred<ClipboardCaptureResult>()
  const write = deferred<void>()
  const unavailable = createUnavailableWorkbenchCapabilities()
  const captureOnce = vi.fn(() => capture.promise)
  const onRouteDraftOperation = vi.fn(() => write.promise)
  const onAttachmentMessage = vi.fn()
  const onPersistAttachmentCandidates = vi.fn(async () => true)
  const session = createRambleSession()
  session.begin(request)
  session.transition('paused', 'Paused')
  const view = mount(RambleSessionController, { target: document.body, props: {
    capabilities: { ...unavailable, clipboardCapture: { ...unavailable.clipboardCapture,
      implementation: { captureOnce } } },
    workspace: previewFixtures.workspace, session, onRouteDraftOperation,
    onAttachmentMessage, onPersistAttachmentCandidates,
    ...extra,
  } })
  views.push(view)
  return { view, session, capture, write, captureOnce, onRouteDraftOperation, onAttachmentMessage, onPersistAttachmentCandidates }
}
const text = { kind: 'text', text: 'Clipboard feedback', capturedAtMs: 1, truncated: false } as const

describe('Ramble input preparation through the mounted controller', () => {
  it('stops a microphone whose start was pending before preparation, then drains its final words', async () => {
    const ready = deferred<void>()
    const write = deferred<void>()
    const unavailable = createUnavailableWorkbenchCapabilities()
    const session = createRambleSession()
    let listener!: SpeechRecognitionListener
    const stop = vi.fn(async () => {
      listener.onEvent({ type: 'stable', sessionId: 'delayed-start', segmentIndex: 0, text: 'The final observation' })
      listener.onEvent({ type: 'stopped', sessionId: 'delayed-start', reason: 'stopped' })
    })
    const start = vi.fn((_options, next: SpeechRecognitionListener) => {
      listener = next
      return { id: 'delayed-start', ready: ready.promise, stop, cancel: async () => {} }
    })
    const onRouteDraftOperation = vi.fn(() => write.promise)
    const previousConfirmation = get(speechConfirmBeforeWrite)
    speechConfirmBeforeWrite.set(false)
    const view = mount(RambleSessionController, { target: document.body, props: {
      capabilities: { ...unavailable, speech: {
        status: { availability: 'available', source: 'browser' },
        implementation: { ...unavailable.speech.implementation, start },
      } }, workspace: previewFixtures.workspace, session, onRouteDraftOperation,
    } })
    views.push(view)
    try {
      const starting = view.toggleRamble()
      await vi.waitFor(() => expect(start).toHaveBeenCalledOnce())
      expect(get(session).voicePhase).toBe('starting')
      let prepared = false
      const preparation = view.prepareFeedback(request.request_id).then((result) => { prepared = true; return result })
      ready.resolve()
      await starting

      await vi.waitFor(() => expect(stop).toHaveBeenCalledOnce())
      expect(prepared).toBe(false)
      expect(onRouteDraftOperation).toHaveBeenCalledWith(request.request_id, expect.objectContaining({
        kind: 'appendSpeech', text: 'The final observation',
      }))
      write.resolve()
      await expect(preparation).resolves.toEqual({ kind: 'ready' })
      expect(get(session)).toMatchObject({ requestId: '', voicePhase: 'idle', voiceActive: false })
    } finally {
      ready.resolve()
      write.resolve()
      speechConfirmBeforeWrite.set(previousConfirmation)
    }
  })

  it('joins capture already in flight and waits for the document write before returning ready', async () => {
    const h = harness()
    const first = h.view.importClipboardNow()
    expect(h.view.importClipboardNow()).toBe(first)
    await tick()
    let prepared = false
    const preparation = h.view.prepareFeedback(request.request_id).then((result) => { prepared = true; return result })
    await tick()
    expect(prepared).toBe(false)
    h.capture.resolve(text)
    await vi.waitFor(() => expect(h.onRouteDraftOperation).toHaveBeenCalledOnce())
    expect(prepared).toBe(false)
    h.write.resolve()
    await expect(preparation).resolves.toEqual({ kind: 'ready' })
    expect(h.captureOnce).toHaveBeenCalledOnce()
    expect(h.onRouteDraftOperation).toHaveBeenCalledWith(request.request_id, expect.objectContaining({ kind: 'appendClipboardText', text: text.text }))
  })

  it('includes a clipboard import accepted while the earlier document queue is draining', async () => {
    const documents = deferred<void>()
    const waitForDocumentWrites = vi.fn(() => documents.promise)
    const h = harness({ waitForDocumentWrites })
    let prepared = false
    const preparation = h.view.prepareFeedback(request.request_id).then((result) => { prepared = true; return result })
    await vi.waitFor(() => expect(waitForDocumentWrites).toHaveBeenCalledOnce())
    const imported = h.view.importClipboardNow()
    await vi.waitFor(() => expect(h.captureOnce).toHaveBeenCalledOnce())
    try {
      documents.resolve()
      await new Promise((resolve) => setTimeout(resolve, 20))
      expect(prepared).toBe(false)
      h.capture.resolve(text)
      await vi.waitFor(() => expect(h.onRouteDraftOperation).toHaveBeenCalledOnce())
      h.write.resolve()
      await imported
      await expect(preparation).resolves.toEqual({ kind: 'ready' })
    } finally {
      documents.resolve()
      h.capture.resolve(text)
      h.write.resolve()
      await imported
    }
  })

  it('reports a failure from a clipboard import first accepted during preparation', async () => {
    const documents = deferred<void>()
    const waitForDocumentWrites = vi.fn(() => documents.promise)
    const h = harness({ waitForDocumentWrites })
    const preparation = h.view.prepareFeedback(request.request_id)
    await vi.waitFor(() => expect(waitForDocumentWrites).toHaveBeenCalledOnce())
    const imported = h.view.importClipboardNow()
    await vi.waitFor(() => expect(h.captureOnce).toHaveBeenCalledOnce())
    documents.resolve()
    h.capture.resolve(text)
    await vi.waitFor(() => expect(h.onRouteDraftOperation).toHaveBeenCalledOnce())
    h.write.reject(new Error('Late clipboard write failed'))
    await imported

    await expect(preparation).resolves.toMatchObject({
      kind: 'failed', message: expect.stringContaining('Late clipboard write failed'),
    })
  })

  it('reports a failed participating write instead of an input-ready result or success receipt', async () => {
    const h = harness()
    void h.view.importClipboardNow()
    await tick()
    const preparation = h.view.prepareFeedback(request.request_id)
    h.capture.resolve(text)
    await vi.waitFor(() => expect(h.onRouteDraftOperation).toHaveBeenCalledOnce())
    expect(get(h.session).message).not.toContain('items captured')
    h.write.reject(new Error('Draft could not be saved'))
    await expect(preparation).resolves.toMatchObject({ kind: 'failed', message: expect.stringContaining('Draft could not be saved') })
    expect(get(h.session).message).not.toContain('items captured')
  })

  it('releases a capture returned after the owning Client was destroyed without persisting it', async () => {
    const h = harness()
    const imported = h.view.importClipboardNow()
    await tick()
    await unmount(h.view)
    views.splice(views.indexOf(h.view), 1)
    const dispose = vi.fn(async () => {})
    h.capture.resolve({ kind: 'attachment', capturedAtMs: 1, candidate: {
      id: 'capture-1', source: 'clipboard-image', fileName: 'capture.png', mediaType: 'image/png',
      byteLength: 4, readBytes: async () => new ArrayBuffer(4), dispose,
    } })
    await imported
    expect(dispose).toHaveBeenCalledOnce()
    expect(h.onPersistAttachmentCandidates).not.toHaveBeenCalled()
  })
})
