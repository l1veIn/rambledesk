/** Small fixture byte store. Every read/write owns its buffer and its request identity. */
export class PreviewAttachmentBytes {
  readonly #requests = new Map<string, Map<string, ArrayBuffer>>()
  put(requestId: string, attachmentId: string, contents: ArrayBuffer) {
    const attachments = this.#requests.get(requestId) ?? new Map<string, ArrayBuffer>()
    attachments.set(attachmentId, contents.slice(0)); this.#requests.set(requestId, attachments)
  }
  read(requestId: string, attachmentId: string): ArrayBuffer {
    const contents = this.#requests.get(requestId)?.get(attachmentId)
    if (!contents) throw new Error('The preview attachment could not be found for this request.')
    return contents.slice(0)
  }
  remove(requestId: string, attachmentId: string) { this.#requests.get(requestId)?.delete(attachmentId) }
  removeRequest(requestId: string) { this.#requests.delete(requestId) }
}

export function previewMediaType(fileName: string, contents: ArrayBuffer): string {
  const bytes = new Uint8Array(contents), prefix = (...signature: number[]) => signature.every((byte, index) => bytes[index] === byte)
  if (prefix(137, 80, 78, 71, 13, 10, 26, 10)) return 'image/png'
  if (prefix(255, 216, 255)) return 'image/jpeg'
  if (prefix(71, 73, 70, 56, 55, 97) || prefix(71, 73, 70, 56, 57, 97)) return 'image/gif'
  if (prefix(82, 73, 70, 70) && bytes[8] === 87 && bytes[9] === 69 && bytes[10] === 66 && bytes[11] === 80) return 'image/webp'
  if (prefix(37, 80, 68, 70, 45)) return 'application/pdf'
  const extension = fileName.split('.').at(-1)?.toLowerCase()
  return ({ md: 'text/markdown', markdown: 'text/markdown', txt: 'text/plain', log: 'text/plain', json: 'application/json', csv: 'text/csv' } as Record<string, string>)[extension ?? ''] ?? 'application/octet-stream'
}
