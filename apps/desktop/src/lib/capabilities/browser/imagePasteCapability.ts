import {
  clientAttachmentFile,
  type ClientAttachmentFile,
} from '../clientAttachmentFile'
import type { ImagePasteCapability } from '../workbenchCapabilities'

export function createBrowserImagePasteCapability(): ImagePasteCapability {
  return {
    subscribe(target, handler, onError) {
      let active = true
      const listener = (event: Event) => {
        if (!active) return
        try {
          const files = pastedImageFiles(event)
          if (files.length === 0 || !handler(files)) return
          event.preventDefault()
          event.stopPropagation()
        } catch (cause) {
          onError(cause)
        }
      }
      target.addEventListener('paste', listener, true)
      return () => {
        if (!active) return
        active = false
        target.removeEventListener('paste', listener, true)
      }
    },
  }
}

function pastedImageFiles(event: Event): readonly ClientAttachmentFile[] {
  if (!('clipboardData' in event)) return []
  const clipboardData = (event as ClipboardEvent).clipboardData
  if (!clipboardData) return []

  const itemFiles = uniqueFiles(
    Array.from(clipboardData.items)
      .filter((item) => item.kind === 'file' && item.type.startsWith('image/'))
      .map((item) => item.getAsFile())
      .filter((file): file is File => file !== null),
  )
  const files = itemFiles.length > 0
    ? itemFiles
    : uniqueFiles(
        Array.from(clipboardData.files).filter((file) => file.type.startsWith('image/')),
      )
  return files.map(clientAttachmentFile)
}

function uniqueFiles(files: readonly File[]): File[] {
  return [...new Set(files)]
}
