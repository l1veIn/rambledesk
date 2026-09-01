import type { ApplicationEvent, ApplicationResourceKey } from '../generated/feedback'
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

export type SnapshotUnstableError = Readonly<{
  code: 'SNAPSHOT_UNSTABLE'
  message: string
  retryable: true
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

export function isSnapshotUnstableError(value: unknown): value is SnapshotUnstableError {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Partial<SnapshotUnstableError>
  return (
    candidate.code === 'SNAPSHOT_UNSTABLE' &&
    typeof candidate.message === 'string' &&
    candidate.retryable === true
  )
}

export function applicationEventRevision(event: ApplicationEvent): bigint {
  return BigInt(event.revision)
}

export function applicationResourceKeyIdentity(resource: ApplicationResourceKey): string {
  switch (resource.kind) {
    case 'all':
    case 'navigation':
      return resource.kind
    case 'host_session_resources':
      return `${resource.kind}:${resource.host_id}:${resource.host_session_id}`
    case 'feedback_workspace':
    case 'published_feedback':
      return `${resource.kind}:${resource.request_id}`
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function isDecimalRevision(value: unknown): value is string {
  return typeof value === 'string' && /^\d+$/u.test(value)
}

function isApplicationResourceKey(value: unknown): boolean {
  if (!isRecord(value) || typeof value.kind !== 'string') return false
  switch (value.kind) {
    case 'all':
    case 'navigation':
      return true
    case 'host_session_resources':
      return isNonEmptyString(value.host_id) && isNonEmptyString(value.host_session_id)
    case 'feedback_workspace':
    case 'published_feedback':
      return isNonEmptyString(value.request_id)
    default:
      return false
  }
}

export function parseApplicationEvent(value: unknown): ApplicationEvent {
  if (
    !isRecord(value) ||
    !isNonEmptyString(value.runtime_generation) ||
    !isDecimalRevision(value.revision)
  ) {
    throw new Error('Application event metadata is invalid.')
  }
  if (value.type === 'ready') return value as unknown as ApplicationEvent
  if (
    value.type === 'invalidate' &&
    Array.isArray(value.resources) &&
    value.resources.length > 0 &&
    value.resources.every(isApplicationResourceKey)
  ) {
    return value as unknown as ApplicationEvent
  }
  throw new Error('Application event payload is invalid.')
}
