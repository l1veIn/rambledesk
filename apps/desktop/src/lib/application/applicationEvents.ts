import type { ApplicationEvent } from '../generated/feedback'
import { defineApplicationStream } from './applicationTransport'

export const APPLICATION_EVENTS_STREAM = defineApplicationStream<ApplicationEvent>(
  'rambledesk://application-events',
)

export function applicationEventRevision(event: ApplicationEvent): bigint {
  return BigInt(event.revision)
}
