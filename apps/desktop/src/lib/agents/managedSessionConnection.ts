import type { ApplicationTransport } from '$lib/application/applicationTransport'
import type { ManagedSessionSnapshot } from '$lib/generated/feedback'

/** A retry must acknowledge unresolved cleanup or a missing previous context. */
export function automaticConnectionIssue(snapshot: ManagedSessionSnapshot): string {
  const { runtime, session, recovery } = snapshot
  if (runtime.connection === 'failed') return runtime.last_error || 'Could not connect to the agent.'
  const management = session.management
  if (runtime.instance_id || (recovery && (recovery.session_id !== session.session_id
    || recovery.status === 'unclosed' || recovery.active_turn_id))
    || (management.kind === 'managed' && !management.remote_session_id
      && (snapshot.activities.length > 0 || (recovery && recovery.status !== 'never_started')))) {
    return 'This session needs an explicit connection retry before continuing.'
  }
  return ''
}

// A fast tab remount may still observe "stopped" while its preceding mount's
// start command is in flight. Share that admission, never the view lifecycle.
const pendingStarts = new WeakMap<ApplicationTransport, Map<string, Promise<ManagedSessionSnapshot>>>()
export function startManagedSessionOnce(transport: ApplicationTransport, sessionId: string): Promise<ManagedSessionSnapshot> {
  let sessions = pendingStarts.get(transport)
  if (!sessions) { sessions = new Map(); pendingStarts.set(transport, sessions) }
  const existing = sessions.get(sessionId)
  if (existing) return existing
  const pending = transport.call('startManagedSession', { session_id: sessionId })
  sessions.set(sessionId, pending)
  const release = () => { if (sessions.get(sessionId) === pending) sessions.delete(sessionId) }
  void pending.then(release, release)
  return pending
}
