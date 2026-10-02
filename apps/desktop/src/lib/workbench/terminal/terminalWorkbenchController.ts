import type { TerminalState } from '../terminalModel'
import { emptyTerminalState, TERMINAL_SESSION_LIMIT } from '../terminalModel'
import type { WorkbenchController, WorkbenchControllerContext } from '../definitions/contracts'
import { createTerminalController, type TerminalRenderer } from './terminalController'
import { finalizeTerminalSession } from './terminalSnapshot'
import { retainTerminalSessions } from './terminalRetention'
import { isMissingTerminalSession } from './terminalErrors'

export type TerminalViewBinding = {
  renderer: TerminalRenderer
  onState: (state: TerminalState) => void
  onBusy: (busy: boolean) => void
  onReady: () => void
  onError: (cause: unknown) => void
}
export type TerminalWorkbenchController = WorkbenchController & {
  start: (cols: number, rows: number) => Promise<void>
  stop: () => Promise<void>
  write: (data: string) => Promise<void>
  resize: (cols: number, rows: number) => Promise<void>
  setLocked: (locked: boolean) => void
}

/** A request owns the trial; attaching a visible renderer never owns publication or the remote PTY. */
export function createTerminalWorkbenchController(context: WorkbenchControllerContext): TerminalWorkbenchController {
  const transport = context.runtime.transport
  const request_id = context.requestId
  let view: TerminalViewBinding | undefined
  let terminal: ReturnType<typeof createTerminalController> | undefined
  let initialization: Promise<void> = Promise.resolve()
  let pendingWrites: Promise<void> = Promise.resolve()
  let pendingWriteFailure: unknown
  let locked = false, preparing = false, disposed = false
  const state = (): TerminalState => {
    const current = context.getState()
    return current?.type === 'terminal' ? current : emptyTerminalState()
  }
  const busy = (value: boolean) => { context.setBusy(value); view?.onBusy(value) }
  function remember(session: TerminalState['sessions'][number]) {
    if (disposed || !context.isEditable()) return
    const current = state(), existing = current.sessions.some((item) => item.id === session.id)
    const sessions = existing ? current.sessions.map((item) => item.id === session.id ? session : item) : [...current.sessions, session]
    const next: TerminalState = { type: 'terminal', sessions: retainTerminalSessions(sessions) }
    context.updateState(next); view?.onState(next)
  }
  function detach(binding: TerminalViewBinding) {
    if (view !== binding) return
    view = undefined
    const previous = terminal
    previous?.pause()
    pendingWrites = previous?.flush().catch((cause) => { if (!isMissingTerminalSession(cause)) pendingWriteFailure = cause }).finally(() => previous.dispose()) ?? Promise.resolve()
    // Preserve a rejection for submission without generating an unhandled rejection on view close.
    void pendingWrites.catch(() => undefined)
    terminal = undefined
  }
  return {
    attach(binding) {
      const next = binding as TerminalViewBinding
      if (view) detach(view)
      view = next
      busy(true)
      initialization = (async () => {
        await pendingWrites
        if (disposed || view !== next) return
        const latest = state().sessions.at(-1)
        if (latest?.output) await next.renderer.write(latest.output)
        if (disposed || view !== next) return
        terminal = createTerminalController({
          initial: latest, renderer: {
            reset: () => { if (view === next) next.renderer.reset() },
            write: (output) => view === next ? next.renderer.write(output) : Promise.resolve(),
            screen: () => view === next ? next.renderer.screen() : state().sessions.at(-1)?.screen ?? '',
          },
          runtime: {
            open: (cols, rows) => transport.call('openTerminalSession', { request_id, cols, rows }),
            read: (session_id, after_sequence) => transport.call('readTerminalSession', { request_id, session_id, after_sequence }),
            write: (session_id, data) => transport.call('writeTerminalSession', { request_id, session_id, data }),
            resize: (session_id, cols, rows) => transport.call('resizeTerminalSession', { request_id, session_id, cols, rows }),
            stop: (session_id) => transport.call('stopTerminalSession', { request_id, session_id }),
          }, onSession: remember, onBusy: busy, onError: (cause) => next.onError(cause),
        })
        terminal.setLocked(locked || preparing)
        if (latest?.status === 'running') {
          try { await terminal.reconnect() } catch { /* Retain context and require an explicit retry. */ }
        }
        if (view === next) next.onReady()
      })().catch((cause) => { if (view === next) next.onError(cause) }).finally(() => busy(false))
      return () => detach(next)
    },
    async start(cols, rows) {
      await initialization
      if (!context.isEditable() || locked || preparing) return
      if (state().sessions.length >= TERMINAL_SESSION_LIMIT && state().sessions.at(-1)?.status !== 'running') return
      await terminal?.start(cols, rows)
      pendingWriteFailure = undefined
    },
    async stop() { await initialization; if (context.isEditable()) await terminal?.stop() },
    write: async (data) => { if (context.isEditable()) { await terminal?.write(data); pendingWriteFailure = undefined } },
    resize: async (cols, rows) => { if (context.isEditable()) await terminal?.resize(cols, rows) },
    setLocked(value) { locked = value; terminal?.setLocked(value || preparing) },
    async prepareSubmission() {
      if (!context.isEditable()) return
      preparing = true; busy(true); terminal?.setLocked(true); terminal?.pause()
      try {
        await initialization
        await pendingWrites
        if (pendingWriteFailure !== undefined) throw pendingWriteFailure
        try { await terminal?.flush() } catch (cause) { if (!isMissingTerminalSession(cause)) throw cause }
        // The same request-bound path runs with or without an attached view. The backend verifies
        // every saved identity, including stopped records; a draft flag is not runtime authority.
        for (const session of [...state().sessions]) {
          remember(await finalizeTerminalSession(session, (session_id) => transport.call('stopTerminalSession', { request_id, session_id }), true))
        }
      } catch (cause) { view?.onError(cause); throw cause }
      finally { preparing = false; terminal?.setLocked(locked); terminal?.resume(); busy(false) }
    },
    dispose() {
      disposed = true
      if (view) detach(view)
      terminal?.dispose()
      context.setBusy(false)
    },
  }
}
