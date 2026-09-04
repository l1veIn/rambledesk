import type { AgentConfig, FeedbackRequestSummary, ManagedSessionSnapshot, SessionActivity as GeneratedSessionActivity, SessionPermission as GeneratedSessionPermission, SessionRecord } from '$lib/generated/feedback'

export type ManagedSessionViewSnapshot = Readonly<Pick<ManagedSessionSnapshot, 'session' | 'runtime' | 'deleting'>>

export type SessionActivity = Readonly<Pick<GeneratedSessionActivity,
  'id' | 'session_id' | 'kind' | 'text' | 'tool_call_id' | 'created_at'>
  & Partial<Pick<GeneratedSessionActivity, 'sequence' | 'turn_id' | 'content'>>>

export type SessionPermission = Readonly<GeneratedSessionPermission>

export function activitiesForSession(sessionId: string, activities: readonly SessionActivity[]): SessionActivity[] {
  // A newer snapshot may contain an updated tool call with the same id.
  const entries = new Map<string, SessionActivity>()
  for (const activity of activities) {
    if (activity.session_id === sessionId) entries.set(activity.id, activity)
  }
  const result = [...entries.values()]
  // Structured snapshots carry the durable sequence, including updated tools.
  // Legacy text-only view fixtures retain their original encounter order.
  return result.every((entry) => typeof entry.sequence === 'number')
    ? result.sort((left, right) => left.sequence! - right.sequence!) : result
}

export function permissionsForSession(sessionId: string, permissions: readonly SessionPermission[]): SessionPermission[] {
  const seen = new Set<string>()
  return permissions.filter((permission) => {
    if (permission.session_id !== sessionId || seen.has(permission.request_id)) return false
    seen.add(permission.request_id)
    return true
  })
}

export function feedbackForSession(session: SessionRecord, requests: readonly FeedbackRequestSummary[]): FeedbackRequestSummary[] {
  return requests.filter((request) => request.host_id === session.host_id && request.host_session_id === session.host_session_id)
}

export function managedSessionActions(snapshot: ManagedSessionViewSnapshot, pendingPermissions: number) {
  const { connection, activity } = snapshot.runtime
  const managed = snapshot.session.management.kind === 'managed' && !snapshot.deleting
  return {
    canPrompt: managed && connection === 'connected' && activity === 'idle' && pendingPermissions === 0,
    canStart: managed && connection !== 'connected' && connection !== 'connecting',
    canCancel: managed && connection === 'connected' && activity !== 'idle',
    canStop: managed && (connection === 'connected' || connection === 'connecting'),
    startLabel: managed && snapshot.session.management.kind === 'managed' && snapshot.session.management.remote_session_id
      ? 'Resume session' : 'Start agent',
  }
}

export function managedSessionComposerState(
  snapshot: ManagedSessionViewSnapshot,
  pendingPermissions: number,
  pending: { busy: boolean; lifecycle: boolean; prompt: boolean },
) {
  const actions = managedSessionActions(snapshot, pendingPermissions)
  return {
    disabled: pending.busy || snapshot.deleting,
    busy: snapshot.runtime.activity !== 'idle' || pending.prompt,
    sendDisabled: pending.busy || pending.lifecycle || !actions.canPrompt,
    canCancel: !pending.busy && !pending.lifecycle && actions.canCancel,
  }
}

export function sessionConfigurationChanged(snapshot: ManagedSessionViewSnapshot, config: AgentConfig | null): boolean {
  const management = snapshot.session.management
  return management.kind === 'managed' && config?.id === management.agent_config_id
    && snapshot.runtime.config_updated_at !== null
    && snapshot.runtime.config_updated_at !== config.updated_at
}

export function activityLabel(kind: SessionActivity['kind']): string {
  switch (kind) {
    case 'user_message': return 'You'
    case 'agent_message': return 'Agent'
    case 'agent_thought': return 'Agent reasoning'
    case 'tool_call': return 'Tool activity'
    case 'status': return 'Session status'
    case 'error': return 'Agent error'
  }
}

/** View state only; no credentials, runtime ownership, or transport side effects. */
export type PromptSubmission = Readonly<{ sessionId: string; text: string; clearedRevision: number }>

export class SessionPromptDrafts {
  readonly #drafts = new Map<string, string>()
  readonly #revisions = new Map<string, number>()
  readonly #deletedSessions = new Set<string>()

  read(sessionId: string): string { return this.#drafts.get(sessionId) ?? '' }
  write(sessionId: string, text: string): void {
    if (this.#deletedSessions.has(sessionId) || this.read(sessionId) === text) return
    this.#drafts.set(sessionId, text)
    this.#revisions.set(sessionId, (this.#revisions.get(sessionId) ?? 0) + 1)
  }
  remove(sessionId: string): void { this.write(sessionId, ''); this.#drafts.delete(sessionId) }
  forgetSession(sessionId: string): void { this.remove(sessionId); this.#deletedSessions.add(sessionId) }

  beginSubmission(sessionId: string, text: string): PromptSubmission {
    this.write(sessionId, text)
    this.remove(sessionId)
    return { sessionId, text, clearedRevision: this.#revisions.get(sessionId) ?? 0 }
  }

  restoreSubmission(submission: PromptSubmission): boolean {
    const { sessionId, text, clearedRevision } = submission
    if (this.#deletedSessions.has(sessionId) || this.read(sessionId) !== '' || (this.#revisions.get(sessionId) ?? 0) !== clearedRevision) return false
    this.write(sessionId, text)
    return true
  }

  accepted(sessionId: string, submittedText: string): void {
    // Do not clear a newer draft written while the previous prompt was sending.
    if (this.read(sessionId) === submittedText) this.remove(sessionId)
  }
}

// Keep unsent drafts while a session view unmounts during tab navigation.
export const sessionPromptDrafts = new SessionPromptDrafts()
