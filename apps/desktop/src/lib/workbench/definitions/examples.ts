import type { WorkbenchExample } from './contracts'
import { workbenchDefinitions } from './registry'

/** Both preview surfaces discover examples from the same registration as production dispatch. */
export const registeredWorkbenchExamples = workbenchDefinitions.flatMap((definition) => [...(definition.examples ?? [])])
  .sort((a, b) => a.order - b.order || a.title.localeCompare(b.title))
export function previewExampleAttachments(example: WorkbenchExample) {
  return (example.attachments ?? []).map((attachment, index) => ({
    attachment_id: attachment.id ?? `example-${example.spec.type}-${example.order}-${index}`,
    file_name: attachment.name, markdown: attachment.content,
  }))
}
