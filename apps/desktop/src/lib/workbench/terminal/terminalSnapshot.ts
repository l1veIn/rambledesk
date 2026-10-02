import type { TerminalSessionSnapshot, TerminalTrialSession } from '../../generated/feedback'
import { terminalTextTail } from './terminalController'
import { terminalScreen } from './terminalScreen'
import { isMissingTerminalSession, lostTerminalSession } from './terminalErrors'

export async function finalizeTerminalSession(
  session: TerminalTrialSession,
  stop: (sessionId: string) => Promise<TerminalSessionSnapshot>,
  verifyStopped = false,
): Promise<TerminalTrialSession> {
  if (session.status === 'stopped' && !verifyStopped) return session
  try { return await replayTerminalSnapshot(await stop(session.id), session) }
  catch (cause) {
    if (!isMissingTerminalSession(cause)) throw cause
    return lostTerminalSession(session)
  }
}

/** Recover the final TUI screen when submission comes from a task/console without a mounted terminal. */
export async function replayTerminalSnapshot(snapshot: TerminalSessionSnapshot, previous?: TerminalTrialSession): Promise<TerminalTrialSession> {
  const { Terminal } = await import('@xterm/xterm')
  // Parsing a stopped transcript needs no DOM surface and must send no terminal responses.
  const terminal = new Terminal({ cols: snapshot.cols, rows: snapshot.rows, disableStdin: true, scrollback: 3000 })
  try {
    await new Promise<void>((resolve) => terminal.write(snapshot.output, resolve))
    return {
      id: snapshot.session_id, cwd: snapshot.cwd, shell: snapshot.shell, cols: snapshot.cols, rows: snapshot.rows,
      status: snapshot.status, exit_code: snapshot.exit_code,
      output: terminalTextTail(snapshot.output, 262_144), screen: terminalTextTail(terminalScreen(terminal), 65_536),
      truncated: snapshot.truncated || !!previous?.truncated || [...snapshot.output].length > 262_144,
    }
  } finally { terminal.dispose() }
}
