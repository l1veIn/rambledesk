import { describe, expect, it } from 'vitest'
import type { SessionActivity } from '../managedSessionUi'
import { groupTimeline, TurnFoldState, turnCopyText, turnDurationLabel, type AgentTurn } from './turn-presentation'

function row(id: string, kind: SessionActivity['kind'], text: string, sequence: number, turn = 'one', createdAt = '2026-09-05T10:00:00Z'): SessionActivity {
  return { id, kind, text, sequence, turn_id: turn, session_id: 'session', created_at: createdAt, tool_call_id: null }
}
function fixture(): SessionActivity[] {
  return [row('u', 'user_message', 'Task', 1), row('s', 'status', 'Turn started', 2),
    row('note', 'agent_message', 'Checking first', 3), row('t', 'agent_thought', 'Reasoning', 4),
    row('tool', 'tool_call', 'Read file', 5), row('a', 'agent_message', 'First answer paragraph', 6),
    row('b', 'agent_message', 'Second answer paragraph', 7), row('f', 'status', 'Turn finished: EndTurn', 8, 'one', '2026-09-05T10:00:29Z')]
}
function lastTurn(rows: readonly SessionActivity[], running = false): AgentTurn {
  const item = groupTimeline(rows, running).findLast((item) => item.type === 'turn')
  if (item?.type !== 'turn') throw new Error('missing turn')
  return item.turn
}

describe('durable turn presentation', () => {
  it('groups by backend turn identity and preserves every trailing answer paragraph', () => {
    const rows = fixture()
    const items = groupTimeline(rows, false)
    expect(items.map((item) => item.type)).toEqual(['activity', 'turn'])
    const turn = lastTurn(rows)
    expect(turn.process.map((row) => row.id)).toEqual(['note', 't', 'tool'])
    expect(turn.answer.map((row) => row.id)).toEqual(['a', 'b'])
    expect(turnCopyText(turn)).toBe('First answer paragraph\n\nSecond answer paragraph')
    expect(turn.durationMs).toBe(29_000)
    expect(turn.completedAt).toBe('2026-09-05T10:00:29Z')
    expect(turn.outcome).toBe('finished')
  })
  it('keeps text-only answers intact without inventing time from chunk creation', () => {
    const rows = fixture().filter((row) => ['a', 'b'].includes(row.id))
    const turn = lastTurn(rows)
    expect(turn.answer).toEqual(rows)
    expect(turn.foldable).toBe(false)
    expect(turn.completedAt).toBeNull()
    expect(turn.durationMs).toBeNull()
    expect(turn.partialStart).toBe(true)
  })
  it('does not hide an unfinished or whitespace-only answer behind the work fold', () => {
    const rows = fixture().filter((row) => !['a', 'b', 'f'].includes(row.id))
    rows.push(row('blank', 'agent_message', ' \n ', 6))
    rows.push(row('err', 'error', 'Turn interrupted before completion.', 7))
    const turn = lastTurn(rows)
    expect(turn.foldable).toBe(false)
    expect(turn.outcome).toBe('interrupted')
    expect(turn.notices.map((row) => row.id)).toEqual(['err'])
    expect(turn.durationMs).toBeNull()
  })
  it('shows cancelled and error outcomes outside process content', () => {
    const rows = fixture()
    rows[7] = { ...rows[7], text: 'Turn finished: Cancelled' }
    rows.splice(7, 0, row('err', 'error', 'Tool failed', 7))
    const turn = lastTurn(rows)
    expect(turn.outcome).toBe('cancelled')
    expect(turn.notices[0].text).toBe('Tool failed')
    expect(turn.answer.map((row) => row.id)).toEqual(['a', 'b'])
  })
  it('does not treat an older ended turn as active before a new turn arrives', () => {
    expect(lastTurn(fixture(), true).active).toBe(false)
  })
  it('fills a paged turn without changing identity or fabricating an incomplete duration', () => {
    const all = fixture()
    const partial = lastTurn(all.slice(3))
    expect(partial.partialStart).toBe(true)
    expect(partial.durationMs).toBeNull()
    const full = lastTurn(all)
    expect(full.id).toBe(partial.id)
    expect(full.partialStart).toBe(false)
    expect(full.durationMs).toBe(29_000)
  })
  it('formats elapsed time from actual persisted boundaries', () => {
    expect(turnDurationLabel(29_000, 'en')).toBe('29s')
    expect(turnDurationLabel(61_000, 'zh-CN')).toBe('1 分钟 1 秒')
  })
})

describe('turn fold state', () => {
  it('starts settled history folded, remembers manual choices, and folds old replies on the next send', () => {
    const state = new TurnFoldState()
    const rows = fixture()
    state.observe(rows, groupTimeline(rows, false))
    const turn = lastTurn(rows)
    expect(state.open(turn)).toBe(false)
    state.toggle(turn.id, true)
    state.observe(rows.map((row) => ({ ...row })), groupTimeline(rows, false))
    expect(state.open(turn)).toBe(true)
    const next = [...rows, row('u2', 'user_message', 'Next', 9, 'two')]
    state.observe(next, groupTimeline(next, false))
    expect(state.open(turn)).toBe(false)
  })
  it('automatically folds work when the current turn finishes', () => {
    const state = new TurnFoldState()
    const all = fixture()
    const live = all.slice(0, -1)
    state.observe(live, groupTimeline(live, true))
    expect(state.open(lastTurn(live, true))).toBe(true)
    state.observe(all, groupTimeline(all, false))
    expect(state.open(lastTurn(all))).toBe(false)
  })
  it('keeps explicit reader choices across turn completion', () => {
    const state = new TurnFoldState()
    const all = fixture()
    const live = all.slice(0, -1)
    state.observe(live, groupTimeline(live, true))
    state.toggle('one', true)
    state.observe(all, groupTimeline(all, false))
    expect(state.open(lastTurn(all))).toBe(true)
    state.toggle('one', false)
    state.observe(all, groupTimeline(all, false))
    expect(state.open(lastTurn(all))).toBe(false)
  })
  it('loading an earlier user message cannot reset a chosen expansion', () => {
    const state = new TurnFoldState()
    const rows = fixture()
    state.observe(rows, groupTimeline(rows, false))
    state.toggle('one', true)
    const older = [row('old', 'user_message', 'Old', 0, 'old'), ...rows]
    state.observe(older, groupTimeline(older, false))
    expect(state.open(lastTurn(rows))).toBe(true)
  })
  it('finding the current turn user row in a previous page preserves a manual fold', () => {
    const state = new TurnFoldState()
    const rows = fixture()
    state.observe(rows.slice(2, -1), groupTimeline(rows.slice(2, -1), true))
    state.toggle('one', false)
    state.observe(rows, groupTimeline(rows, false))
    expect(state.open(lastTurn(rows))).toBe(false)
  })
})
