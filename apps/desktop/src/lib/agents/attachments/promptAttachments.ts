import type { AgentPromptCapabilities, SessionPromptContent } from '$lib/generated/feedback'

export type PromptAttachment = Readonly<{
  id: string
  name: string
  detail: string
  content: Exclude<SessionPromptContent, { type: 'text' }>
}>

export const MAX_IMAGE_BYTES = 1_572_864
export const MAX_TEXT_BYTES = 256 * 1024
export const MAX_CONTENT_BYTES = 4 * 1024 * 1024
export const MAX_CONTENT_BLOCKS = 16
const encoder = new TextEncoder()
const textBytes = (value: string) => encoder.encode(value).byteLength
const IMAGE_MIMES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp']
const TEXT_EXTENSIONS = '.txt,.md,.json,.yaml,.yml,.toml,.rs,.ts,.tsx,.js,.jsx,.py,.go,.java,.c,.h,.cpp,.css,.html,.xml,.sh,.ps1,.sql,.log,.csv,.ini,.conf,.svelte,.vue'

export function attachmentAccept(capabilities: AgentPromptCapabilities): string {
  return [capabilities.image ? IMAGE_MIMES.join(',') : '', capabilities.embedded_context ? `text/*,${TEXT_EXTENSIONS}` : ''].filter(Boolean).join(',')
}

export function canAttachFiles(capabilities: AgentPromptCapabilities): boolean {
  return capabilities.image || capabilities.embedded_context
}

function imageMime(bytes: Uint8Array): string | null {
  const starts = (prefix: number[]) => prefix.every((byte, index) => bytes[index] === byte)
  if (starts([137, 80, 78, 71, 13, 10, 26, 10])) return 'image/png'
  if (starts([255, 216, 255])) return 'image/jpeg'
  const signature = String.fromCharCode(...bytes.subarray(0, 12))
  if (signature.startsWith('GIF87a') || signature.startsWith('GIF89a')) return 'image/gif'
  if (signature.startsWith('RIFF') && signature.slice(8, 12) === 'WEBP') return 'image/webp'
  return null
}

function base64(bytes: Uint8Array): string {
  const chunks: string[] = []
  for (let index = 0; index < bytes.length; index += 0x8000) chunks.push(String.fromCharCode(...bytes.subarray(index, index + 0x8000)))
  return btoa(chunks.join(''))
}

/** Reads only explicit chooser/paste Files. It never resolves or guesses project paths. */
export async function readPromptFiles(files: readonly File[], capabilities: AgentPromptCapabilities): Promise<PromptAttachment[]> {
  if (files.length > MAX_CONTENT_BLOCKS) throw new Error('A message can contain at most 16 content blocks.')
  const attachments: PromptAttachment[] = []
  for (const file of files) {
    if (file.size > MAX_IMAGE_BYTES) throw new Error('Images must be 1.5 MiB or smaller; text files must be 256 KiB or smaller.')
    const bytes = new Uint8Array(await file.arrayBuffer())
    const mime = imageMime(bytes)
    const id = crypto.randomUUID()
    const name = file.name || (mime ? `image.${mime.split('/')[1]}` : 'attachment.txt')
    if (name.length > 512 || /[\\/\u0000-\u001f\u007f]/.test(name)) throw new Error('The attachment filename is invalid.')
    if (mime) {
      if (!capabilities.image) throw new Error('This agent does not accept image attachments.')
      attachments.push({ id, name, detail: `${mime} · ${bytes.byteLength} bytes`, content: { type: 'image', mime_type: mime, data: base64(bytes) } })
      continue
    }
    if (file.type.startsWith('image/') || file.type.startsWith('audio/') || file.type.startsWith('video/')) throw new Error('Choose a PNG, JPEG, GIF, WebP image or a UTF-8 text file.')
    if (!capabilities.embedded_context) throw new Error('This agent does not accept text-file attachments.')
    if (bytes.byteLength > MAX_TEXT_BYTES) throw new Error('Text files must be 256 KiB or smaller.')
    let text: string
    try { text = new TextDecoder('utf-8', { fatal: true }).decode(bytes) }
    catch { throw new Error('Text attachments must use UTF-8 encoding.') }
    if (/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(text)) throw new Error('This file contains binary data. Choose a UTF-8 text file.')
    const mimeType = /^text\/[!-~]+$/.test(file.type) ? file.type : 'text/plain'
    attachments.push({ id, name, detail: `${mimeType} · ${bytes.byteLength} bytes`, content: {
      type: 'resource', uri: `ramble-attachment://${id}/${encodeURIComponent(name)}`, mime_type: mimeType, text,
    } })
  }
  return attachments
}

/** These bounds match the typed command; reject before clearing a draft or sending. */
export function validatePromptAttachments(text: string, attachments: readonly PromptAttachment[], capabilities: AgentPromptCapabilities): void {
  if (attachments.length + (text ? 1 : 0) > MAX_CONTENT_BLOCKS) throw new Error('A message can contain at most 16 content blocks.')
  let total = textBytes(text)
  let prose = total
  for (const attachment of attachments) {
    const block = attachment.content
    if (block.type === 'image') {
      if (!capabilities.image) throw new Error('This agent does not accept image attachments.')
      total += block.data.length + textBytes(block.mime_type)
    } else if (block.type === 'resource') {
      if (!capabilities.embedded_context) throw new Error('This agent does not accept text-file attachments.')
      prose += textBytes(block.text)
      total += textBytes(block.uri) + textBytes(block.mime_type ?? '') + textBytes(block.text)
    } else {
      if (!capabilities.resource_links) throw new Error('This agent does not accept resource links.')
      total += textBytes(block.uri) + textBytes(block.name) + textBytes(block.mime_type ?? '')
    }
  }
  if (prose > MAX_TEXT_BYTES) throw new Error('Message text and text attachments together must be 256 KiB or smaller.')
  if (total > MAX_CONTENT_BYTES) throw new Error('Message content must be 4 MiB or smaller.')
}
