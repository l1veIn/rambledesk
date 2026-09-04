import { describe, expect, it } from 'vitest'
import type { SessionActivity, SessionToolCall } from '$lib/generated/feedback'
import { activitiesForSession } from '../managedSessionUi'
import { activityInRunningTurn, activityMessage, activityQuoteText, activityTool, formatToolJson, inlineMediaSource, latestStreamingActivity, locationLabel, toolPresentation } from './activity-presentation'
import { diffLines } from './diff-lines'
import { generateUnifiedDiff } from './unified-diff-generator'

export function toolActivity(overrides: Partial<SessionActivity> = {}, tool: Partial<SessionToolCall> = {}): SessionActivity {
  return { id: 'a', session_id: 'session-a', sequence: 1, turn_id: 'turn-a', kind: 'tool_call', text: 'Legacy summary',
    tool_call_id: 'tool-1', created_at: 'today', content: { type: 'tool_call', tool: {
      id: 'tool-1', name: 'write_file', title: 'Update source', kind: 'edit', status: 'in_progress',
      raw_input: '{"path":"/repo/main.ts"}', raw_output: null, content: [], locations: [{ path: '/repo/main.ts', line: 4 }], truncated: false, ...tool,
    } }, ...overrides }
}

describe('structured activity presentation', () => {
  it('takes the final full tool patch without moving it after later messages or merging reused tool IDs across turns', () => {
    const start = toolActivity()
    const final = toolActivity({}, { status: 'completed', raw_output: '{"written":true}', content: [{ type: 'diff', path: '/repo/main.ts', old_text: 'old', new_text: 'new' }] })
    const nextTurn = toolActivity({ id: 'c', sequence: 3, turn_id: 'turn-b' })
    const visible = activitiesForSession('session-a', [
      nextTurn, start, toolActivity({ id: 'b', sequence: 2, kind: 'agent_message', content: undefined, text: 'After tool' }),
      final, toolActivity({ session_id: 'session-b', text: 'Foreign patch' }),
    ])
    expect(visible.map((item) => item.id)).toEqual(['a', 'b', 'c'])
    expect(activityTool(visible[0])).toMatchObject({ status: 'completed', raw_output: '{"written":true}', content: [{ type: 'diff' }] })
    expect(activityTool(visible[2])?.status).toBe('in_progress')
    expect(activitiesForSession('session-b', [start, toolActivity({ session_id: 'session-b' })])).toHaveLength(1)
  })

  it('does not invent successful completion or animate prior interrupted tools during a later turn', () => {
    expect(toolPresentation('in_progress', false)).toMatchObject({ spinning: false, incomplete: true, label: 'No final result' })
    expect(toolPresentation('completed', true)).toMatchObject({ spinning: false, incomplete: false, label: 'Completed' })
    const old = toolActivity()
    const current = toolActivity({ id: 'b', sequence: 2, turn_id: 'turn-b' })
    expect(activityInRunningTurn(old, [old, current], true)).toBe(false)
    expect(activityInRunningTurn(current, [old, current], true)).toBe(true)
    expect(activityInRunningTurn(current, [old, current], false)).toBe(false)
    expect(latestStreamingActivity([old, { ...current, kind: 'agent_thought' }], true)).toBe('b')
    expect(latestStreamingActivity([old], true)).toBeNull()
    expect(latestStreamingActivity([{ ...current, kind: 'agent_message' }], false)).toBeNull()
  })

  it('preserves old text and quotes actual structured results rather than obsolete tool summaries', () => {
    expect(activityMessage(toolActivity({ content: undefined }))).toEqual({ blocks: [{ type: 'text', text: 'Legacy summary' }], truncated: false })
    const quote = activityQuoteText(toolActivity({}, { content: [
      { type: 'text', text: 'Changed source' }, { type: 'diff', path: '/repo/main.ts', old_text: 'before', new_text: 'after' },
      { type: 'resource', uri: 'file:///repo/main.ts', name: 'Source', mime_type: 'text/plain', text: 'File contents' },
    ] }))
    expect(quote).toContain('-before\n+after')
    expect(quote).toContain('Source\nFile contents')
    expect(quote).not.toContain('Legacy summary')
    expect(formatToolJson('{"count":2}')).toBe('{\n  "count": 2\n}')
    expect(formatToolJson('{"partial":')).toBe('{"partial":')
    expect(locationLabel({ path: '/repo/main.ts', line: 0 })).toBe('/repo/main.ts:0')
  })

  it('only creates bounded raster/audio data URLs and never automatically loads agent-provided URIs', () => {
    expect(inlineMediaSource({ type: 'image', mime_type: 'image/png', data: 'aGVsbG8=', uri: null })).toBe('data:image/png;base64,aGVsbG8=')
    expect(inlineMediaSource({ type: 'image', mime_type: 'image/svg+xml', data: 'aGVsbG8=', uri: null })).toBeNull()
    expect(inlineMediaSource({ type: 'image', mime_type: 'image/png', data: null, uri: 'https://example.com/track.png' })).toBeNull()
    expect(inlineMediaSource({ type: 'audio', mime_type: 'audio/mpeg', data: 'a'.repeat(1_400_001) })).toBeNull()
  })
})

describe('diff view line coordinates', () => {
  it('preserves header-like body lines and assigns correct line numbers for additions and deletions', () => {
    const rows = diffLines(generateUnifiedDiff('-- old marker', '++ new marker', 'source.ts'))
    expect(rows.slice(3)).toEqual([
      { kind: 'deletion', text: '--- old marker', oldLine: 1, newLine: null },
      { kind: 'addition', text: '+++ new marker', oldLine: null, newLine: 1 },
    ])
    expect(diffLines(generateUnifiedDiff('', 'first\nsecond', 'new.ts')).slice(3).map((row) => [row.oldLine, row.newLine])).toEqual([[null, 1], [null, 2]])
  })
})
