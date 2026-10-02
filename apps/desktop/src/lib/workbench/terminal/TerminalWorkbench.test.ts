// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ApplicationTransport } from '../../application/applicationTransport'
import { createTerminalWorkbenchController, type TerminalWorkbenchController } from './terminalWorkbenchController'
import type { TerminalData, TerminalTrialSession } from '../../generated/feedback'
import { locale } from '../../preferences'
import { emptyTerminalState, type TerminalState } from '../terminalModel'
import { TERMINAL_APPEARANCE_KEY, terminalAppearanceFor } from './terminalAppearance'
import type { TerminalSessionSnapshot } from './terminalController'
import TerminalWorkbench from './TerminalWorkbench.svelte'

type Hooks = { onData: (data: string) => void; onResize: (size: { cols: number; rows: number }) => void; onSelection: (text: string) => void }
const bridge = vi.hoisted(() => ({
  hooks: null as Hooks | null, screen: '', interactive: false,
  dispose: vi.fn(), focus: vi.fn(), factory: vi.fn(), setAppearance: vi.fn(),
}))
vi.mock('./xtermAdapter', () => ({ createXtermAdapter: async (_root: HTMLElement, hooks: Hooks) => {
  bridge.factory(); bridge.hooks = hooks
  return { size: () => ({ cols: 80, rows: 24 }), fit() {}, focus: bridge.focus,
    reset: () => bridge.screen = '', write: async (output: string) => { bridge.screen += output },
    screen: () => bridge.screen, selection: () => '', setInteractive: (value: boolean) => bridge.interactive = value,
    setAppearance: bridge.setAppearance, dispose: bridge.dispose }
} }))
const data: TerminalData = { cwd: '/project' }
const snapshot = (changes: Partial<TerminalSessionSnapshot> = {}): TerminalSessionSnapshot => ({
  session_id: 'trial-1', request_id: 'request-1', cwd: '/project', shell: 'bash', cols: 80, rows: 24,
  status: 'running', exit_code: null, output: '$ ', first_sequence: 0, next_sequence: 2, truncated: false, ...changes,
})
let view: ReturnType<typeof mount> | undefined
let state: TerminalState
let runtime: TerminalWorkbenchController
let wire: ReturnType<typeof vi.fn>
const onQuote = vi.fn(), onOpenReview = vi.fn()
const button = (text: string): HTMLButtonElement => [...document.querySelectorAll('button')].find((node) => node.textContent?.trim() === text || node.getAttribute('aria-label') === text)!
beforeEach(() => {
  localStorage.removeItem(TERMINAL_APPEARANCE_KEY)
  locale.set('en'); state = emptyTerminalState(); bridge.screen = ''; bridge.hooks = null; bridge.interactive = false; vi.clearAllMocks()
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  wire = vi.fn(async (name: string) => name === 'stopTerminalSession' ? snapshot({ status: 'stopped' }) : snapshot())
})
afterEach(async () => { if (view) await unmount(view); view = undefined; runtime?.dispose(); document.body.replaceChildren(); vi.unstubAllGlobals() })
function open(readOnly = false, initial = emptyTerminalState()) {
  state = initial
  runtime = createTerminalWorkbenchController({requestId:'request-1',runtime:{transport:{call:wire} as unknown as ApplicationTransport},
    getState:() => state, updateState:(next) => { if(next.type === 'terminal') state=next }, isEditable:() => !readOnly, setBusy:() => {}})
  view = mount(TerminalWorkbench, { target: document.body, props: {
    data, runtime, state: initial, readOnly, onQuote, onOpenReview,
  } })
}
async function start() {
  await vi.waitFor(() => expect(button('Start terminal')?.disabled).toBe(false))
  button('Start terminal').click()
  await vi.waitFor(() => expect(bridge.interactive).toBe(true))
}

describe('terminal workbench', () => {
  it('applies and saves color changes without replacing the live renderer or trial', async () => {
    wire.mockImplementation(async (name: string, input: { after_sequence?: number | null }) =>
      name === 'readTerminalSession' && input.after_sequence !== null ? snapshot({ output: '' }) : snapshot())
    open(); await start()
    bridge.hooks!.onSelection('selected output')
    const saved = structuredClone(state), screen = bridge.screen
    const before = wire.mock.calls.filter(([name]) => name === 'openTerminalSession' || name === 'stopTerminalSession')
    button('Terminal style').click()
    await vi.waitFor(() => expect(button('Paper')).toBeDefined())
    button('Paper').click()
    await vi.waitFor(() => expect(bridge.setAppearance).toHaveBeenLastCalledWith(terminalAppearanceFor('paper').theme))
    expect(button('Paper').getAttribute('aria-pressed')).toBe('true')
    expect(localStorage.getItem(TERMINAL_APPEARANCE_KEY)).toBe('paper')
    expect(document.querySelector('[data-terminal-workbench]')?.getAttribute('data-terminal-style')).toBe('paper')
    expect(bridge.factory).toHaveBeenCalledTimes(1)
    expect(bridge.dispose).not.toHaveBeenCalled()
    expect(bridge.interactive).toBe(true)
    expect(bridge.screen).toBe(screen)
    expect(state).toEqual(saved)
    expect(wire.mock.calls.filter(([name]) => name === 'openTerminalSession' || name === 'stopTerminalSession')).toEqual(before)
    button('Close').click()
    button('Quote selected output').click()
    expect(onQuote).toHaveBeenCalledWith('Terminal trial · /project\n\n> selected output')
    bridge.hooks!.onData('help\r')
    await vi.waitFor(() => expect(wire).toHaveBeenCalledWith('writeTerminalSession', { request_id: 'request-1', session_id: 'trial-1', data: 'help\r' }))
  })
  it('restores the saved appearance and keeps cosmetic choices available in history', async () => {
    localStorage.setItem(TERMINAL_APPEARANCE_KEY, 'amber')
    open(true, { type: 'terminal', sessions: [{ id: 'saved', cwd: '/project', shell: 'bash', cols: 80, rows: 24,
      status: 'stopped', exit_code: null, output: 'saved output', screen: 'saved output', truncated: false }] })
    await vi.waitFor(() => expect(document.querySelector('[data-terminal-workbench]')?.getAttribute('data-terminal-style')).toBe('amber'))
    button('Terminal style').click()
    await vi.waitFor(() => expect(button('Forest')).toBeDefined())
    button('Forest').click()
    await vi.waitFor(() => expect(document.querySelector('[data-terminal-workbench]')?.getAttribute('data-terminal-style')).toBe('forest'))
    expect(document.querySelector('[data-terminal-history] pre')?.textContent).toBe('saved output')
    expect(bridge.factory).not.toHaveBeenCalled()
    expect(wire).not.toHaveBeenCalled()
    await unmount(view!); view = undefined; runtime.dispose()
    open()
    await vi.waitFor(() => expect(bridge.setAppearance).toHaveBeenLastCalledWith(terminalAppearanceFor('forest').theme))
  })
  it('keeps saved output and allows local submission after the backend permanently loses the session', async () => {
    const saved: TerminalTrialSession = { id: 'saved-trial', cwd: '/project', shell: 'bash', cols: 80, rows: 24,
      status: 'running', exit_code: null, output: '$ previous output', screen: '$ previous output', truncated: false }
    wire.mockRejectedValue({ code: 'INVALID_ARGUMENT', message: 'The terminal session was not found for this request.', retryable: false })
    open(false, { type: 'terminal', sessions: [saved] })
    await vi.waitFor(() => expect(document.querySelector('[role="alert"]')?.textContent).toContain('Saved output is preserved'))
    expect(state.sessions[0]).toEqual({ ...saved, status: 'stopped', exit_code: null })
    await runtime.prepareSubmission()
    expect(wire).toHaveBeenCalledTimes(2)
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
  it('starts on user action and accepts free terminal input without a command shortcut area', async () => {
    open()
    await vi.waitFor(() => expect(bridge.hooks).not.toBeNull())
    expect(wire).not.toHaveBeenCalled()
    expect(document.querySelector('[data-terminal-suggestions]')).toBeNull()
    await start()
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
    await runtime.prepareSubmission()
    expect(wire).toHaveBeenCalledWith('stopTerminalSession', { request_id: 'request-1', session_id: 'trial-1' })
    expect(state.sessions[0]).toMatchObject({ status: 'stopped', output: '$ cli\r\nfinal output', screen: '$ cli\nfinal output' })
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
  it('restarts a stopped terminal only on user action, retaining independent trial output', async () => {
    let number = 0
    wire.mockImplementation(async (name: string, input: { session_id?: string }) => {
      if (name === 'openTerminalSession') number += 1
      const id = input.session_id ?? `trial-${number}`
      return snapshot({ session_id: id, status: name === 'stopTerminalSession' ? 'stopped' : 'running',
        output: name === 'stopTerminalSession' ? `final ${id}` : `prompt ${id}`, next_sequence: 13 })
    })
    open(); await start()
    button('Stop terminal').click()
    await vi.waitFor(() => expect(button('Restart terminal')?.disabled).toBe(false))
    expect(number).toBe(1)
    button('Restart terminal').click()
    await vi.waitFor(() => expect(state.sessions).toHaveLength(2))
    await vi.waitFor(() => expect(bridge.interactive).toBe(true))
    expect(state.sessions[0]).toMatchObject({ id: 'trial-1', status: 'stopped', output: 'final trial-1', screen: 'final trial-1' })
    expect(state.sessions[1]).toMatchObject({ id: 'trial-2', output: 'prompt trial-2', screen: 'prompt trial-2' })
    expect(bridge.screen).toBe('prompt trial-2')
    await runtime.prepareSubmission()
    expect(state.sessions[1].output).toBe('final trial-2')
    expect(wire.mock.calls.filter(([name]) => name === 'openTerminalSession')).toHaveLength(2)
  })
  it('blocks the seventeenth trial before calling the runtime', async () => {
    const sessions: TerminalTrialSession[] = Array.from({ length: 16 }, (_, number) => ({ id: `trial-${number}`,
      cwd: '/project', shell: 'bash', cols: 80, rows: 24, status: 'stopped', exit_code: null, output: '', screen: '', truncated: false }))
    open(false, { type: 'terminal', sessions })
    await vi.waitFor(() => expect(button('Restart terminal')).toBeDefined())
    expect(button('Restart terminal').disabled).toBe(true)
    button('Restart terminal').dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await runtime.start(80,24)
    expect(wire).not.toHaveBeenCalled()
    expect(document.body.textContent).toContain('16 trial limit')
  })
})
