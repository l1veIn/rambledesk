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
