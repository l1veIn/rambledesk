// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import App from './App.svelte'
import { PreviewApplicationTransport } from './lib/preview/previewApplicationTransport'
import type { ApplicationCommandInput, ApplicationCommandName, ApplicationCommandResult } from './lib/application/contracts'
import type { TerminalSessionSnapshot } from './lib/generated/feedback'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from './lib/capabilities/unavailableCapabilities'
import { autoOpenTaskBrief, cookingEnabled, locale, onboardingCompleted } from './lib/preferences'
import { previewFixtures } from './lib/preview/previewFixtures'
import { resetPreviewWorkspaceSnapshot, savePreviewWorkspaceSnapshot } from './lib/workspace/previewWorkspaceSnapshot'
import { readWorkbenchState } from './lib/workbenchState'
import { requestTaskViewDescriptor, sessionViewDescriptor, workbenchReviewViewDescriptor, workspaceViewKey } from './lib/workspace/viewDescriptors'
import { createSimulatedSpeech } from './dev/simulatedSpeech'
import type { WorkbenchCapabilities } from './lib/capabilities/workbenchCapabilities'
import type { RambleConsoleCommand, RambleConsoleState } from './lib/rambleConsole'

const terminal = vi.hoisted(() => ({
  onData: undefined as ((input: string) => void) | undefined,
  onSelection: undefined as ((selection: string) => void) | undefined,
  create: vi.fn(), dispose: vi.fn(),
}))
vi.mock('./lib/workbench/terminal/xtermAdapter', () => ({
  createXtermAdapter: async (root: HTMLElement, events: typeof terminal) => {
    terminal.create(); terminal.onData = events.onData; terminal.onSelection = events.onSelection
    const surface = document.createElement('div'); surface.dataset.testTerminal = ''; root.append(surface)
    let screen = ''
    return { reset: () => { screen = '' }, write: async (text: string) => { screen += text; surface.textContent = screen },
      screen: () => screen, size: () => ({ cols: 80, rows: 24 }), fit() {}, focus() {},
      paste: (text: string) => terminal.onData?.(text), setInteractive() {},
      dispose: () => { terminal.dispose(); surface.remove() } }
  },
}))

const request = previewFixtures.requests[0]
const sessionKey = workspaceViewKey(sessionViewDescriptor(request.host_id, request.host_session_id))
const reviewKey = workspaceViewKey(workbenchReviewViewDescriptor(request.request_id))
const bodySelector = '[contenteditable="true"][aria-label="Markdown rich-text feedback body"]'
let app: ReturnType<typeof mount> | undefined
let previousUrl = ''
const animations = Object.getOwnPropertyDescriptor(Element.prototype, 'getAnimations')
beforeEach(() => {
  localStorage.clear(); resetPreviewWorkspaceSnapshot(); terminal.create.mockClear(); terminal.dispose.mockClear()
  previousUrl = location.href; history.replaceState(null, '', '?preview=fixtures&workspace=terminal')
  locale.set('en'); onboardingCompleted.set(true); autoOpenTaskBrief.set(false); cookingEnabled.set(false)
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', (query: string) => ({ matches: false, media: query,
    addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} }))
  Object.defineProperty(Element.prototype, 'getAnimations', { configurable: true, value: () => [] })
})
afterEach(async () => {
  if (app) await unmount(app)
  app = undefined; document.body.replaceChildren(); history.replaceState(null, '', previousUrl)
  resetPreviewWorkspaceSnapshot(); vi.restoreAllMocks(); vi.unstubAllGlobals()
  if (animations) Object.defineProperty(Element.prototype, 'getAnimations', animations)
  else Reflect.deleteProperty(Element.prototype, 'getAnimations')
})
function button(label: string) {
  const found = [...document.querySelectorAll<HTMLButtonElement>('button')]
    .find((item) => item.textContent?.trim() === label || item.getAttribute('aria-label') === label)
  expect(found, label).toBeDefined(); return found!
}
const activeKey = () => document.querySelector('[data-workspace-tab-item][data-active="true"]')?.getAttribute('data-workspace-view-key')
function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((accept) => resolve = accept)
  return { promise, resolve }
}
class TerminalTransport extends PreviewApplicationTransport {
  names: string[] = []
  opening: ReturnType<typeof deferred> | null = null
  stopping: ReturnType<typeof deferred> | null = null
  failStop = false
  missingStop = false
  constructor() { super(UNAVAILABLE_CAPABILITY_MANIFEST, { workspace: 'terminal' }) }
  override async call<Name extends ApplicationCommandName>(name: Name, input: ApplicationCommandInput<Name>): Promise<ApplicationCommandResult<Name>> {
    this.names.push(name)
    if (name === 'openTerminalSession' && this.opening) await this.opening.promise
    if (name === 'stopTerminalSession') {
      if (this.failStop) { this.failStop = false; throw new Error('Could not stop terminal') }
      if (this.missingStop) throw { code: 'INVALID_ARGUMENT', message: 'The terminal session was not found for this request.', retryable: false }
      if (this.stopping) await this.stopping.promise
      const final = await super.call(name, input) as TerminalSessionSnapshot
      return { ...final, output: final.output + '\r\nFinal output after stop', next_sequence: final.next_sequence + 25 } as ApplicationCommandResult<Name>
    }
    return super.call(name, input)
  }
}
function mountPreview(transport: PreviewApplicationTransport, capabilities?: WorkbenchCapabilities) {
  app = mount(App, { target: document.body, props: { applicationTransport: transport, capabilities,
    environment: 'browser', previewMode: true, publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} } } })
}
async function startWithNotes() {
  await vi.waitFor(() => expect(button('Start terminal').disabled).toBe(false))
  button('Start terminal').click()
  await vi.waitFor(() => expect(button('Stop terminal').disabled).toBe(false))
  terminal.onSelection?.('CLI trial feedback')
  await tick(); button('Quote selected output').click()
  await vi.waitFor(() => expect(button('Submit feedback').disabled).toBe(false))
}
function taskTab() {
  const task = requestTaskViewDescriptor(request.request_id)
  savePreviewWorkspaceSnapshot({ version: 2, views: [{ ...sessionViewDescriptor(request.host_id, request.host_session_id), lastRequestId: request.request_id }, task], activeViewKey: sessionKey })
  return workspaceViewKey(task)
}
function tab(key: string) {
  const item = [...document.querySelectorAll('[data-workspace-tab-item]')]
    .find((item) => item.getAttribute('data-workspace-view-key') === key)
  const control = item?.querySelector<HTMLElement>('[role="tab"]')
  expect(control).not.toBeNull(); return control!
}

it('locks navigation cancellation and repeated submit while final terminal output drains', async () => {
  const taskKey = taskTab(), transport = new TerminalTransport()
  mountPreview(transport); await startWithNotes()
  transport.stopping = deferred()
  button('Submit feedback').click()
  await vi.waitFor(() => expect(transport.names).toContain('stopTerminalSession'))
  await tick()
  expect(tab(taskKey).getAttribute('aria-disabled')).toBe('true')
  expect(button('Cancel feedback').disabled).toBe(true)
  expect(button('Submit feedback').disabled).toBe(true)
  tab(taskKey).click(); button('Cancel feedback').click(); button('Submit feedback').click()
  await tick()
  expect(activeKey()).toBe(sessionKey)
  expect(transport.names.filter((name) => name === 'stopTerminalSession')).toHaveLength(1)
  expect(transport.names).not.toContain('cancelFeedbackRequest')
  transport.stopping.resolve()
  await vi.waitFor(async () => expect((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).request.status).toBe('completed'))
  const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
  const state = readWorkbenchState(saved.draft.document_json)
  expect(state?.type === 'terminal' && state.sessions[0].output).toContain('Final output after stop')
})

it('does not leave or cancel while starting, and saves the new session before navigation unlocks', async () => {
  const taskKey = taskTab(), transport = new TerminalTransport()
  transport.opening = deferred()
  mountPreview(transport)
  await vi.waitFor(() => expect(button('Start terminal').disabled).toBe(false))
  button('Start terminal').click()
  await vi.waitFor(() => expect(transport.names).toContain('openTerminalSession'))
  await tick()
  expect(tab(taskKey).getAttribute('aria-disabled')).toBe('true')
  expect(button('Cancel feedback').disabled).toBe(true)
  tab(taskKey).click(); await tick()
  expect(activeKey()).toBe(sessionKey)
  transport.opening.resolve()
  await vi.waitFor(() => expect(button('Stop terminal').disabled).toBe(false))
  await vi.waitFor(() => expect(tab(taskKey).getAttribute('aria-disabled')).toBe('false'))
  tab(taskKey).click()
  await vi.waitFor(() => expect(activeKey()).toBe(taskKey))
  const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
  expect(readWorkbenchState(saved.draft.document_json)?.type).toBe('terminal')
})

it('captures final evidence when submitting from a task tab without a mounted terminal', async () => {
  const taskKey = taskTab(), transport = new TerminalTransport()
  mountPreview(transport); await startWithNotes()
  tab(taskKey).click()
  await vi.waitFor(() => expect(activeKey()).toBe(taskKey))
  expect(document.querySelector('[data-test-terminal]')).toBeNull()
  button('Submit feedback').click()
  await vi.waitFor(async () => expect((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).request.status).toBe('completed'))
  const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
  const state = readWorkbenchState(saved.draft.document_json)
  expect(state?.type === 'terminal' && state.sessions[0].screen).toContain('Final output after stop')
  expect(transport.names.indexOf('submitFeedback')).toBeGreaterThan(transport.names.indexOf('stopTerminalSession'))
})

it.each(['stop', 'exit'] as const)('retains both trials after %s and restart, then finalizes them from the task tab', async (end) => {
  const taskKey = taskTab(), transport = new TerminalTransport()
  mountPreview(transport); await startWithNotes()
  terminal.onData?.('demo --help\r')
  await vi.waitFor(() => expect(document.querySelector('[data-test-terminal]')?.textContent).toContain('CLI Demo'))
  if (end === 'stop') button('Stop terminal').click()
  else terminal.onData?.('exit\r')
  await vi.waitFor(() => expect(button('Restart terminal').disabled).toBe(false))
  button('Restart terminal').click()
  await vi.waitFor(() => expect(button('Stop terminal').disabled).toBe(false))
  terminal.onData?.('demo error\r')
  await vi.waitFor(() => expect(document.querySelector('[data-test-terminal]')?.textContent).toContain('缺少配置'))
  expect(document.querySelector('[data-test-terminal]')?.textContent).not.toContain('CLI Demo')
  tab(taskKey).click()
  await vi.waitFor(() => expect(activeKey()).toBe(taskKey))
  const before = readWorkbenchState((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).draft.document_json)
  expect(before?.type === 'terminal' && before.sessions.map((item) => item.status)).toEqual([end === 'stop' ? 'stopped' : 'exited', 'running'])
  button('Submit feedback').click()
  await vi.waitFor(async () => expect((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).request.status).toBe('completed'))
  const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
  const state = readWorkbenchState(saved.draft.document_json)
  expect(state?.type).toBe('terminal')
  if (state?.type !== 'terminal') throw new Error('Missing terminal state')
  expect(state.sessions).toHaveLength(2)
  expect(new Set(state.sessions.map((item) => item.id)).size).toBe(2)
  expect(state.sessions.map((item) => item.status)).toEqual([end === 'stop' ? 'stopped' : 'exited', 'stopped'])
  expect(state.sessions[0].exit_code).toBe(end === 'stop' ? null : 0)
  expect(state.sessions[0].output).toContain('CLI Demo')
  expect(state.sessions[1].output).toContain('缺少配置')
  expect(state.sessions.every((item) => item.output.includes('Final output after stop'))).toBe(true)
  expect(saved.draft.body_markdown).toContain('CLI trial feedback')
  expect(transport.names.filter((name) => name === 'openTerminalSession')).toHaveLength(2)
})

it('blocks a failed stop and lets the user explicitly retry submission', async () => {
  const transport = new TerminalTransport()
  mountPreview(transport); await startWithNotes()
  transport.failStop = true
  button('Submit feedback').click()
  await vi.waitFor(() => expect(document.body.textContent).toContain('Could not stop terminal'))
  expect(transport.names).not.toContain('submitFeedback')
  await vi.waitFor(() => expect(button('Submit feedback').disabled).toBe(false))
  button('Submit feedback').click()
  await vi.waitFor(async () => expect((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).request.status).toBe('completed'))
  expect(transport.names.filter((name) => name === 'submitFeedback')).toHaveLength(1)
})

it('submits preserved trial history from a task tab when the backend session was permanently lost', async () => {
  const taskKey = taskTab(), transport = new TerminalTransport()
  mountPreview(transport); await startWithNotes()
  tab(taskKey).click()
  await vi.waitFor(() => expect(activeKey()).toBe(taskKey))
  const before = readWorkbenchState((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).draft.document_json)
  transport.missingStop = true
  button('Submit feedback').click()
  await vi.waitFor(async () => expect((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).request.status).toBe('completed'))
  const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
  const state = readWorkbenchState(saved.draft.document_json)
  expect(state?.type === 'terminal' && state.sessions[0]).toMatchObject({ status: 'stopped', exit_code: null })
  expect(state?.type === 'terminal' && state.sessions[0].output).toBe(before?.type === 'terminal' && before.sessions[0].output)
  expect(saved.draft.body_markdown).toContain('CLI trial feedback')
})

it('captures final evidence from the native Ramble console submit entry point', async () => {
  const transport = new TerminalTransport(), speech = createSimulatedSpeech()
  let command: ((command: RambleConsoleCommand) => void) | undefined
  const publish = vi.fn(async (_state: RambleConsoleState) => {})
  const base = speech.capabilities
  const capabilities: WorkbenchCapabilities = { ...base, rambleConsole: {
    status: { availability: 'available', source: 'native' }, implementation: { ...base.rambleConsole.implementation,
      show: async () => {}, hide: async () => {}, restoreVisibility: async () => {}, publish,
      publishSpeechOverlay: async () => {}, recordDiagnostic: async () => {},
      onCommand: (handler) => { command = handler; return () => {} }, onReady: () => () => {}, onSpeechOverlayReady: () => () => {},
    },
  } }
  mountPreview(transport, capabilities); await startWithNotes()
  const voice = document.querySelector<HTMLButtonElement>('[data-voice-input]')
  expect(voice).not.toBeNull(); voice!.click()
  await vi.waitFor(() => expect(publish.mock.calls.some(([state]) => state.canSubmit)).toBe(true))
  expect(command).toBeDefined()
  command!({ type: 'submit' })
  await vi.waitFor(async () => expect((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).request.status).toBe('completed'))
  const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
  const state = readWorkbenchState(saved.draft.document_json)
  expect(state?.type === 'terminal' && state.sessions[0].output).toContain('Final output after stop')
})

it('keeps the terminal and feedback editor through full tab navigation, then freezes final evidence on submit', async () => {
  const transport = new PreviewApplicationTransport(UNAVAILABLE_CAPABILITY_MANIFEST, { workspace: 'terminal' })
  const stop = vi.spyOn(transport, 'call')
  app = mount(App, { target: document.body, props: { applicationTransport: transport, environment: 'browser', previewMode: true,
    publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} } } })
  await vi.waitFor(() => expect(document.querySelector(bodySelector)).not.toBeNull())
  expect(activeKey()).toBe(sessionKey)
  await vi.waitFor(() => expect(button('Start terminal').disabled).toBe(false))
  button('Start terminal').click()
  await vi.waitFor(() => expect(button('Stop terminal').disabled).toBe(false))
  const surface = document.querySelector('[data-test-terminal]')!
  const editor = document.querySelector<HTMLElement>(bodySelector)!
  const rail = document.querySelector('aside[aria-label="Projects"]')
  expect(rail).not.toBeNull()
  terminal.onData?.('demo --help\r')
  await vi.waitFor(() => expect(surface.textContent).toContain('CLI Demo'))
  terminal.onSelection?.('demo --help\nCLI Demo')
  await tick()
  button('Quote selected output').click()
  await vi.waitFor(() => expect(editor.textContent).toContain('CLI Demo'))
  button('Full screen trial').click()
  await vi.waitFor(() => expect(activeKey()).toBe(reviewKey))
  expect(document.querySelector('[data-test-terminal]')).toBe(surface)
  expect(document.querySelector(bodySelector)).toBe(editor)
  expect(document.querySelector('aside[aria-label="Projects"]')).toBe(rail)
  expect(terminal.create).toHaveBeenCalledTimes(1)
  button('Return to workbench').click()
  await vi.waitFor(() => expect(activeKey()).toBe(sessionKey))
  expect(terminal.dispose).not.toHaveBeenCalled()
  button('Submit feedback').click()
  await vi.waitFor(async () => expect((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).request.status).toBe('completed'))
  const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
  const state = readWorkbenchState(saved.draft.document_json)
  expect(state?.type).toBe('terminal')
  if (state?.type === 'terminal') {
    expect(state.sessions[0].status).toBe('stopped')
    expect(state.sessions[0].output).toContain('CLI Demo')
    expect(state.sessions[0].screen).toContain('CLI Demo')
    expect(state.sessions[0]).not.toHaveProperty('input')
  }
  expect(saved.draft.body_markdown).toContain('CLI Demo')
  const names = stop.mock.calls.map(([name]) => name)
  const stopIndex = names.lastIndexOf('stopTerminalSession')
  const submitIndex = names.indexOf('submitFeedback')
  expect(stopIndex).toBeGreaterThan(-1)
  expect(submitIndex).toBeGreaterThan(stopIndex)
  expect(names.slice(stopIndex, submitIndex)).toContain('saveFeedbackDraft')
})
