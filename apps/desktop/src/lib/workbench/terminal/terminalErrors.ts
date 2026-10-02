import type { TerminalTrialSession } from '../../generated/feedback'

/** Only the backend's permanent missing-session response retires a stored PTY. */
export function isMissingTerminalSession(cause: unknown): boolean {
  return !!cause && typeof cause === 'object' && 'code' in cause && 'message' in cause
    && cause.code === 'INVALID_ARGUMENT'
    && cause.message === 'The terminal session was not found for this request.'
}

export function lostTerminalSession(session: TerminalTrialSession): TerminalTrialSession {
  return { ...session, status: 'stopped', exit_code: session.status === 'running' ? null : session.exit_code }
}
