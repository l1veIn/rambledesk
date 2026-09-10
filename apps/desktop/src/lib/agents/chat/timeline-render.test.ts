import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import type { SessionActivity, SessionToolCall } from '$lib/generated/feedback'
import { activitiesForSession } from '../managedSessionUi'
import SessionTimeline from './SessionTimeline.svelte'
import UserMessageText from './UserMessageText.svelte'
import ToolCallCard from './ToolCallCard.svelte'
import AgentTurn from './AgentTurn.svelte'
import TurnFooter from './TurnFooter.svelte'
import { groupTimeline } from './turn-presentation'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

function activity(id: string, sequence: number, tool: Partial<SessionToolCall>): SessionActivity {
  return { id, sequence, session_id: 'session', turn_id: 'turn', kind: 'tool_call', text: 'Old plain summary', tool_call_id: id, created_at: 'today',
    content: { type: 'tool_call', tool: { id, title: 'Update main.ts', name: 'write_file', kind: 'edit', status: 'in_progress',
      raw_input: '{"path":"/repo/main.ts"}', raw_output: null, content: [], locations: [{ path: '/repo/main.ts', line: 4 }], truncated: false, ...tool } } }
}

describe('structured timeline rendering', () => {
  it('folds settled process without mounting its details, while retaining the final reply', () => {
    const start = activity('call', 1, {})
    const final = activity('call', 1, { status: 'completed', raw_output: '{"written":true}', content: [{ type: 'diff', path: '/repo/main.ts', old_text: 'old content', new_text: 'new content' }] })
    const message: SessionActivity = { ...activity('answer', 2, {}), kind: 'agent_message', content: { type: 'message', blocks: [{ type: 'text', text: 'Final answer' }], truncated: false } }
    const { body } = render(SessionTimeline, { props: { sessionId: 'session', activities: activitiesForSession('session', [message, start, final]), runActive: false } })
    expect(body).not.toContain('data-tool-id="call"')
    expect(body).not.toContain('old content')
    expect(body).not.toContain('Raw output')
    expect(body).not.toContain('Old plain summary')
    expect(body).toContain('Final answer')
    expect(body).toContain('Copy reply')
    expect(body).not.toContain('Quote in message')
    const item = groupTimeline(activitiesForSession('session', [message, start, final]), false)[0]
    if (item.type !== 'turn') throw new Error('missing turn')
    const expanded = render(AgentTurn, { props: { turn: item.turn, open: true, onOpenChange: vi.fn() } }).body
    expect(expanded.match(/data-tool-id="call"/g)).toHaveLength(1)
    expect(expanded).toContain('data-tool-status="completed"')
    expect(expanded.indexOf('data-activity-id="call"')).toBeLessThan(expanded.indexOf('Final answer'))
    expect(expanded).not.toContain('Raw output')
    expect(expanded).not.toContain('Quote in message')
    if (final.content?.type !== 'tool_call') throw new Error('missing tool')
    const details = render(ToolCallCard, { props: { tool: final.content.tool, open: true } }).body
    expect(details).toContain('/repo/main.ts:4')
    expect(details).toContain('-old content')
    expect(details).toContain('+new content')
    expect(details).toContain('Raw output')
  })

  it('shows unfinished tool history without a spinner and escapes untrusted raw input', () => {
    const { body } = render(SessionTimeline, { props: { sessionId: 'session', activities: [activity('call', 1, { raw_input: '<script>unsafe()</script>' })], runActive: false } })
    expect(body).toContain('No final result')
    expect(body).not.toContain('animate-spin')
    expect(body).not.toContain('<script>unsafe()')
    expect(body).not.toContain('unsafe()')
    const tool = activity('call', 1, { raw_input: '<script>unsafe()</script>' }).content
    if (tool?.type !== 'tool_call') throw new Error('missing tool')
    const details = render(ToolCallCard, { props: { tool: tool.tool, open: true } }).body
    expect(details).toContain('&lt;script')
    expect(details).not.toContain('<script>unsafe()')
  })

  it('bounds long running and expanded work to the latest 60 rows while preserving replies and errors', () => {
    const process = Array.from({ length: 2000 }, (_, index) => activity(`tool-${index}`, index + 1, { status: 'completed' }))
    const error: SessionActivity = { ...activity('error', 2001, {}), kind: 'error', text: 'A tool needs attention', content: undefined }
    const running = render(SessionTimeline, { props: { sessionId: 'long-running', activities: [...process, error], runActive: true } }).body
    expect(running.match(/data-tool-id=/g)).toHaveLength(60)
    expect(running).toContain('data-tool-id="tool-1999"')
    expect(running).not.toContain('data-tool-id="tool-1939"')
    expect(running).toContain('View earlier work')
    expect(running).toContain('1940')
    expect(running).toContain('A tool needs attention')

    const answer: SessionActivity = { ...activity('answer', 2002, {}), kind: 'agent_message', text: 'Final reply', content: undefined }
    const completed = [...process, error, answer]
    const folded = render(SessionTimeline, { props: { sessionId: 'long-completed', activities: completed, runActive: false } }).body
    expect(folded).not.toContain('data-tool-id=')
    expect(folded).toContain('Final reply')
    expect(folded).toContain('A tool needs attention')
    const item = groupTimeline(completed, false)[0]
    if (item.type !== 'turn') throw new Error('missing turn')
    const expanded = render(AgentTurn, { props: { turn: item.turn, open: true, onOpenChange: vi.fn() } }).body
    expect(expanded.match(/data-tool-id=/g)).toHaveLength(60)
    expect(expanded).toContain('data-tool-id="tool-1999"')
    expect(expanded).not.toContain('data-tool-id="tool-1939"')
    expect(expanded).toContain('View earlier work')
    expect(expanded).toContain('Final reply')
    expect(expanded).toContain('A tool needs attention')
  })

  it('keeps a true completion timestamp without deriving one from message chunks', () => {
    const { body } = render(TurnFooter, { props: { copyText: 'Answer', completedAt: '2026-09-05T10:00:29Z' } })
    expect(body).toContain('datetime="2026-09-05T10:00:29Z"')
    expect(body).toContain('Completed at')
    const unknown = render(TurnFooter, { props: { copyText: 'Answer', completedAt: null } }).body
    expect(unknown).not.toContain('<time')
    expect(unknown).toContain('Copy reply')
  })

  it('keeps typed Markdown literal while rendering composer quote structure', () => {
    const { body } = render(UserMessageText, { props: { text: '> quoted code\n>\n> another line\n\n**literal question**\n<script>unsafe()</script>' } })
    expect(body).toContain('<blockquote')
    expect(body).toContain('quoted code')
    expect(body).toContain('**literal question**')
    expect(body).not.toContain('<strong>')
    expect(body).not.toContain('<script>')
  })
})
