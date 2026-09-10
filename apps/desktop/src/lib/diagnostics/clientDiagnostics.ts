import schema from './clientDiagnosticSchema.json'

export type DiagnosticDetails = Readonly<Record<string, string | number | boolean>>
export type DiagnosticOutcome = 'started' | 'ok' | 'failed' | 'cancelled' | 'skipped' | 'blocked'
export type ClientDiagnosticEvent = Readonly<{
  activity: string
  outcome: DiagnosticOutcome
  operationId: string
  durationMs?: number
  details?: DiagnosticDetails
}>
type DiagnosticInput = Omit<ClientDiagnosticEvent, 'operationId'> & { operationId?: string }
type DiagnosticSink = (event: ClientDiagnosticEvent) => void | Promise<void>
const MAX_DURATION_MS = 7 * 24 * 60 * 60 * 1000
const MAX_PENDING_EVENTS = 128
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu
const enumDetails = schema.details as Record<string, readonly string[]>

export function diagnosticAgentId(value: string | undefined): string {
  return value && schema.details.agent.includes(value) ? value : 'custom'
}

/** Runtime allowlist is also loaded by the native receiver. No arbitrary strings cross IPC. */
export function sanitizeDiagnosticDetails(details: DiagnosticDetails = {}): DiagnosticDetails {
  const safe: Record<string, string | number | boolean> = {}
  for (const [key, value] of Object.entries(details)) {
    if (typeof value === 'string' && enumDetails[key]?.includes(value)) safe[key] = value
    else if (typeof value === 'boolean' && schema.flags.includes(key)) safe[key] = value
    else if (typeof value === 'number' && schema.counts.includes(key) && Number.isSafeInteger(value) && value >= 0) safe[key] = value
  }
  return safe
}

/** Failures, slow sinks and duplicate finishes must never affect the user's operation. */
export function createClientDiagnosticRecorder(
  sink: DiagnosticSink,
  now: () => number = () => performance.now(),
  newId: () => string = () => crypto.randomUUID(),
) {
  let pending = 0
  let dropped = 0
  let enabled = true
  let generation = 0
  const writes = new Set<Promise<void>>()
  const record = (input: DiagnosticInput) => {
    try {
      if (!enabled) return false
      if (pending >= MAX_PENDING_EVENTS) { dropped += 1; return false }
      if (!schema.activities.includes(input.activity) || !schema.outcomes.includes(input.outcome)) return false
      const operationId = input.operationId ?? newId()
      if (!UUID.test(operationId)) return false
      const durationMs = input.durationMs === undefined ? undefined : Math.min(MAX_DURATION_MS, Math.max(0, Math.round(input.durationMs)))
      if (durationMs !== undefined && !Number.isFinite(durationMs)) return false
      const event = { activity: input.activity, outcome: input.outcome, operationId, durationMs, details: sanitizeDiagnosticDetails(input.details) }
      const lossCount = event.activity === 'diagnostic_backpressure' && typeof event.details.dropped_count === 'number' ? event.details.dropped_count : 1
      const writeGeneration = generation
      pending += 1
      try {
        const write = Promise.resolve(sink(event)).then(() => undefined, () => {
          if (enabled && generation === writeGeneration) dropped += lossCount
        }).finally(() => { pending -= 1; writes.delete(write) })
        writes.add(write)
        return true
      }
      catch { pending -= 1; dropped += lossCount; return false }
    } catch { return false }
  }
  const start = (activity: string, details: DiagnosticDetails = {}) => {
    if (!enabled) return () => {}
    const startGeneration = generation
    let finished = false
    let started = 0
    let operationId: string | undefined
    try { started = now(); operationId = newId() } catch { /* Unsupported runtime: no events. */ }
    let initial: DiagnosticDetails = {}
    try { initial = sanitizeDiagnosticDetails(details) } catch { operationId = undefined }
    if (operationId && !record({ activity, outcome: 'started', operationId, details: initial })) operationId = undefined
    return (outcome: Exclude<DiagnosticOutcome, 'started'>, finalDetails: DiagnosticDetails = {}) => {
      if (finished || !operationId || !enabled || startGeneration !== generation) return
      finished = true
      try { record({ activity, outcome, operationId, durationMs: now() - started, details: { ...initial, ...sanitizeDiagnosticDetails(finalDetails) } }) }
      catch { /* A clock or sink failure cannot change the application result. */ }
    }
  }
  const flush = async (timeoutMs = 1000) => {
    const started = Date.now()
    const wait = async (remaining: number) => {
      let timer: ReturnType<typeof setTimeout> | undefined
      try {
        await Promise.race([Promise.all([...writes]), new Promise<void>(resolve => { timer = setTimeout(resolve, remaining) })])
      } finally { if (timer !== undefined) clearTimeout(timer) }
    }
    if (writes.size) await wait(timeoutMs)
    if (enabled && dropped > 0 && pending < MAX_PENDING_EVENTS) {
      const count = dropped
      dropped = 0
      record({ activity: 'diagnostic_backpressure', outcome: 'skipped', details: { dropped_count: count } })
      const remaining = timeoutMs - (Date.now() - started)
      if (remaining > 0) await wait(remaining)
    }
  }
  const setEnabled = (next: boolean) => {
    if (enabled === next) return
    enabled = next
    generation += 1
    dropped = 0
  }
  return { record, start, flush, setEnabled }
}

let recorder: ReturnType<typeof createClientDiagnosticRecorder> | undefined
let recordingEnabled = true
/** The native entry writes to IPC; development previews may inject an in-memory sink. */
export function configureClientDiagnostics(sink: DiagnosticSink): () => void {
  const configured = createClientDiagnosticRecorder(sink)
  configured.setEnabled(recordingEnabled)
  recorder = configured
  return () => { if (recorder === configured) recorder = undefined }
}
/** Native settings remain authoritative; this also stops local event production. */
export function setClientDiagnosticsEnabled(enabled: boolean): void {
  recordingEnabled = enabled
  recorder?.setEnabled(enabled)
}
export function recordClientDiagnostic(event: DiagnosticInput): void { recorder?.record(event) }
/** Export waits briefly for already queued metadata; an unavailable sink cannot stall export. */
export async function flushClientDiagnostics(): Promise<void> { await recorder?.flush() }
export function startClientDiagnostic(activity: string, details: DiagnosticDetails = {}) {
  return recorder?.start(activity, details) ?? (() => {})
}

/** Classify structured error codes only; never serialize messages, stacks or thrown objects. */
export function diagnosticErrorCategory(cause: unknown): string {
  if (!cause || typeof cause !== 'object') return 'unknown'
  let code: unknown
  try { code = 'code' in cause ? cause.code : cause instanceof Error ? cause.name : undefined } catch { return 'unknown' }
  if (typeof code !== 'string') return 'unknown'
  const categories: Record<string, string> = {
    TIMEOUT: 'timeout', REQUEST_TIMEOUT: 'timeout', TimeoutError: 'timeout',
    CANCELLED: 'cancelled', CANCELED: 'cancelled', AbortError: 'cancelled',
    PERMISSION_DENIED: 'permission_denied', UNAUTHORIZED: 'permission_denied',
    UNAVAILABLE: 'unavailable', CAPABILITY_UNAVAILABLE: 'unavailable', CapabilityUnavailableError: 'unavailable',
    INVALID_ARGUMENT: 'invalid_input', INVALID_INPUT: 'invalid_input',
    CONFLICT: 'conflict', NOT_FOUND: 'not_found',
  }
  return categories[code] ?? 'unknown'
}
