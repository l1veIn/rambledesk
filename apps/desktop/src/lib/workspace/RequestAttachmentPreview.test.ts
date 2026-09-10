// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { TestApplicationTransport } from '../application/testApplicationTransport'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import { toast } from '../components/ui/sonner'
import type { AttachmentView } from '../feedback'
import { locale } from '../preferences'
import AttachmentPreviewHarness from './fixtures/AttachmentPreviewHarness.svelte'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail })
  return { promise, resolve, reject }
}

const attachment = (id: string, media = 'text/plain'): AttachmentView => ({
  attachment_id: id, file_name: `${id}.txt`, media_type: media,
  byte_size: 1, sha256: id, position: 0,
})

let host: HTMLDivElement
let view: {
  show: (id: string, item: AttachmentView, kind?: 'request' | 'workspace') => void
  close: () => void
} | undefined

beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  host = document.createElement('div')
  document.body.append(host)
})

afterEach(async () => {
  if (view) {
    view.close()
    await tick()
    await unmount(view)
  }
  view = undefined
  host.remove()
  document.body.replaceChildren()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

async function setup(options: { openAttachment?: () => Promise<string>; revealAttachment?: () => Promise<string> } = {}) {
  const read = vi.fn(async () => new Uint8Array([1]).buffer)
  const transport = new TestApplicationTransport(undefined)
    .handle('readRequestAttachment', read)
    .handle('readFeedbackAttachment', read)
  const base = createUnavailableWorkbenchCapabilities()
  view = mount(AttachmentPreviewHarness, {
    target: host,
    props: { transport, capabilities: { serverPaths: {
      status: { availability: 'available', source: 'native' },
      implementation: { ...base.serverPaths.implementation, ...options },
    } } },
  })
  await tick()
  return { read, transport, view }
}

function clickButton(text: string) {
  const button = Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.includes(text))
  expect(button).toBeDefined()
  button!.click()
}

describe('attachment preview ownership', () => {
  it('does not report an old external-open result inside a newly selected attachment', async () => {
    const opened = deferred<string>()
    const { view } = await setup({ openAttachment: () => opened.promise })
    view.show('request-a', attachment('a'))
    await vi.waitFor(() => expect(document.body.textContent).toContain('This file type cannot be previewed.'))
    clickButton('Open with the system default app')
    view.show('request-b', attachment('b'))
    await tick()
    opened.resolve('/old/a.txt')
    await tick()
    await tick()

    expect(document.body.textContent).not.toContain('/old/a.txt')
  })

  it('does not toast a reveal failure after its dialog has closed', async () => {
    const revealed = deferred<string>()
    const toastError = vi.spyOn(toast, 'error').mockReturnValue('toast')
    const { view } = await setup({ revealAttachment: () => revealed.promise })
    view.show('request-a', attachment('a'))
    await vi.waitFor(() => expect(document.body.textContent).toContain('Show in folder'))
    clickButton('Show in folder')
    view.close()
    await tick()
    revealed.reject(new Error('Old native reveal failed'))
    await tick()
    await tick()

    expect(toastError).not.toHaveBeenCalled()
  })

  it('reloads when the attachment read capability changes', async () => {
    const { view, read } = await setup()
    view.show('request-a', attachment('a'), 'request')
    await vi.waitFor(() => expect(read).toHaveBeenCalledOnce())
    view.show('request-a', attachment('a'), 'workspace')
    await vi.waitFor(() => expect(read).toHaveBeenCalledTimes(2))
  })

  it('retains an image URL until its pending decoder releases it after close', async () => {
    const decoded = deferred<void>()
    const create = vi.fn(() => 'blob:pending-image')
    const revoke = vi.fn()
    vi.stubGlobal('URL', { createObjectURL: create, revokeObjectURL: revoke })
    vi.stubGlobal('Image', class {
      src = ''
      naturalWidth = 100
      naturalHeight = 100
      decode() { return decoded.promise }
    })
    const { view } = await setup()
    view.show('request-a', attachment('a', 'image/png'))
    await vi.waitFor(() => expect(create).toHaveBeenCalledOnce())
    view.close()
    await tick()
    expect(revoke).not.toHaveBeenCalled()
    decoded.resolve()
    await vi.waitFor(() => expect(revoke).toHaveBeenCalledWith('blob:pending-image'))
    expect(document.querySelector('img')).toBeNull()
  })
})
