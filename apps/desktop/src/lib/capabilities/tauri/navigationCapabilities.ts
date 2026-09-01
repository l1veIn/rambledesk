import type {
  ExternalLinkCapability,
  ServerPathCapability,
  TrayCapability,
} from '../workbenchCapabilities'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

export function createTauriTrayCapability(api: TauriCapabilityApi): TrayCapability {
  return { setPendingCount: (count) => api.invoke<void>('set_pending_count', { count }) }
}

export function createTauriExternalLinkCapability(
  api: TauriCapabilityApi,
): ExternalLinkCapability {
  return { open: (url) => api.openUrl(url) }
}

export function createTauriServerPathCapability(
  api: TauriCapabilityApi,
): ServerPathCapability {
  return {
    async chooseDirectory() {
      const selected = await api.choosePath({ directory: true, multiple: false })
      return typeof selected === 'string' ? selected : null
    },
    async chooseFile(input) {
      const selected = await api.choosePath({
        directory: false,
        multiple: false,
        filters: [{ name: 'Files', extensions: [...input.extensions] }],
      })
      return typeof selected === 'string' ? selected : null
    },
    chooseSaveFile: (input) =>
      api.savePath({
        defaultPath: input.defaultName,
        filters: [{ name: 'Files', extensions: [...input.extensions] }],
      }),
    reveal: (path) => api.invoke<void>('reveal_path_in_folder', { path }),
    openAttachment: (input) =>
      api.invoke<string>('open_feedback_attachment', {
        input: attachmentInput(input),
      }),
    revealAttachment: (input) =>
      api.invoke<string>('reveal_feedback_attachment', {
        input: attachmentInput(input),
      }),
    onFileDrop(handler, onError) {
      let active = true
      let unlisten: (() => void) | undefined
      void api
        .currentWebview()
        .onDragDropEvent(({ payload }) => handler(payload))
        .then((nextUnlisten) => {
          if (active) unlisten = nextUnlisten
          else nextUnlisten()
        })
        .catch((cause) => {
          if (active) onError(cause)
        })
      return () => {
        if (!active) return
        active = false
        unlisten?.()
      }
    },
    importAttachmentPath: (input) =>
      api.invoke('import_feedback_attachment_path', {
        requestId: input.requestId,
        path: input.path,
        expectedRevision: input.expectedRevision,
      }),
  }
}

function attachmentInput(input: {
  requestId: string
  attachmentId: string
  kind: 'request' | 'workspace'
}) {
  return {
    requestId: input.requestId,
    attachmentId: input.attachmentId,
    kind: input.kind,
  }
}
