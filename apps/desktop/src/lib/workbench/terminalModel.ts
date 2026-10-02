import type { TerminalData, WorkbenchState } from '../generated/feedback'

export type TerminalState = Extract<WorkbenchState, { type: 'terminal' }>
export const TERMINAL_SESSION_LIMIT = 16
export const TERMINAL_OUTPUT_LIMIT = 262144
export const TERMINAL_SCREEN_LIMIT = 65536
export const TERMINAL_TOTAL_LIMIT = 600000
const stableId = /^[a-z0-9][a-z0-9_-]{0,63}$/
const textWithin = (value: unknown, max: number, required = false): value is string =>
  typeof value === 'string' && !value.includes('\0') && !/\p{Surrogate}/u.test(value)
  && [...value].length <= max && (!required || /[^\p{White_Space}]/u.test(value))
const integerWithin = (value: unknown, min: number, max: number): value is number =>
  typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max

export function emptyTerminalState(): TerminalState { return { type: 'terminal', sessions: [] } }

/** A shell prompt and startup banner are context, not the reviewer's opinion. */
export function hasTerminalInput(_data: TerminalData, _state: TerminalState | null): boolean { return false }

export function validateTerminalState(_data: TerminalData, state: TerminalState | null): string | null {
  if (!state) return null
  if (state.type !== 'terminal' || !Array.isArray(state.sessions) || state.sessions.length > TERMINAL_SESSION_LIMIT
    || Object.keys(state).some((key) => !['type', 'sessions'].includes(key))) return 'Invalid terminal trial state.'
  const ids = new Set<string>()
  let total = 0
  for (const session of state.sessions) {
    if (!session || typeof session.id !== 'string' || !stableId.test(session.id) || ids.has(session.id)
      || Object.keys(session).some((key) => !['id', 'cwd', 'shell', 'cols', 'rows', 'status', 'exit_code', 'output', 'screen', 'truncated'].includes(key))) return 'Invalid or duplicate terminal session id.'
    ids.add(session.id)
    if (!textWithin(session.cwd, 8192, true) || !textWithin(session.shell, 8192, true)
      || !integerWithin(session.cols, 20, 300) || !integerWithin(session.rows, 5, 100)
      || !['running', 'exited', 'stopped'].includes(session.status)
      || (session.exit_code !== null && !integerWithin(session.exit_code, -2147483648, 2147483647))
      || (session.status === 'running' && session.exit_code !== null)) return 'Invalid terminal session context.'
    if (!textWithin(session.output, TERMINAL_OUTPUT_LIMIT) || !textWithin(session.screen, TERMINAL_SCREEN_LIMIT)
      || typeof session.truncated !== 'boolean') return 'Terminal trial capture exceeds its limit.'
    total += [...session.output].length + [...session.screen].length
    if (total > TERMINAL_TOTAL_LIMIT) return 'Terminal trial capture exceeds its limit.'
  }
  return null
}
