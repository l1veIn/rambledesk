import { get, writable } from 'svelte/store'

export type AttachmentMessageTone = 'info' | 'success' | 'error'

/**
 * Runtime state for attachment capture and import: what is in flight, the preview
 * URLs the editor shows, and the status line the workbench reports.
 */
export type AttachmentSessionState = Readonly<{
  busy: boolean
  captureBusy: boolean
  message: string
  tone: AttachmentMessageTone
  previews: Record<string, string>
  dragActive: boolean
}>

const initial: AttachmentSessionState = {
  busy: false,
  captureBusy: false,
  message: '',
  tone: 'info',
  previews: {},
  dragActive: false,
}

export type AttachmentSession = ReturnType<typeof createAttachmentSession>

export function createAttachmentSession() {
  const store = writable<AttachmentSessionState>(initial)

  function patch(next: Partial<AttachmentSessionState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  function setBusy(busy: boolean) {
    patch({ busy })
  }

  function setCaptureBusy(captureBusy: boolean) {
    patch({ captureBusy })
  }

  function setMessage(message: string, tone?: AttachmentMessageTone) {
    patch({ message, ...(tone ? { tone } : {}) })
  }

  function setPreviews(previews: Record<string, string>) {
    patch({ previews })
  }

  function setDragActive(dragActive: boolean) {
    patch({ dragActive })
  }

  return {
    subscribe: store.subscribe,
    setBusy,
    setCaptureBusy,
    setMessage,
    setPreviews,
    setDragActive,
    busy: () => get(store).busy,
    captureBusy: () => get(store).captureBusy,
  }
}
