import {
  type AttachmentCandidate,
  type ImagePastePlugin,
} from '../capturePlugin'
import { fileAttachmentCandidate } from '../fileAttachmentCandidate'

export function createBrowserImagePastePlugin(): ImagePastePlugin {
  return {
    subscribe(target, handler, onError) {
      let active = true
      const listener = (event: Event) => {
        if (!active) return
        let candidates: readonly AttachmentCandidate[] = []
        try {
          candidates = pastedImageCandidates(event)
          if (candidates.length === 0) return
          if (!handler(candidates)) {
            void disposeCandidates(candidates)
            return
          }
          event.preventDefault()
          event.stopPropagation()
        } catch (cause) {
          void disposeCandidates(candidates)
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

async function disposeCandidates(candidates: readonly AttachmentCandidate[]) {
  await Promise.allSettled(candidates.map((candidate) => candidate.dispose()))
}

function pastedImageCandidates(event: Event): readonly AttachmentCandidate[] {
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
  return files.map((file) => fileAttachmentCandidate(file, 'image-paste'))
}

function uniqueFiles(files: readonly File[]): File[] {
  return [...new Set(files)]
}
