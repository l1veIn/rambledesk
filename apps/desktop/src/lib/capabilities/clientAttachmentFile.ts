import {
  defineAttachmentCandidate,
  type AttachmentCandidate,
  type AttachmentCandidateSource,
} from './capturePlugin'

export type ClientAttachmentFile = Readonly<{
  fileName: string
  mediaType: string
  byteLength: number
  readBytes: () => Promise<ArrayBuffer>
}>

export type ClientAttachmentFileSource = Readonly<{
  name: string
  type: string
  size: number
  arrayBuffer: () => Promise<ArrayBuffer>
}>

/** A Client-local file projection. It deliberately cannot carry a Server path. */
export function clientAttachmentFile(
  file: ClientAttachmentFileSource,
): ClientAttachmentFile {
  return Object.freeze({
    fileName: file.name,
    mediaType: file.type,
    byteLength: file.size,
    readBytes: () => file.arrayBuffer(),
  })
}

/**
 * Transitional projection for DOM File inputs that still expose the older
 * ClientAttachmentFile interface. The shared Draft flow owns the returned
 * candidate and may dispose it without knowing that a browser File needs no
 * explicit cleanup.
 */
export function clientAttachmentCandidate(
  file: ClientAttachmentFile,
  source: AttachmentCandidateSource = 'file-input',
): AttachmentCandidate {
  return defineAttachmentCandidate({
    id: crypto.randomUUID(),
    source,
    fileName: file.fileName,
    mediaType: file.mediaType,
    byteLength: file.byteLength,
    readBytes: file.readBytes,
    dispose: async () => undefined,
  })
}
