import type { WorkbenchExample } from './contracts'
import { workbenchDefinitions } from './registry'

/** Both preview surfaces discover examples from the same registration as production dispatch. */
export const registeredWorkbenchExamples = workbenchDefinitions.flatMap((definition) => [...(definition.examples ?? [])])
  .sort((a, b) => a.order - b.order || a.title.localeCompare(b.title))
export const workbenchExampleKey = (example: WorkbenchExample) => example.key ?? `${example.spec.type}:${example.order}`
/** A unique example key takes precedence; a type continues to select its first example. */
export function findWorkbenchExample(selector: string | null | undefined) {
  return registeredWorkbenchExamples.find((example) => workbenchExampleKey(example) === selector)
    ?? registeredWorkbenchExamples.find((example) => example.spec.type === selector)
}
export function previewExampleAttachments(example: WorkbenchExample) {
  return (example.attachments ?? []).map((attachment, index) => {
    const contents = attachment.contentsBase64 !== undefined
      ? Uint8Array.from(atob(attachment.contentsBase64), (character) => character.charCodeAt(0)).buffer
      : new TextEncoder().encode(attachment.content).buffer
    const attachment_id = attachment.id ?? `example-${example.spec.type}-${example.order}-${index}`
    return { attachment_id, file_name: attachment.name, contents,
      media_type: attachment.mimeType ?? (attachment.contentsBase64 === undefined ? 'text/markdown' : 'application/octet-stream'), byte_size: contents.byteLength,
      sha256: `preview-${attachment_id}`, position: index }
  })
}
