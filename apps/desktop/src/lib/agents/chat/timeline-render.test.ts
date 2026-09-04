import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import type { SessionActivity, SessionToolCall } from '$lib/generated/feedback'
import { activitiesForSession } from '../managedSessionUi'
import SessionTimeline from './SessionTimeline.svelte'
import UserMessageText from './UserMessageText.svelte'

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
  it('renders the latest completed tool patch with details, diff and location in original sequence order', () => {
    const start = activity('call', 1, {})
    const final = activity('call', 1, { status: 'completed', raw_output: '{"written":true}', content: [{ type: 'diff', path: '/repo/main.ts', old_text: 'old content', new_text: 'new content' }] })
    const message: SessionActivity = { ...activity('answer', 2, {}), kind: 'agent_message', content: { type: 'message', blocks: [{ type: 'text', text: 'Final answer' }], truncated: false } }
    const { body } = render(SessionTimeline, { props: { sessionId: 'session', activities: activitiesForSession('session', [message, start, final]), runActive: false, onQuote: vi.fn() } })
    expect(body.match(/data-tool-id="call"/g)).toHaveLength(1)
    expect(body).toContain('data-tool-status="completed"')
    expect(body).toContain('Completed')
    expect(body).toContain('/repo/main.ts:4')
    expect(body).toContain('-old content')
    expect(body).toContain('+new content')
    expect(body).toContain('Raw output')
    expect(body).not.toContain('Old plain summary')
    expect(body.indexOf('data-activity-id="call"')).toBeLessThan(body.indexOf('Final answer'))
  })

  it('shows unfinished tool history without a spinner and escapes untrusted raw input', () => {
    const { body } = render(SessionTimeline, { props: { sessionId: 'session', activities: [activity('call', 1, { raw_input: '<script>unsafe()</script>' })], runActive: false, onQuote: vi.fn() } })
    expect(body).toContain('No final result')
    expect(body).not.toContain('animate-spin')
    expect(body).not.toContain('<script>unsafe()')
    expect(body).toContain('&lt;script')
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
