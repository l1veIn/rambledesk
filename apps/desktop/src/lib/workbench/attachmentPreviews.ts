import { tick } from 'svelte'

import { isImageMediaType } from '../attachmentMarkdown'
import type { ApplicationTransport } from '../application/applicationTransport'
import type { FeedbackWorkspaceView } from '../feedback'

/** Owns Editor preview URLs; a discarded read never owns URLs already on screen. */
export function createAttachmentPreviews(context: {
  transport: ApplicationTransport
  getRequestId: () => string | undefined
  publish: (previews: Record<string, string>) => void
}) {
  let generation = 0
  let previews: Record<string, string> = {}

  async function publish(next: Record<string, string>) {
    const previous = previews
    previews = next
    context.publish(next)
    // Let Svelte remove the old src values before releasing their last owner.
    await tick()
    const retained = new Set(Object.values(previews))
    for (const url of Object.values(previous)) {
      if (!retained.has(url)) URL.revokeObjectURL(url)
    }
  }

  async function refresh(workspace: FeedbackWorkspaceView) {
    const currentGeneration = ++generation
    const requestId = workspace.request.request_id
    const stillCurrent = () => currentGeneration === generation && context.getRequestId() === requestId
    const next: Record<string, string> = {}
    const created: string[] = []
    try {
      for (const attachment of workspace.attachments) {
        if (!isImageMediaType(attachment.media_type)) continue
        const id = attachment.attachment_id
        if (previews[id]) {
          next[id] = previews[id]
          continue
        }
        try {
          const bytes = await context.transport.call('readFeedbackAttachment', {
            request_id: requestId,
            attachment_id: id,
          })
          if (!stillCurrent()) return
          const url = URL.createObjectURL(new Blob([bytes], { type: attachment.media_type }))
          created.push(url)
          next[id] = url
        } catch {
          // An unavailable preview does not block editing or submission.
        }
      }
      if (!stillCurrent()) return
      // Ownership passes to the displayed projection, including any newly loaded URL.
      created.length = 0
      await publish(next)
    } finally {
      // Reused URLs belong to the displayed projection, never to this abandoned read.
      for (const url of created) URL.revokeObjectURL(url)
    }
  }

  function release() {
    generation += 1
    void publish({})
  }

  return { refresh, release }
}
