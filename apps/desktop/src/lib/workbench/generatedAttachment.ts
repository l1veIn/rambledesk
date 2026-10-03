import type { ApplicationTransport } from '../application/applicationTransport'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import type { AttachmentView, FeedbackWorkspaceView } from '../feedback'

export type GeneratedAttachmentInput = { fileName: string; contents: ArrayBuffer }

/** Generated workbench artifacts share the attachment CAS queue without inserting text. */
export function createGeneratedAttachmentPersistence(context: {
  transport: ApplicationTransport
  getWorkspace: () => FeedbackWorkspaceView | null
  saveDraftNow: () => Promise<boolean>
  waitForDocument: () => Promise<void>
  queue: (requestId: string, work: (active: () => boolean) => Promise<boolean>) => Promise<boolean>
  showMutation: (workspace: FeedbackWorkspaceView) => Promise<void>
  reconcileFailure: (requestId: string, active: () => boolean) => Promise<void>
}) {
  return async (requestId: string, input: GeneratedAttachmentInput): Promise<AttachmentView> => {
    if (!input.contents.byteLength || input.contents.byteLength > 20 * 1024 * 1024) {
      throw new Error('Generated attachment must be between 1 byte and 20 MiB.')
    }
    let created: AttachmentView | undefined
    let failure: unknown
    const ok = await context.queue(requestId, async (active) => {
      let uploading = false
      try {
        if (context.getWorkspace()?.request.request_id !== requestId || !active()) return false
        if (!(await context.saveDraftNow())) throw new Error('Save the draft before generating its attachment.')
        await context.waitForDocument()
        if (context.getWorkspace()?.request.request_id !== requestId || !active()) return false
        const previous = await readApplicationSnapshot(context.transport, 'getFeedbackWorkspace', { request_id: requestId })
        if (!previous || ['completed', 'cancelled'].includes(previous.request.status) || !active()) return false
        const previousIds = new Set(previous.attachments.map((item) => item.attachment_id))
        uploading = true
        const next = await context.transport.call('addFeedbackAttachment', {
          request_id: requestId, file_name: input.fileName, contents: input.contents,
          expected_revision: previous.draft.saved_revision,
        })
        uploading = false
        if (context.getWorkspace()?.request.request_id !== requestId || !active()) return false
        created = next.attachments.find((item) => !previousIds.has(item.attachment_id))
        if (!created) throw new Error('The generated attachment was not returned by the server.')
        await context.showMutation(next)
        return active()
      } catch (cause) {
        failure = cause
        if (uploading && active()) await context.reconcileFailure(requestId, active)
        throw cause
      }
    })
    if (!ok || !created) throw failure ?? new Error('The request changed before its generated attachment was saved.')
    return created
  }
}
