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
