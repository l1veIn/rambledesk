import { toast } from '../components/ui/sonner'
import type { AttachmentMessageTone } from './attachmentSession'

/**
 * Turns store messages into toasts exactly once per new message. Each channel
 * keeps its own cursor so a repeated message never re-toasts, and clearing the
 * message resets the channel.
 */
export function createMessageToaster(context: { tr: (source: string) => string }) {
  const delivered = new Map<string, string>()

  function deliver(
    channel: string,
    message: string,
    present: (message: string) => void,
  ) {
    if (!message) {
      delivered.delete(channel)
      return
    }
    if (delivered.get(channel) === message) return
    delivered.set(channel, message)
    present(message)
  }

  function pageError(message: string) {
    deliver('page-error', message, (current) => {
      toast.error(context.tr('Operation failed'), { description: current })
    })
  }

  function saveError(message: string) {
    deliver('save-error', message, (current) => {
      toast.error(context.tr('Save failed'), { description: current })
    })
  }

  function attachmentMessage(message: string, tone: AttachmentMessageTone) {
    deliver('attachment', message, (current) => {
      const options = { description: current }
      if (tone === 'success') toast.success(context.tr('Attachment action completed'), options)
      else if (tone === 'info') toast.info(context.tr('Attachment status'), options)
      else toast.error(context.tr('Attachment action failed'), options)
    })
  }

  return { pageError, saveError, attachmentMessage }
}

export type MessageToaster = ReturnType<typeof createMessageToaster>
