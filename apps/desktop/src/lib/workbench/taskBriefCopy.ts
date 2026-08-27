import type { FeedbackWorkspaceView } from '../feedback'

/**
 * Builds a plain-text copy of the task brief: title, "What happened", the
 * ordered actions, context references, and attachment names. Used by the
 * "Copy task brief" affordance in the fullscreen preview.
 */
export function buildTaskBriefText(workspace: FeedbackWorkspaceView): string {
  const parts: string[] = [workspace.request.title]

  const whatHappened = workspace.request.what_happened.trim()
  if (whatHappened) parts.push(whatHappened)

  const actions = workspace.actions.map((action, index) => `${index + 1}. ${action.instruction.trim()}`)
  if (actions.length > 0) parts.push(`Actions to experience\n${actions.join('\n')}`)

  const refs = workspace.context_refs.map((ref) => `- ${ref.label}: ${ref.uri}`)
  if (refs.length > 0) parts.push(`Context references\n${refs.join('\n')}`)

  const files = workspace.request_attachments.map(
    (attachment) => `- ${attachment.file_name} (${(attachment.byte_size / 1024).toFixed(1)} KiB)`,
  )
  if (files.length > 0) parts.push(`Files\n${files.join('\n')}`)

  return parts.join('\n\n')
}
