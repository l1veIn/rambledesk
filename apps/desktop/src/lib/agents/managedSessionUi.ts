import type { AgentConfig, ManagedSessionSnapshot, SessionActivity as GeneratedSessionActivity, SessionInteraction as GeneratedSessionInteraction } from '$lib/generated/feedback'

export type ManagedSessionViewSnapshot = Readonly<Pick<ManagedSessionSnapshot, 'session' | 'runtime' | 'deleting'>>

export type SessionActivity = Readonly<Pick<GeneratedSessionActivity,
  'id' | 'session_id' | 'kind' | 'text' | 'tool_call_id' | 'created_at'>
  & Partial<Pick<GeneratedSessionActivity, 'sequence' | 'turn_id' | 'content'>>>

export type SessionInteraction = Readonly<GeneratedSessionInteraction>

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

export function interactionsForSession(sessionId: string, interactions: readonly SessionInteraction[]): SessionInteraction[] {
  const seen = new Set<string>()
  return interactions.filter((interaction) => {
    if (interaction.session_id !== sessionId || seen.has(interaction.request_id)) return false
    seen.add(interaction.request_id)
    return true
  })
}

export function managedSessionActions(snapshot: ManagedSessionViewSnapshot, pendingInteractions: number) {
  const { connection, activity } = snapshot.runtime
  const managed = snapshot.session.management.kind === 'managed' && !snapshot.deleting
  return {
    canPrompt: managed && connection === 'connected' && activity === 'idle' && pendingInteractions === 0,
    canStart: managed && connection !== 'connected' && connection !== 'connecting',
    canCancel: managed && connection === 'connected' && activity !== 'idle',
  }
}

export function managedSessionComposerState(
  snapshot: ManagedSessionViewSnapshot,
  pendingInteractions: number,
  pending: { busy: boolean; lifecycle: boolean; prompt: boolean },
) {
  const actions = managedSessionActions(snapshot, pendingInteractions)
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
  readonly #awaitingAcknowledgement = new Map<string, PromptSubmission>()
  readonly #failedMessages = new Map<string, { id: string; text: string }>()
  readonly #restoredFailures = new Map<string, string>()

  read(sessionId: string): string { return this.#drafts.get(sessionId) ?? '' }
  write(sessionId: string, text: string): void {
    if (this.#deletedSessions.has(sessionId) || this.read(sessionId) === text) return
    this.#drafts.set(sessionId, text)
    this.#revisions.set(sessionId, (this.#revisions.get(sessionId) ?? 0) + 1)
  }
  remove(sessionId: string): void {
    this.write(sessionId, '')
    this.#drafts.delete(sessionId)
  }
  forgetSession(sessionId: string): void {
    this.remove(sessionId)
    this.#awaitingAcknowledgement.delete(sessionId)
    this.#failedMessages.delete(sessionId)
    this.#restoredFailures.delete(sessionId)
    this.#deletedSessions.add(sessionId)
  }

  awaitAcknowledgement(submission: PromptSubmission): void { this.#awaitingAcknowledgement.set(submission.sessionId, submission) }
  resolveAcknowledgement(sessionId: string, accepted: boolean): boolean {
    const submission = this.#awaitingAcknowledgement.get(sessionId)
    this.#awaitingAcknowledgement.delete(sessionId)
    return Boolean(submission && !accepted && this.restoreSubmission(submission))
  }

  failedMessage(sessionId: string, latestUser: SessionActivity | undefined, reportedFailure: boolean): { id: string; text: string } | undefined {
    const existing = this.#failedMessages.get(sessionId)
    if (latestUser && existing && latestUser.id !== existing.id) this.#failedMessages.delete(sessionId)
    if (reportedFailure && latestUser && latestUser.id !== this.#restoredFailures.get(sessionId)) {
      this.#failedMessages.set(sessionId, { id: latestUser.id, text: latestUser.text })
    }
    return this.#failedMessages.get(sessionId)
  }

  restoreFailedMessage(sessionId: string): boolean {
    const failed = this.#failedMessages.get(sessionId)
    if (!failed || this.read(sessionId).trim()) return false
    this.write(sessionId, failed.text)
    this.#restoredFailures.set(sessionId, failed.id)
    this.#failedMessages.delete(sessionId)
    return true
  }

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
}

// Keep unsent drafts while a session view unmounts during tab navigation.
export const sessionPromptDrafts = new SessionPromptDrafts()
