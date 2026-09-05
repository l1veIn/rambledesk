// Process/answer splitting and fold lifecycle adapted from Codeg at 3ebdfed.
// Sources: completed-turn-content.tsx, message-list-view.tsx, ai-elements-adapter.ts.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk uses durable turn_id and persisted lifecycle markers.
import type { Locale } from '$lib/preferences'
import type { SessionActivity } from '../managedSessionUi'
import { activityHasQuote, activityQuoteText } from './activity-presentation'

export type AgentTurn = Readonly<{
  id: string
  activities: readonly SessionActivity[]
  process: readonly SessionActivity[]
  answer: readonly SessionActivity[]
  notices: readonly SessionActivity[]
  active: boolean
  outcome: 'working' | 'finished' | 'cancelled' | 'interrupted' | 'stopped'
  completedAt: string | null
  durationMs: number | null
  partialStart: boolean
  foldable: boolean
}>
export type TimelineItem = Readonly<{ type: 'activity'; id: string; activity: SessionActivity }>
  | Readonly<{ type: 'turn'; id: string; turn: AgentTurn }>

export function isTurnStart(activity: SessionActivity): boolean {
  return activity.kind === 'status' && activity.text === 'Turn started'
}
export function isTurnFinish(activity: SessionActivity): boolean {
  return activity.kind === 'status' && activity.text.startsWith('Turn finished: ')
}
function validTimestamp(value: string | undefined): string | null {
  return value && Number.isFinite(Date.parse(value)) ? value : null
}

/** Commentary before/between tools is process; all trailing messages stay visible. */
export function splitTurnContent(activities: readonly SessionActivity[]) {
  let boundary = activities.length
  while (boundary > 0 && activities[boundary - 1].kind === 'agent_message') boundary -= 1
  return { process: activities.slice(0, boundary), answer: activities.slice(boundary) }
}

function presentTurn(id: string, activities: readonly SessionActivity[], activeId: string | null, partialStart: boolean): AgentTurn {
  const start = activities.find(isTurnStart)
  const finish = activities.findLast(isTurnFinish)
  const notices = activities.filter((row) => row.kind === 'error')
  const interrupted = notices.some((row) => row.text === 'Turn interrupted before completion.')
  const active = id === activeId && !finish && !interrupted
  const body = activities.filter((row) => row.kind !== 'error' && !isTurnStart(row) && !isTurnFinish(row))
  const split = splitTurnContent(body)
  const hasAnswer = split.answer.some(activityHasQuote)
  const completedAt = validTimestamp(finish?.created_at)
  const startedAt = validTimestamp(start?.created_at)
  const duration = completedAt && startedAt ? Date.parse(completedAt) - Date.parse(startedAt) : null
  const reason = finish?.text.slice('Turn finished: '.length).toLowerCase()
  return {
    id, activities, ...split, notices, active,
    outcome: active ? 'working' : interrupted ? 'interrupted' : reason?.includes('cancelled') ? 'cancelled'
      : reason === 'endturn' || reason === 'end_turn' ? 'finished' : 'stopped',
    completedAt, durationMs: duration !== null && duration >= 0 ? duration : null,
    partialStart: partialStart && !start,
    foldable: split.process.length > 0 && (active || hasAnswer),
  }
}

export function groupTimeline(activities: readonly SessionActivity[], runActive: boolean): TimelineItem[] {
  const activeId = runActive ? activities.findLast((row) => row.turn_id)?.turn_id ?? null : null
  const result: TimelineItem[] = []
  let pending: SessionActivity[] = []
  let pendingId: string | null = null
  const flush = () => {
    if (!pendingId || pending.length === 0) return
    const partialStart = result.length === 0 && (pending[0].sequence ?? 1) > 1
    result.push({ type: 'turn', id: `turn:${pendingId}`, turn: presentTurn(pendingId, pending, activeId, partialStart) })
    pending = []
    pendingId = null
  }
  for (const activity of activities) {
    if (!activity.turn_id || activity.kind === 'user_message') {
      flush()
      result.push({ type: 'activity', id: activity.id, activity })
    } else {
      if (pendingId !== activity.turn_id) flush()
      pendingId = activity.turn_id
      pending.push(activity)
    }
  }
  flush()
  return result
}

export function turnCopyText(turn: AgentTurn): string {
  return turn.answer.filter(activityHasQuote).map(activityQuoteText).join('\n\n')
}

export function turnDurationLabel(durationMs: number, locale: Locale): string {
  const seconds = Math.max(1, Math.round(durationMs / 1000))
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor(seconds % 3600 / 60)
  const remainder = seconds % 60
  const labels = locale === 'zh-CN' ? [' 小时', ' 分钟', ' 秒'] : ['h', 'm', 's']
  return [hours ? `${hours}${labels[0]}` : '', minutes ? `${minutes}${labels[1]}` : '', remainder || (!hours && !minutes) ? `${remainder || 1}${labels[2]}` : ''].filter(Boolean).join(' ')
}

/** Completion, paging and component remounts keep reader choices. */
export class TurnFoldState {
  #initialized = false
  #latestUserSequence = -1
  #latestUserId: string | null = null
  #observedSequence = -1
  #overrides = new Map<string, boolean>()
  observe(activities: readonly SessionActivity[], _items: readonly TimelineItem[]): void {
    const user = activities.findLast((row) => row.kind === 'user_message')
    const sequence = user?.sequence ?? 0
    if (user && user.id !== this.#latestUserId && (sequence > this.#latestUserSequence || this.#latestUserSequence < 0)) {
      // A user row discovered by paging backwards is not a newly sent prompt.
      const newSend = typeof user.sequence === 'number' ? user.sequence > this.#observedSequence : this.#latestUserId !== null
      if (this.#initialized && newSend) {
        this.#overrides.clear()
      }
      this.#latestUserId = user.id
      this.#latestUserSequence = sequence
    }
    for (const row of activities) this.#observedSequence = Math.max(this.#observedSequence, row.sequence ?? -1)
    this.#initialized = true
  }
  open(turn: AgentTurn): boolean {
    return !turn.foldable || (this.#overrides.get(turn.id) ?? turn.active)
  }
  toggle(id: string, open: boolean): void { this.#overrides.set(id, open) }
}

const sessionFolds = new Map<string, TurnFoldState>()
export function sessionTurnFolds(sessionId: string): TurnFoldState {
  let state = sessionFolds.get(sessionId)
  if (!state) state = new TurnFoldState()
  // Keep revisited tabs' state while bounding abandoned/deleted session metadata.
  sessionFolds.delete(sessionId)
  sessionFolds.set(sessionId, state)
  if (sessionFolds.size > 64) sessionFolds.delete(sessionFolds.keys().next().value!)
  return state
}
