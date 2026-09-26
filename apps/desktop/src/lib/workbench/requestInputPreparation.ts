import type { FeedbackPreparation } from '../speech/rambleSessionControllerHandle'
import type { AttachmentPreparation } from './attachmentController'

type RequestInputPreparationContext = {
  prepareSpeech: (requestId: string) => Promise<FeedbackPreparation>
  prepareAttachments: (requestId: string) => Promise<AttachmentPreparation>
  tr: (source: string) => string
}

/** The request owns input completion; hiding an input component is not a receipt. */
export function createRequestInputPreparation(context: RequestInputPreparationContext) {
  const flights = new Map<string, Promise<FeedbackPreparation>>()

  async function prepare(requestId: string): Promise<FeedbackPreparation> {
    // The second pass joins clipboard/speech received while attachments drained.
    // The caller must still recheck its request and busy state before freezing.
    for (let pass = 0; pass < 2; pass += 1) {
      const speech = await context.prepareSpeech(requestId)
      if (speech.kind !== 'ready') return speech
      const attachments = await context.prepareAttachments(requestId)
      if (attachments.kind === 'pending-capture') {
        return {
          kind: 'failed',
          message: context.tr('Finish or cancel the pending screen capture before ending this request.'),
        }
      }
      if (attachments.kind !== 'ready') return attachments
    }
    return { kind: 'ready' }
  }

  function prepareFeedback(requestId: string): Promise<FeedbackPreparation> {
    const active = flights.get(requestId)
    if (active) return active
    const flight = Promise.resolve().then(() => prepare(requestId)).finally(() => {
      if (flights.get(requestId) === flight) flights.delete(requestId)
    })
    flights.set(requestId, flight)
    return flight
  }

  return { prepareFeedback }
}
