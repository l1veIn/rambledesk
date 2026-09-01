import { describe, expect, it, vi } from 'vitest'

import {
  defineAttachmentCandidate,
  type ClipboardCapturePlugin,
  type ImagePastePlugin,
  type ScreenCapturePlugin,
} from './capturePlugin'

describe('AttachmentCandidate contract', () => {
  it('exposes only acquisition metadata and client-local byte lifecycle methods', async () => {
    const dispose = vi.fn(async () => undefined)
    const candidate = defineAttachmentCandidate({
      id: 'candidate-1',
      source: 'screen-capture',
      fileName: 'screen.png',
      mediaType: 'image/png',
      byteLength: 3,
      readBytes: async () => new Uint8Array([1, 2, 3]).buffer,
      dispose,
    })

    expect(Object.keys(candidate)).toEqual([
      'id',
      'source',
      'fileName',
      'mediaType',
      'byteLength',
      'readBytes',
      'dispose',
    ])
    expect(candidate).not.toHaveProperty('requestId')
    expect(candidate).not.toHaveProperty('expectedRevision')
    expect(candidate).not.toHaveProperty('path')
    await expect(candidate.readBytes()).resolves.toEqual(
      new Uint8Array([1, 2, 3]).buffer,
    )
    expect(Object.isFrozen(candidate)).toBe(true)
  })

  it('disposes its backing resource exactly once, including concurrent callers', async () => {
    const dispose = vi.fn(async () => undefined)
    const candidate = defineAttachmentCandidate({
      id: 'candidate-1',
      source: 'clipboard-image',
      fileName: 'clipboard.png',
      mediaType: 'image/png',
      byteLength: 1,
      readBytes: async () => new ArrayBuffer(1),
      dispose,
    })

    const first = candidate.dispose()
    const second = candidate.dispose()
    expect(first).toBe(second)
    await Promise.all([first, second, candidate.dispose()])
    expect(dispose).toHaveBeenCalledOnce()
  })
})

describe('Capture Plugin contracts', () => {
  it('keep acquisition interfaces free of Draft and transport arguments', () => {
    const screen = {
      onCandidate: vi.fn(() => vi.fn()),
      onFinished: vi.fn(() => vi.fn()),
      onShortcut: vi.fn(() => vi.fn()),
      begin: vi.fn(async () => undefined),
    } satisfies ScreenCapturePlugin
    const clipboard = {
      captureOnce: vi.fn(async () => ({
        kind: 'text' as const,
        text: 'note',
        capturedAtMs: 1,
        truncated: false,
      })),
    } satisfies ClipboardCapturePlugin
    const imagePaste = {
      subscribe: vi.fn(() => vi.fn()),
    } satisfies ImagePastePlugin

    expect(Object.keys(screen)).toEqual([
      'onCandidate',
      'onFinished',
      'onShortcut',
      'begin',
    ])
    expect(Object.keys(clipboard)).toEqual(['captureOnce'])
    expect(Object.keys(imagePaste)).toEqual(['subscribe'])
  })
})
