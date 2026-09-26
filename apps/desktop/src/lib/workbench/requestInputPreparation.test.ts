import { describe, expect, it, vi } from 'vitest'

import type { FeedbackPreparation } from '../speech/rambleSessionControllerHandle'
import type { AttachmentPreparation } from './attachmentController'
import { createRequestInputPreparation } from './requestInputPreparation'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

function setup() {
  const context = {
    prepareSpeech: vi.fn(async (_requestId: string): Promise<FeedbackPreparation> => ({ kind: 'ready' })),
    prepareAttachments: vi.fn(async (_requestId: string): Promise<AttachmentPreparation> => ({ kind: 'ready' })),
    tr: (source: string) => source,
  }
  return { context, ...createRequestInputPreparation(context) }
}

describe('request input preparation', () => {
  it('drains speech and clipboard before attachments and checks arrivals during the drain', async () => {
    const { context, prepareFeedback } = setup()
    const calls: string[] = []
    context.prepareSpeech.mockImplementation(async () => { calls.push('speech'); return { kind: 'ready' } })
    context.prepareAttachments.mockImplementation(async () => { calls.push('attachments'); return { kind: 'ready' } })

    expect(await prepareFeedback('request-1')).toEqual({ kind: 'ready' })
    expect(calls).toEqual(['speech', 'attachments', 'speech', 'attachments'])
    expect(context.prepareSpeech).toHaveBeenCalledWith('request-1')
    expect(context.prepareAttachments).toHaveBeenCalledWith('request-1')
  })

  it.each([
    { kind: 'failed', message: 'Clipboard image failed' },
    { kind: 'pending-speech' },
  ] as const)('does not report ready when speech preparation is $kind', async (speech) => {
    const { context, prepareFeedback } = setup()
    context.prepareSpeech.mockResolvedValue(speech)
    expect(await prepareFeedback('request-1')).toEqual(speech)
    expect(context.prepareAttachments).not.toHaveBeenCalled()
  })

  it('propagates media failure and converts an open capture into an actionable failure', async () => {
    const { context, prepareFeedback } = setup()
    context.prepareAttachments.mockResolvedValueOnce({ kind: 'failed', message: 'Upload failed' })
    expect(await prepareFeedback('request-1')).toEqual({ kind: 'failed', message: 'Upload failed' })
    context.prepareAttachments.mockResolvedValueOnce({ kind: 'pending-capture' })
    expect(await prepareFeedback('request-1')).toEqual({
      kind: 'failed', message: 'Finish or cancel the pending screen capture before ending this request.',
    })
  })

  it('shares concurrent preparation for one request without blocking another request', async () => {
    const draining = deferred<FeedbackPreparation>()
    const { context, prepareFeedback } = setup()
    context.prepareSpeech.mockImplementation((requestId) => requestId === 'request-1' ? draining.promise : Promise.resolve({ kind: 'ready' }))
    const first = prepareFeedback('request-1')
    expect(prepareFeedback('request-1')).toBe(first)
    expect(await prepareFeedback('request-2')).toEqual({ kind: 'ready' })
    draining.resolve({ kind: 'ready' })
    expect(await first).toEqual({ kind: 'ready' })
    expect(prepareFeedback('request-1')).not.toBe(first)
  })

  it('waits for a clipboard arrival during the first attachment drain', async () => {
    const clipboard = deferred<FeedbackPreparation>()
    const { context, prepareFeedback } = setup()
    let clipboardArrived = false
    let clipboardSaved = false
    let attachmentPass = 0
    context.prepareSpeech.mockImplementation(async () => {
      if (!clipboardArrived) return { kind: 'ready' }
      const result = await clipboard.promise
      clipboardSaved = true
      return result
    })
    context.prepareAttachments.mockImplementation(async () => {
      if (++attachmentPass === 1) clipboardArrived = true
      else expect(clipboardSaved).toBe(true)
      return { kind: 'ready' }
    })
    let ready = false
    const preparing = prepareFeedback('request-1').then((result) => { ready = true; return result })
    await vi.waitFor(() => expect(context.prepareSpeech).toHaveBeenCalledTimes(2))
    expect(ready).toBe(false)
    clipboard.resolve({ kind: 'ready' })
    expect(await preparing).toEqual({ kind: 'ready' })
  })
})
