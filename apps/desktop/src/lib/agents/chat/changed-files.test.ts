import { describe, expect, it } from 'vitest'
import type { SessionActivity, SessionContentBlock, SessionToolCall } from '$lib/generated/feedback'
import { directoryOf, displayPath, fileNameOf, pathFromRawInput, turnChangedFiles } from './changed-files'

function toolActivity(tool: Partial<SessionToolCall> = {}, overrides: Partial<SessionActivity> = {}): SessionActivity {
  return {
    id: 'a', session_id: 'session-a', sequence: 1, turn_id: 'turn-a', kind: 'tool_call',
    text: 'Legacy summary', tool_call_id: 'tool-1', created_at: 'today',
    content: { type: 'tool_call', tool: {
      id: 'tool-1', name: 'write_file', title: 'Update source', kind: 'edit', status: 'completed',
      raw_input: null, raw_output: null, content: [], locations: [], truncated: false, ...tool,
    } },
    ...overrides,
  }
}

function diffBlock(path: string, oldText: string | null, newText: string): SessionContentBlock {
  return { type: 'diff', path, old_text: oldText, new_text: newText }
}

describe('turn changed files', () => {
  it('sums diff content per file and keeps first-seen order', () => {
    const files = turnChangedFiles([
      toolActivity({ content: [diffBlock('/repo/b.ts', 'one\ntwo\n', 'one\nthree\n')] }),
      toolActivity({ content: [diffBlock('/repo/a.ts', null, 'first\nsecond\n')] }, { id: 'b', sequence: 2 }),
      toolActivity({ content: [diffBlock('/repo/b.ts', 'one\nthree\n', 'one\nthree\nfour\n')] }, { id: 'c', sequence: 3 }),
    ])

    expect(files.map((file) => [file.path, file.kind, file.additions, file.deletions])).toEqual([
      ['/repo/b.ts', 'modified', 2, 1],
      ['/repo/a.ts', 'added', 2, 0],
    ])
    expect(files[0]!.diff).toContain('--- a//repo/b.ts')
    // Two edits to one file keep both diff chunks in a single entry.
    expect(files[0]!.diff.match(/^@@ /gm)?.length).toBe(2)
    expect(files[0]!.id).toBe('/repo/b.ts')
  })

  it('reports a deleted file as removed even when the same turn created it', () => {
    const files = turnChangedFiles([
      toolActivity({ content: [diffBlock('/repo/old.ts', 'body\n', '')] }, { id: 'a' }),
      toolActivity({ kind: 'delete', content: [diffBlock('/repo/old.ts', 'body\n', '')] }, { id: 'b', sequence: 2 }),
      toolActivity({ kind: 'delete', locations: [{ path: '/repo/gone.ts', line: null }] }, { id: 'c', sequence: 3 }),
    ])

    expect(files.map((file) => [file.path, file.kind])).toEqual([
      ['/repo/old.ts', 'removed'],
      ['/repo/gone.ts', 'removed'],
    ])
    expect(files[1]!.diff).toBe('')
  })

  it('prefers diff content over a location that repeats the same file in another form', () => {
    const files = turnChangedFiles([
      toolActivity({
        content: [diffBlock('/repo/src/main.ts', 'old\n', 'new\n')],
        locations: [{ path: '/repo/main.ts', line: 1 }],
      }),
    ])

    expect(files.map((file) => file.path)).toEqual(['/repo/src/main.ts'])
  })

  it('lists an edited file reported only through locations or bounded raw input', () => {
    const files = turnChangedFiles([
      toolActivity({ locations: [{ path: 'C:\\repo\\main.ts', line: 4 }] }),
      toolActivity({ raw_input: '{"input":{"file_path":"/repo/other.ts"}}' }, { id: 'b', sequence: 2 }),
    ])

    expect(files.map((file) => file.path)).toEqual(['C:/repo/main.ts', '/repo/other.ts'])
    expect(files.every((file) => file.kind === 'modified' && file.additions === 0 && file.diff === '')).toBe(true)
  })

  it('ignores read, search and malformed tool payloads', () => {
    const files = turnChangedFiles([
      toolActivity({ kind: 'read', locations: [{ path: '/repo/read.ts', line: 1 }] }),
      toolActivity({ kind: 'search', locations: [{ path: '/repo/search.ts', line: 1 }] }, { id: 'b', sequence: 2 }),
      toolActivity({ kind: 'edit', raw_input: '{"partial":' }, { id: 'c', sequence: 3 }),
      toolActivity({ kind: 'edit', raw_input: '{"note":"no path here"}' }, { id: 'd', sequence: 4 }),
    ])

    expect(files).toEqual([])
  })

  it('keeps a diff reported by a tool whose kind is not an edit', () => {
    const files = turnChangedFiles([
      toolActivity({ kind: 'other', content: [diffBlock('/repo/tool.ts', 'a\n', 'b\n')] }),
    ])

    expect(files).toHaveLength(1)
    expect(files[0]).toMatchObject({ path: '/repo/tool.ts', kind: 'modified', additions: 1, deletions: 1 })
  })

  it('reads only a bounded, path-shaped field from raw input', () => {
    expect(pathFromRawInput('{"file_path":"/repo/a.ts"}')).toBe('/repo/a.ts')
    expect(pathFromRawInput('{"arguments":{"path":"/repo/b.ts"}}')).toBe('/repo/b.ts')
    expect(pathFromRawInput('{"other":true}')).toBeNull()
    expect(pathFromRawInput('not json')).toBeNull()
    expect(pathFromRawInput(null)).toBeNull()
    expect(pathFromRawInput(JSON.stringify({ a: { b: { c: { d: { path: '/too/deep.ts' } } } } }))).toBeNull()
  })

  it('derives readable names and session-relative display paths', () => {
    expect(fileNameOf('/repo/src/main.ts')).toBe('main.ts')
    expect(directoryOf('/repo/src/main.ts')).toBe('/repo/src')
    expect(directoryOf('main.ts')).toBe('')
    expect(displayPath('/repo/src/main.ts', '/repo')).toBe('src/main.ts')
    expect(displayPath('/repo/src/main.ts', '/repo/')).toBe('src/main.ts')
    expect(displayPath('/elsewhere/main.ts', '/repo')).toBe('/elsewhere/main.ts')
    expect(displayPath('/repo/src/main.ts', '')).toBe('/repo/src/main.ts')
  })
})
