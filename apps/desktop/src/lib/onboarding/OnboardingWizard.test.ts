// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { AgentCatalogEntry, AgentConfig, AgentInspection } from '$lib/generated/feedback'
import { locale, onboardingCompleted, setCookingEnabled, setOnboardingStep } from '$lib/preferences'
import OnboardingWizard from './OnboardingWizard.svelte'

const deepseek: AgentCatalogEntry = {
  id: 'deepseek-acp', name: 'DeepSeek', host_id: 'dsh', description: '', connection_kind: 'bridge',
  distribution: { kind: 'npm', package: 'deepseek-acp', pinned_version: '0.8.0', command: 'deepseek-acp', node_required: '22.0.0' },
  args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: '' },
}
const found: AgentInspection = { agent_id: deepseek.id, source: 'managed', version: '0.8.0', command: 'C:/node.exe',
  args: ['C:/isolated/agents/deepseek-acp/index.js'], dependencies: [], checks: [] }
const missing: AgentInspection = { ...found, source: 'missing', version: null, command: null, args: [],
  checks: [{ id: 'entry', status: 'fail', message: 'ACP component not installed' }] }
let view: ReturnType<typeof mount> | undefined
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]')
const button = (label: string) => [...(dialog()?.querySelectorAll<HTMLButtonElement>('button') ?? [])]
  .find(element => element.textContent?.trim() === label)!

beforeEach(() => {
  localStorage.clear()
  locale.set('en')
  onboardingCompleted.set(false)
  setOnboardingStep(0)
  setCookingEnabled(false)
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  Element.prototype.getAnimations = (() => []) as never
  HTMLElement.prototype.scrollIntoView = vi.fn()
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

function harness(entry = deepseek, initial = missing) {
  let configs: AgentConfig[] = []
  let inspection = { ...initial, agent_id: entry.id }
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .resolve('listAvailableAgents', [entry]).resolve('listAgentInstallJobs', [])
    .handle('listAgentConfigs', () => configs)
    .handle('inspectAgentInstallation', () => inspection)
    // Match the real service: a newly discovered launch is enabled; existing
    // disabled profiles are handled separately by the catalog controller.
    .handle('resolveCatalogAgent', () => {
      const saved: AgentConfig = { id: 'isolated-agent', catalog_id: entry.id, name: entry.name, host_id: entry.host_id,
        protocol: 'acp', enabled: true, command: inspection.command!, args: inspection.args, env: {}, created_at: '', updated_at: '' }
      configs = [saved]
      return saved
    })
    .handle('saveAgentConfig', input => {
      const saved = { ...input, id: input.id ?? 'isolated-agent', created_at: '', updated_at: '' }
      configs = [saved]
      return saved
    })
    .handle('installAgent', () => {
      inspection = { ...found, agent_id: entry.id }
      return { id: 'isolated-install', agent_id: entry.id, phase: 'complete', messages: [], result: null, cancel_requested: false }
    })
    .resolve('checkAgentConfig', { ok: true, message: 'ACP connected', details: [] })
  const onStartSession = vi.fn(async (_configId?: string) => {})
  view = mount(OnboardingWizard, { target: document.body, props: { transport, openWizard: true, onStartSession } })
  return { transport, onStartSession }
}
async function enterAgents() {
  await vi.waitFor(() => expect(button('Continue')).toBeDefined())
  button('Continue').click()
  await vi.waitFor(() => expect(dialog()?.textContent).toContain('ACP connection'))
}
async function finish() {
  button('Continue').click()
  await tick()
  expect(dialog()?.textContent).toContain('Enable Feedback Cooking?')
  button('Continue').click()
  await tick()
  button('New session').click()
}

describe('isolated first-run onboarding', () => {
  it('lets a new user continue when no agent is installed', async () => {
    const { transport, onStartSession } = harness()
    await enterAgents()
    await vi.waitFor(() => expect(button('Connect in one click')?.disabled).toBe(false))
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(0)
    expect(button('Connect later').disabled).toBe(false)
    button('Connect later').click()
    await tick()
    button('Continue').click()
    await tick()
    button('New session').click()
    await vi.waitFor(() => expect(onStartSession).toHaveBeenCalledExactlyOnceWith(undefined))
    await vi.waitFor(() => expect(get(onboardingCompleted)).toBe(true))
  })

  it('prepares DeepSeek, retries a failed ACP check, and hands the checked profile to the first session', async () => {
    const { transport, onStartSession } = harness()
    let fail = true
    transport.handle('checkAgentConfig', () => fail
      ? { ok: false, message: 'Temporary ACP connection failure', details: [] }
      : { ok: true, message: 'ACP connected', details: [] })
    await enterAgents()
    await vi.waitFor(() => expect(button('Connect in one click')?.disabled).toBe(false))
    button('Connect in one click').click()
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Connection failed'))
    expect(button('Check connection').disabled).toBe(false)
    expect(button('Connect later').disabled).toBe(false)
    fail = false
    button('Check connection').click()
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('The last ACP connection check passed'))
    await finish()
    await vi.waitFor(() => expect(onStartSession).toHaveBeenCalledExactlyOnceWith('isolated-agent'))
    expect(transport.callsFor('installAgent')).toHaveLength(1)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(2)
    await vi.waitFor(() => expect(get(onboardingCompleted)).toBe(true))
  })

  it('checks a newly discovered native ACP agent during the initial scan', async () => {
    const native: AgentCatalogEntry = { ...deepseek, id: 'native-test', name: 'Native test agent', host_id: 'native-test',
      connection_kind: 'native', distribution: { kind: 'manual', command: 'native-test', version: '1', instructions: '', docs_url: '' } }
    const { transport, onStartSession } = harness(native, { ...found, source: 'system' })
    await enterAgents()
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('The last ACP connection check passed'))
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    await finish()
    await vi.waitFor(() => expect(onStartSession).toHaveBeenCalledExactlyOnceWith('isolated-agent'))
  })

  it('keeps the wizard recoverable if opening the first session fails', async () => {
    const { onStartSession } = harness()
    onStartSession.mockRejectedValueOnce(new Error('Could not open a new session'))
    await vi.waitFor(() => expect(button('Set up later')).toBeDefined())
    button('Set up later').click()
    await vi.waitFor(() => expect(dialog()?.querySelector('[role="alert"]')?.textContent).toContain('Could not open a new session'))
    expect(get(onboardingCompleted)).toBe(false)
    expect(button('Set up later').disabled).toBe(false)
    button('Set up later').click()
    await vi.waitFor(() => expect(onStartSession).toHaveBeenCalledTimes(2))
    await vi.waitFor(() => expect(get(onboardingCompleted)).toBe(true))
  })
})
