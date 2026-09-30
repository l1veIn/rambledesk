import type { TerminalTrialSession } from '../../generated/feedback'
import { TERMINAL_OUTPUT_LIMIT, TERMINAL_SCREEN_LIMIT, TERMINAL_TOTAL_LIMIT } from '../terminalModel'

/** Preserve every trial's identity; trim older transcripts before screens to fit the draft budget. */
export function retainTerminalSessions(sessions: readonly TerminalTrialSession[]): TerminalTrialSession[] {
  const retained = sessions.map((session) => {
    const output = [...session.output], screen = [...session.screen]
    const trimmed = output.length > TERMINAL_OUTPUT_LIMIT || screen.length > TERMINAL_SCREEN_LIMIT
    return { ...session, output: output.slice(-TERMINAL_OUTPUT_LIMIT).join(''), screen: screen.slice(-TERMINAL_SCREEN_LIMIT).join(''),
      truncated: session.truncated || trimmed }
  })
  let excess = retained.reduce((total, session) => total + [...session.output].length + [...session.screen].length, 0) - TERMINAL_TOTAL_LIMIT
  for (const field of ['output', 'screen'] as const) {
    for (const session of retained) {
      if (excess <= 0) return retained
      const characters = [...session[field]]
      const removed = Math.min(excess, characters.length)
      if (removed) { session[field] = characters.slice(removed).join(''); session.truncated = true; excess -= removed }
    }
  }
  return retained
}
