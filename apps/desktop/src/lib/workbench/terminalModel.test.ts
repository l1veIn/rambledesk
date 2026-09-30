import { describe, expect, it } from 'vitest'
import type { TerminalData, TerminalTrialSession, WorkbenchSpec } from '../generated/feedback'
import { resolveWorkbenchPolicy, decodeWorkbenchState, workbenchSupportsApproval } from '../workbenchPolicy'
import { canSubmitWorkbench, readWorkbenchState, withWorkbenchState } from '../workbenchState'
import { snapshotFeedbackDraftMarkdown, updateFeedbackDraftDocument } from '../feedbackDraftDocument'
import { emptyTerminalState, validateTerminalState } from './terminalModel'

const data = { cwd: '/project', commands: [{ id: 'help', title: 'Explore help', command: 'my-cli --help' }] } satisfies TerminalData
const spec: WorkbenchSpec = { type: 'terminal', version: 1, data }
const session = (): TerminalTrialSession => ({ id: 'trial-1', cwd: '/project', shell: 'sh', cols: 80, rows: 24,
  status: 'stopped', exit_code: null, output: '\x1b[32mUsage: my-cli 😀\x1b[0m\r\n', screen: 'Usage: my-cli 😀', truncated: false })
const state = () => ({ type: 'terminal' as const, sessions: [session()] })

describe('terminal trial contract', () => {
  it('requires an opinion even with output, preserves notes-only feedback, and exposes no approval', () => {
    expect(resolveWorkbenchPolicy(spec)?.type).toBe('terminal')
    expect(workbenchSupportsApproval(spec)).toBe(false)
    expect(canSubmitWorkbench(spec, state(), '')).toBe(false)
    expect(canSubmitWorkbench(spec, state(), 'Help is clear')).toBe(true)
    expect(canSubmitWorkbench(spec, null, 'CLI could not start')).toBe(true)
    expect(canSubmitWorkbench(spec, emptyTerminalState(), 'CLI could not start')).toBe(true)
    expect(canSubmitWorkbench(spec, { type: 'questions', answers: [] }, 'CLI could not start')).toBe(false)
  })

  it('keeps captured screen and ANSI output while feedback text and attachments change', () => {
    const evidence = state()
    evidence.sessions[0].truncated = true
    const snapshot = withWorkbenchState(snapshotFeedbackDraftMarkdown('First impression'), evidence)
    const changed = updateFeedbackDraftDocument(snapshot, (doc) => ({ ...doc, content: [
      { type: 'paragraph', content: [{ type: 'text', text: 'Added opinion and screenshot', marks: [{ type: 'link', attrs: { href: 'attachment://shot' } }] }] },
    ] }))
    expect(readWorkbenchState(changed.documentJson)).toEqual(evidence)
    expect(decodeWorkbenchState(evidence)).toEqual(evidence)
    expect(validateTerminalState(data, evidence)).toBeNull()
    expect(evidence.sessions[0]).not.toHaveProperty('input')
    expect(evidence.sessions[0]).not.toHaveProperty('commands')
  })

  it('rejects invalid dimensions, outcome, duplicate sessions, unknown fields, and over-limit capture', () => {
    const invalid = [
      { cols: 19 }, { rows: 101 }, { status: 'running' as const, exit_code: 0 }, { cwd: '' },
      { output: 'x'.repeat(262145) }, { screen: '😀'.repeat(65537) }, { truncated: undefined }, { input: 'password' },
    ]
    for (const patch of invalid) {
      const evidence = { type: 'terminal' as const, sessions: [{ ...session(), ...patch } as TerminalTrialSession] }
      expect(validateTerminalState(data, evidence)).not.toBeNull()
      expect(canSubmitWorkbench(spec, evidence, 'Useful notes')).toBe(false)
    }
    expect(validateTerminalState(data, { type: 'terminal', sessions: [session(), session()] })).not.toBeNull()
    expect(validateTerminalState(data, { type: 'terminal', sessions: Array.from({ length: 17 }, (_, index) => ({ ...session(), id: `trial-${index}` })) })).not.toBeNull()
    const bounded = { ...session(), output: 'x'.repeat(262144), screen: '😀'.repeat(65536), truncated: true }
    expect(validateTerminalState(data, { type: 'terminal', sessions: [bounded] })).toBeNull()
    expect(validateTerminalState(data, { type: 'terminal', sessions: [bounded, { ...bounded, id: 'trial-2' }] })).not.toBeNull()
  })

  it('keeps structurally editable invalid drafts but rejects broken shapes', () => {
    expect(decodeWorkbenchState({ type: 'terminal', sessions: [{ ...session(), cols: 1 }] })).not.toBeNull()
    for (const bad of [{ type: 'terminal', sessions: [{}] }, { type: 'terminal', sessions: null },
      { type: 'terminal', sessions: [{ ...session(), status: 'unknown' }] }]) expect(decodeWorkbenchState(bad)).toBeNull()
  })

  it('rejects suggestion controls, unknown input fields, duplicate command ids and future versions', () => {
    for (const control of ['\n', '\r', '\t', '\x1b', '\x7f']) {
      const input = { ...spec, data: { ...data, commands: [{ ...data.commands[0], command: `my-cli${control}` }] } }
      expect(resolveWorkbenchPolicy(input)).toBeNull()
    }
    for (const input of [{ ...spec, version: 2 }, { ...spec, data: { ...data, future: true } },
      { ...spec, data: { ...data, commands: [data.commands[0], data.commands[0]] } },
      { ...spec, data: { ...data, commands: [{ ...data.commands[0], command: '😀'.repeat(4001) }] } }]) {
      expect(resolveWorkbenchPolicy(input)).toBeNull()
    }
  })
})
