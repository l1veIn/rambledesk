import type { ApplicationEvent } from '../generated/feedback'
import { defineApplicationStream } from './applicationTransport'

export const APPLICATION_EVENTS_STREAM = defineApplicationStream<ApplicationEvent>(
  'rambledesk://application-events',
)

export const RUNTIME_GENERATION_HEADER = 'X-RambleDesk-Runtime-Generation'
export const REVISION_HEADER = 'X-RambleDesk-Revision'
export const APPLICATION_EVENT_PROTOCOL = 'rambledesk-events'
export const APPLICATION_EVENT_CREDENTIAL_PROTOCOL_PREFIX = 'rambledesk-session.'

export type RuntimeGenerationStaleError = Readonly<{
  code: 'RUNTIME_GENERATION_STALE'
  message: string
  retryable: false
}>

export function isRuntimeGenerationStaleError(
  value: unknown,
): value is RuntimeGenerationStaleError {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Partial<RuntimeGenerationStaleError>
  return (
    candidate.code === 'RUNTIME_GENERATION_STALE' &&
    typeof candidate.message === 'string' &&
    candidate.retryable === false
  )
}

export function applicationEventRevision(event: ApplicationEvent): bigint {
  return BigInt(event.revision)
}
