import type { TerminalSessionSnapshot, TerminalTrialSession } from '../../generated/feedback'
import { isMissingTerminalSession, lostTerminalSession } from './terminalErrors'
export type { TerminalSessionSnapshot } from '../../generated/feedback'

export type TerminalRuntime = {
  open: (cols: number, rows: number) => Promise<TerminalSessionSnapshot>
  read: (sessionId: string, afterSequence: number | null) => Promise<TerminalSessionSnapshot>
  write: (sessionId: string, data: string) => Promise<unknown>
  resize: (sessionId: string, cols: number, rows: number) => Promise<unknown>
  stop: (sessionId: string) => Promise<TerminalSessionSnapshot>
}
export type TerminalRenderer = {
  reset: () => void
  write: (output: string) => Promise<void>
  screen: () => string
}
const MAX_OUTPUT = 262_144
const MAX_SCREEN = 65_536
export const terminalTextTail = (text: string, limit: number): string => [...text].slice(-limit).join('')
export const isTerminalResponse = (data: string): boolean => /^\x1b\[(?:\d+;\d+R|[?>]?[\d;]*c|[?>]?\d+n)$/.test(data)
const sameSession = (a: TerminalTrialSession | undefined, b: TerminalTrialSession): boolean =>
  !!a && a.id === b.id && a.cwd === b.cwd && a.shell === b.shell && a.cols === b.cols && a.rows === b.rows
  && a.status === b.status && a.exit_code === b.exit_code && a.output === b.output && a.screen === b.screen && a.truncated === b.truncated

/** PTY output is authoritative. Keystrokes are forwarded but never persisted. */
export function createTerminalController(options: {
  runtime: TerminalRuntime; renderer: TerminalRenderer; initial?: TerminalTrialSession
  onSession: (session: TerminalTrialSession) => void
  onBusy: (busy: boolean) => void
  onError: (cause: unknown) => void
  pollMs?: number
}) {
  let session = options.initial
  let sequence: number | null = null
  let timer: ReturnType<typeof setTimeout> | undefined
  let reading: Promise<void> | null = null
  let writes: Promise<void> = Promise.resolve()
  let writeFailure: unknown
  let disposed = false
  let locked = false
  let busy = false
  let starting = false
  let paused = false
  let operation: Promise<void> | null = null
  const setBusy = (value: boolean) => { busy = value; if (!disposed) options.onBusy(value) }
  const clearTimer = () => { if (timer) clearTimeout(timer); timer = undefined }
  const report = (cause: unknown) => { if (!disposed) options.onError(cause) }
  function retireMissing(cause: unknown): boolean {
    if (!session || !isMissingTerminalSession(cause)) return false
    session = lostTerminalSession(session)
    writeFailure = undefined
    if (!disposed) options.onSession(session)
    return true
  }
  function schedule() {
    clearTimer()
    if (!disposed && !paused && session?.status === 'running') {
      timer = setTimeout(() => void poll().catch(report), options.pollMs ?? 200)
    }
  }
  async function apply(snapshot: TerminalSessionSnapshot, reset: boolean) {
    if (disposed) return
    const lostOutput = sequence !== null && snapshot.first_sequence > sequence
    const replay = reset || lostOutput
    if (replay) options.renderer.reset()
    if (snapshot.output) await options.renderer.write(snapshot.output)
    if (disposed) return
    const previous = session?.id === snapshot.session_id ? session : undefined
    const output = replay ? snapshot.output : (previous?.output ?? '') + snapshot.output
    session = {
      id: snapshot.session_id, cwd: snapshot.cwd, shell: snapshot.shell,
      cols: snapshot.cols, rows: snapshot.rows, status: snapshot.status, exit_code: snapshot.exit_code,
      output: terminalTextTail(output, MAX_OUTPUT),
      screen: terminalTextTail(options.renderer.screen(), MAX_SCREEN),
      truncated: snapshot.truncated || !!previous?.truncated || lostOutput || [...output].length > MAX_OUTPUT,
    }
    sequence = snapshot.next_sequence
    if (!sameSession(previous, session)) options.onSession(session)
  }
  async function poll() {
    if (disposed || !session) return
    if (reading) return reading
    const after = sequence
    const id = session.id
    reading = (async () => {
      const snapshot = await options.runtime.read(id, after)
      if (!disposed && session?.id === id) await apply(snapshot, after === null)
    })()
    try { await reading } catch (cause) { retireMissing(cause); throw cause }
    finally { reading = null; schedule() }
  }
  async function start(cols: number, rows: number) {
    if (disposed || locked || busy) return
    clearTimer(); paused = true; starting = true; setBusy(true)
    operation = (async () => {
      try {
        if (reading) await reading.catch(() => undefined)
        await writes
        writeFailure = undefined
        if (session?.status === 'exited') {
          // Finalize the previous trial before opening a replacement with its own output.
          try { await apply(await options.runtime.stop(session.id), true) }
          catch (cause) { if (!retireMissing(cause)) throw cause }
        }
        if (disposed) return
        const opened = await options.runtime.open(cols, rows)
        if (disposed) return
        session = session?.id === opened.session_id ? session : {
          id: opened.session_id, cwd: opened.cwd, shell: opened.shell, cols: opened.cols, rows: opened.rows,
          status: opened.status, exit_code: opened.exit_code, output: '', screen: '', truncated: false,
        }
        options.onSession(session)
        sequence = null
        await poll()
        paused = false
      } catch (cause) { report(cause); throw cause }
      finally { starting = false; setBusy(false); schedule() }
    })()
    try { await operation } finally { operation = null }
  }
  async function reconnect() {
    if (disposed || locked || busy || !session) return
    clearTimer(); paused = true; starting = true; setBusy(true)
    operation = (async () => {
      try {
        if (reading) await reading.catch(() => undefined)
        await writes
        writeFailure = undefined
        sequence = null
        // A stored session is a reference, never authorization to start a new shell.
        await poll()
        paused = false
      } catch (cause) { report(cause); throw cause }
      finally { starting = false; setBusy(false); schedule() }
    })()
    try { await operation } finally { operation = null }
  }
  function write(data: string): Promise<void> {
    const startupResponse = starting && isTerminalResponse(data)
    if (disposed || locked || (!startupResponse && (busy || paused)) || session?.status !== 'running' || !data) return Promise.resolve()
    const id = session.id
    const next = writes.then(async () => { await options.runtime.write(id, data); writeFailure = undefined })
    writes = next.catch((cause) => { writeFailure = cause; report(cause) })
    return next
  }
  async function resize(cols: number, rows: number) {
    if (disposed || locked || busy || paused || session?.status !== 'running') return
    const id = session.id
    const next = writes.then(async () => { await options.runtime.resize(id, cols, rows) })
    writes = next.catch(report)
    await writes
  }
  async function stop() {
    if (disposed || locked || busy || session?.status !== 'running') return
    await finish()
  }
  async function finish() {
    if (disposed) return
    if (operation) await operation
    if (!session) return
    if (session.status === 'stopped') return
    clearTimer(); paused = true; setBusy(true)
    try {
      if (reading) await reading
      await writes
      if (writeFailure !== undefined) throw writeFailure
      // Stop resolves only after the PTY reader has drained its final retained output.
      await apply(await options.runtime.stop(session.id), true)
    } catch (cause) {
      report(cause)
      if (retireMissing(cause)) return
      paused = false; schedule(); throw cause
    }
    finally { setBusy(false) }
  }
  return {
    start, reconnect, write, resize, stop, prepareSubmission: finish,
    pause: () => { paused = true; clearTimer() },
    resume: () => { paused = false; schedule() },
    flush: async () => {
      if (operation) await operation
      if (reading) await reading
      await writes
      if (writeFailure !== undefined) throw writeFailure
    },
    setLocked: (value: boolean) => { locked = value },
    session: () => session,
    dispose: () => { disposed = true; clearTimer() },
  }
}
