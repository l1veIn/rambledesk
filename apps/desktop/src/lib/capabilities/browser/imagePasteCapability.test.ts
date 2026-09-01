import { describe, expect, it, vi } from 'vitest'

import type { AttachmentCandidate } from '../capturePlugin'
import { createBrowserImagePastePlugin } from './imagePasteCapability'

type PasteEventFixture = Event & {
  clipboardData: {
    items: Array<{ kind: string; type: string; getAsFile: () => File | null }>
    files: File[]
  }
}

function file(name: string, type: string, bytes = [1, 2, 3]): File {
  return new File([new Uint8Array(bytes)], name, { type, lastModified: 123 })
}

function pasteEvent(input: {
  items?: PasteEventFixture['clipboardData']['items']
  files?: File[]
}): PasteEventFixture {
  const event = new Event('paste', { bubbles: true, cancelable: true }) as PasteEventFixture
  Object.defineProperty(event, 'clipboardData', {
    value: {
      items: input.items ?? [],
      files: input.files ?? [],
    },
  })
  return event
}

describe('browser image-paste capability', () => {
  it('uses image items before files, deduplicates them, and owns accepted mixed image and text paste', async () => {
    const target = new EventTarget()
    const image = file('screen.png', 'image/png')
    const fallback = file('fallback.png', 'image/png', [4])
    const handler = vi.fn((_candidates: readonly AttachmentCandidate[]) => true)

    const unsubscribe = createBrowserImagePastePlugin().subscribe(
      target,
      handler,
      vi.fn(),
    )
    const event = pasteEvent({
      items: [
        { kind: 'file', type: 'image/png', getAsFile: () => image },
        { kind: 'file', type: 'image/png', getAsFile: () => image },
        { kind: 'string', type: 'text/plain', getAsFile: () => null },
      ],
      files: [fallback],
    })
    const stopPropagation = vi.spyOn(event, 'stopPropagation')

    target.dispatchEvent(event)

    expect(handler).toHaveBeenCalledTimes(1)
    const [candidates] = handler.mock.calls[0]!
    expect(candidates).toHaveLength(1)
    expect(candidates[0]).toMatchObject({
      source: 'image-paste',
      fileName: 'screen.png',
      mediaType: 'image/png',
      byteLength: 3,
    })
    expect(candidates[0]).not.toHaveProperty('path')
    await expect(candidates[0]!.readBytes()).resolves.toEqual(new Uint8Array([1, 2, 3]).buffer)
    expect(event.defaultPrevented).toBe(true)
    expect(stopPropagation).toHaveBeenCalledTimes(1)

    unsubscribe()
    target.dispatchEvent(pasteEvent({ files: [image] }))
    expect(handler).toHaveBeenCalledTimes(1)
  })

  it('falls back to clipboard files and leaves rejected image paste untouched', () => {
    const target = new EventTarget()
    const image = file('fallback.png', 'image/png')
    const handler = vi.fn((_candidates: readonly AttachmentCandidate[]) => false)
    const bubbleListener = vi.fn()
    target.addEventListener('paste', bubbleListener)
    createBrowserImagePastePlugin().subscribe(target, handler, vi.fn())
    const event = pasteEvent({ files: [image, file('notes.txt', 'text/plain')] })

    target.dispatchEvent(event)

    expect(handler).toHaveBeenCalledTimes(1)
    expect(handler.mock.calls[0]![0]).toHaveLength(1)
    expect(event.defaultPrevented).toBe(false)
    expect(bubbleListener).toHaveBeenCalledTimes(1)
  })

  it('leaves ordinary text paste untouched and reports extraction failures without intercepting', () => {
    const target = new EventTarget()
    const handler = vi.fn((_candidates: readonly AttachmentCandidate[]) => true)
    const onError = vi.fn()
    const bubbleListener = vi.fn()
    target.addEventListener('paste', bubbleListener)
    createBrowserImagePastePlugin().subscribe(target, handler, onError)

    const textEvent = pasteEvent({
      items: [{ kind: 'string', type: 'text/plain', getAsFile: () => null }],
    })
    target.dispatchEvent(textEvent)
    const failedEvent = pasteEvent({
      items: [{ kind: 'file', type: 'image/png', getAsFile: () => { throw new Error('clipboard failed') } }],
    })
    target.dispatchEvent(failedEvent)

    expect(handler).not.toHaveBeenCalled()
    expect(textEvent.defaultPrevented).toBe(false)
    expect(failedEvent.defaultPrevented).toBe(false)
    expect(bubbleListener).toHaveBeenCalledTimes(2)
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'clipboard failed' }))
  })
})
