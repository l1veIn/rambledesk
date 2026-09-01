import {
  defineAttachmentCandidate,
  type AttachmentCandidate,
} from './capturePlugin'

export type FileAttachmentSource = 'file-input' | 'image-paste'

export type FileAttachmentCandidateSource = Readonly<{
  name: string
  type: string
  size: number
  arrayBuffer: () => Promise<ArrayBuffer>
}>

/** Projects a browser File/Blob-like source directly into the capture seam. */
export function fileAttachmentCandidate(
  file: FileAttachmentCandidateSource,
  source: FileAttachmentSource,
): AttachmentCandidate {
  return defineAttachmentCandidate({
    id: crypto.randomUUID(),
    source,
    fileName: file.name,
    mediaType: file.type,
    byteLength: file.size,
    readBytes: () => file.arrayBuffer(),
    dispose: async () => undefined,
  })
}
