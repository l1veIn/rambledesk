// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ApplicationTransport } from '../../application/applicationTransport'
import type { TerminalData, TerminalTrialSession } from '../../generated/feedback'
import { locale } from '../../preferences'
import { emptyTerminalState, type TerminalState } from '../terminalModel'
import type { TerminalSessionSnapshot } from './terminalController'
import TerminalWorkbench from './TerminalWorkbench.svelte'

type Hooks = { onData: (data: string) => void; onResize: (size: { cols: number; rows: number }) => void; onSelection: (text: string) => void }
const bridge = vi.hoisted(() => ({
  hooks: null as Hooks | null, screen: '', interactive: false,
  dispose: vi.fn(), paste: vi.fn(), focus: vi.fn(), factory: vi.fn(),
}))
vi.mock('./xtermAdapter', () => ({ createXtermAdapter: async (_root: HTMLElement, hooks: Hooks) => {
  bridge.factory(); bridge.hooks = hooks
  return { size: () => ({ cols: 80, rows: 24 }), fit() {}, focus: bridge.focus, paste: bridge.paste,
    reset: () => bridge.screen = '', write: async (output: string) => { bridge.screen += output },
    screen: () => bridge.screen, selection: () => '', setInteractive: (value: boolean) => bridge.interactive = value,
    dispose: bridge.dispose }
} }))
const data: TerminalData = { cwd: '/project', commands: [{ id: 'help', title: 'Help', command: 'node cli.mjs --help', description: 'Inspect available commands.' }] }
const snapshot = (changes: Partial<TerminalSessionSnapshot> = {}): TerminalSessionSnapshot => ({
  session_id: 'trial-1', request_id: 'request-1', cwd: '/project', shell: 'bash', cols: 80, rows: 24,
  status: 'running', exit_code: null, output: '$ ', first_sequence: 0, next_sequence: 2, truncated: false, ...changes,
})
let view: ReturnType<typeof mount> | undefined
let state: TerminalState
let wire: ReturnType<typeof vi.fn>
const onQuote = vi.fn(), onOpenReview = vi.fn()
const button = (text: string): HTMLButtonElement => [...document.querySelectorAll('button')].find((node) => node.textContent?.trim() === text || node.getAttribute('aria-label') === text)!
beforeEach(() => {
  locale.set('en'); state = emptyTerminalState(); bridge.screen = ''; bridge.hooks = null; bridge.interactive = false; vi.clearAllMocks()
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  wire = vi.fn(async (name: string) => name === 'stopTerminalSession' ? snapshot({ status: 'stopped' }) : snapshot())
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.body.replaceChildren(); vi.unstubAllGlobals() })
function open(readOnly = false, initial = emptyTerminalState()) {
  view = mount(TerminalWorkbench, { target: document.body, props: {
    requestId: 'request-1', data, transport: { call: wire } as unknown as ApplicationTransport,
    state: initial, readOnly, onChange: (next) => state = next, onQuote, onOpenReview,
  } })
}
async function start() {
  await vi.waitFor(() => expect(button('Start terminal')?.disabled).toBe(false))
  button('Start terminal').click()
  await vi.waitFor(() => expect(bridge.interactive).toBe(true))
}

describe('terminal workbench', () => {
  it('keeps saved output and allows local submission after the backend permanently loses the session', async () => {
    const saved: TerminalTrialSession = { id: 'saved-trial', cwd: '/project', shell: 'bash', cols: 80, rows: 24,
      status: 'running', exit_code: null, output: '$ previous output', screen: '$ previous output', truncated: false }
    wire.mockRejectedValueOnce({ code: 'INVALID_ARGUMENT', message: 'The terminal session was not found for this request.', retryable: false })
    open(false, { type: 'terminal', sessions: [saved] })
    await vi.waitFor(() => expect(document.querySelector('[role="alert"]')?.textContent).toContain('Saved output is preserved'))
    expect(state.sessions[0]).toEqual({ ...saved, status: 'stopped', exit_code: null })
    await (view as unknown as { prepareSubmission: () => Promise<void> }).prepareSubmission()
    expect(wire).toHaveBeenCalledTimes(1)
    expect(bridge.interactive).toBe(false)
  })
  it('reconnects stored sessions by id and leaves a missing process for a manual start', async () => {
    const saved: TerminalTrialSession = { id: 'saved-trial', cwd: '/project', shell: 'bash', cols: 80, rows: 24,
      status: 'running', exit_code: null, output: '$ previous output', screen: '$ previous output', truncated: false }
    wire.mockRejectedValueOnce(new Error('Session no longer exists'))
    open(false, { type: 'terminal', sessions: [saved] })
    await vi.waitFor(() => expect(document.querySelector('[role="alert"]')?.textContent).toContain('Session no longer exists'))
    expect(wire).toHaveBeenCalledWith('readTerminalSession', { request_id: 'request-1', session_id: 'saved-trial', after_sequence: null })
    expect(wire.mock.calls.some(([name]) => name === 'openTerminalSession')).toBe(false)
    expect(bridge.interactive).toBe(false)
    expect(button('Retry connection').disabled).toBe(false)
    button('Retry connection').click()
    await vi.waitFor(() => expect(wire.mock.calls.some(([name]) => name === 'openTerminalSession')).toBe(true))
  })
  it('starts on user action and fills a suggestion without appending Enter', async () => {
    open()
    await vi.waitFor(() => expect(bridge.hooks).not.toBeNull())
    expect(wire).not.toHaveBeenCalled()
    expect(button('Fill command: Help').disabled).toBe(true)
    await start()
    button('Fill command: Help').click()
    expect(bridge.paste).toHaveBeenCalledWith('node cli.mjs --help')
    expect(bridge.paste.mock.lastCall?.[0]).not.toContain('\r')
    expect(bridge.paste.mock.lastCall?.[0]).not.toContain('\n')
    bridge.hooks!.onData('\x1b[A\x03')
    await vi.waitFor(() => expect(wire).toHaveBeenCalledWith('writeTerminalSession', { request_id: 'request-1', session_id: 'trial-1', data: '\x1b[A\x03' }))
    expect(JSON.stringify(state)).not.toContain('data')
  })
  it('quotes selection into the feedback callback without adding it to the session log', async () => {
    open(); await start()
    bridge.hooks!.onSelection('unknown option --full\nTry --help')
    await vi.waitFor(() => expect(button('Quote selected output').disabled).toBe(false))
    button('Quote selected output').click()
    expect(onQuote).toHaveBeenCalledWith('Terminal trial · /project\n\n> unknown option --full\n> Try --help')
    expect(state.sessions[0].output).toBe('$ ')
    const toolbar = document.querySelector('[data-terminal-toolbar]')!
    expect(toolbar.contains(button('Full screen trial'))).toBe(true)
    button('Full screen trial').click()
    expect(onOpenReview).toHaveBeenCalledTimes(1)
  })
  it('captures the final output and screen before allowing submission', async () => {
    open(); await start()
    wire.mockImplementation(async (name: string) => name === 'stopTerminalSession'
      ? snapshot({ output: '$ cli\r\nfinal output', status: 'stopped', next_sequence: 20 }) : snapshot({ status: 'stopped' }))
    await (view as unknown as { prepareSubmission: () => Promise<void> }).prepareSubmission()
    expect(wire).toHaveBeenCalledWith('stopTerminalSession', { request_id: 'request-1', session_id: 'trial-1' })
    expect(state.sessions[0]).toMatchObject({ status: 'stopped', output: '$ cli\r\nfinal output', screen: '$ cli\r\nfinal output' })
  })
  it('shows saved history without opening a terminal or allowing execution', async () => {
    open(true, { type: 'terminal', sessions: [{
      id: 'trial-1', cwd: '/project', shell: 'bash', cols: 80, rows: 24, status: 'stopped', exit_code: null,
      output: '\x1b[32mCLI output\x1b[0m', screen: 'CLI output', truncated: true,
    }] })
    await vi.waitFor(() => expect(document.querySelector('[data-terminal-history]')?.textContent).toContain('CLI output'))
    expect(bridge.factory).not.toHaveBeenCalled()
    expect(wire).not.toHaveBeenCalled()
    expect(document.querySelector('[data-terminal-suggestions]')).toBeNull()
    expect(button('Start terminal')).toBeUndefined()
    expect(button('Stop terminal')).toBeUndefined()
  })
  it('retains the remote session when the workbench unmounts', async () => {
    open(); await start()
    await unmount(view!); view = undefined
    expect(bridge.dispose).toHaveBeenCalledTimes(1)
    expect(wire.mock.calls.some(([name]) => name === 'stopTerminalSession')).toBe(false)
  })
})
