import { describe, expect, it, vi } from 'vitest'

const toast = vi.hoisted(() => ({
  error: vi.fn(),
  success: vi.fn(),
  info: vi.fn(),
}))
vi.mock('../components/ui/sonner', () => ({ toast }))

import { createMessageToaster } from './messageToasts'

function toaster() {
  toast.error.mockReset()
  toast.success.mockReset()
  toast.info.mockReset()
  return createMessageToaster({ tr: (source) => source })
}

describe('message toasts', () => {
  it('delivers each new page error once', () => {
    const messages = toaster()
    messages.pageError('boom')
    messages.pageError('boom')
    expect(toast.error).toHaveBeenCalledTimes(1)
    expect(toast.error).toHaveBeenCalledWith('Operation failed', { description: 'boom' })

    messages.pageError('other')
    expect(toast.error).toHaveBeenCalledTimes(2)
  })

  it('resets a channel once the message clears', () => {
    const messages = toaster()
    messages.pageError('boom')
    messages.pageError('')
    messages.pageError('boom')
    expect(toast.error).toHaveBeenCalledTimes(2)
  })

  it('keeps save and page errors on separate cursors', () => {
    const messages = toaster()
    messages.pageError('same text')
    messages.saveError('same text')
    expect(toast.error).toHaveBeenCalledTimes(2)
    expect(toast.error).toHaveBeenLastCalledWith('Save failed', { description: 'same text' })
  })

  it('maps the attachment tone to the matching toast kind', () => {
    const messages = toaster()
    messages.attachmentMessage('inserted', 'success')
    messages.attachmentMessage('working', 'info')
    messages.attachmentMessage('failed', 'error')
    expect(toast.success).toHaveBeenCalledWith('Attachment action completed', {
      description: 'inserted',
    })
    expect(toast.info).toHaveBeenCalledWith('Attachment status', { description: 'working' })
    expect(toast.error).toHaveBeenCalledWith('Attachment action failed', { description: 'failed' })
  })
})
