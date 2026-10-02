import type { FeedbackWorkspaceView } from '$lib/feedback'
import { previewFixtures } from '$lib/preview/previewFixtures'
import { registeredWorkbenchExamples, previewExampleAttachments } from '$lib/workbench/definitions/examples'

export const workbenchExamples = registeredWorkbenchExamples.map((example) => example.spec)
export const workbenchPreviewLabels = registeredWorkbenchExamples.map((example) => example.title)
export const workbenchPreviewAttachments = registeredWorkbenchExamples.flatMap(previewExampleAttachments)

export function workbenchPreviewWorkspace(index: number): FeedbackWorkspaceView {
  const workspace = structuredClone(previewFixtures.workspace)
  const example = registeredWorkbenchExamples[index]
  if (!example) throw new Error('Unknown workbench preview.')
  workspace.workbench = structuredClone(example.spec)
  workspace.request = { ...workspace.request, request_id: `workbench-preview-${index}`, title: example.title,
    what_happened: example.markdown, status: 'in_progress', resolution: null, allow_finish: false, final_summary: null }
  workspace.actions = structuredClone([...(example.actions ?? [])])
  workspace.context_refs = []
  workspace.request_attachments = previewExampleAttachments(example).map(({ markdown, ...attachment }, position) => ({
    ...attachment, media_type: 'text/markdown', byte_size: new TextEncoder().encode(markdown).byteLength,
    sha256: `preview-${attachment.attachment_id}`, position,
  }))
  workspace.attachments = []
  workspace.draft = { document_json: null, body_markdown: '', saved_revision: 0, updated_at: null }
  workspace.feedback = null
  return workspace
}
