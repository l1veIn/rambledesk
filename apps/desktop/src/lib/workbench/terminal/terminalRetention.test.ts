import { expect, it } from 'vitest'
import type { TerminalTrialSession } from '../../generated/feedback'
import { validateTerminalState } from '../terminalModel'
import { retainTerminalSessions } from './terminalRetention'

const trial = (id: string): TerminalTrialSession => ({ id, cwd: '/project', shell: 'bash', cols: 80, rows: 24,
  status: 'stopped', exit_code: null, output: '🙂'.repeat(262144), screen: 's'.repeat(65536), truncated: false })

it('keeps every trial and screen while trimming the oldest transcript to the combined draft budget', () => {
  const original = [trial('first'), trial('second')]
  const sessions = retainTerminalSessions(original)
  expect(sessions.map((session) => session.id)).toEqual(['first', 'second'])
  expect(sessions[0].screen).toBe(original[0].screen)
  expect(sessions[1]).toEqual(original[1])
  expect(sessions[0].truncated).toBe(true)
  expect(sessions[0].output.endsWith('🙂')).toBe(true)
  expect(validateTerminalState({ cwd: '/project' }, { type: 'terminal', sessions })).toBeNull()
  expect(original[0].truncated).toBe(false)
})

it('retains all 16 identities when the screens themselves exceed the combined limit', () => {
  const sessions = retainTerminalSessions(Array.from({ length: 16 }, (_, index) => trial(`trial-${index}`)))
  expect(sessions).toHaveLength(16)
  expect(sessions.at(-1)?.screen).toBe(trial('last').screen)
  expect(validateTerminalState({ cwd: '/project' }, { type: 'terminal', sessions })).toBeNull()
  expect(sessions[0].truncated).toBe(true)
})
