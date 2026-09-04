import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { AgentCatalogEntry, AgentConfig, AgentInspection, AgentInstallJob } from '$lib/generated/feedback'
import { catalogConfiguration, configurationsForAgent, createAgentCatalogController } from './agentCatalogController'
import { AGENT_SETUP, applyAgentCredentials } from './agentOnboarding'

const entry: AgentCatalogEntry = {
  id: 'deepseek-acp', name: 'DeepSeek ACP', host_id: 'dsh', description: '', connection_kind: 'bridge',
  distribution: { kind: 'npm', package: 'deepseek-acp', pinned_version: '0.8.0', command: 'deepseek-acp', node_required: '22.0.0' },
  args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: '' },
}
const inspection: AgentInspection = { agent_id: entry.id, source: 'managed', version: '0.8.0', command: 'C:/node.exe', args: ['C:/agents/new/node_modules/deepseek-acp/index.js'], dependencies: [], checks: [] }
const job: AgentInstallJob = { id: 'job', agent_id: entry.id, phase: 'installing', messages: [], result: null, cancel_requested: false }
async function flush() { for (let index = 0; index < 12; index++) await Promise.resolve() }
function harness(jobs: AgentInstallJob[] = []) {
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .resolve('listAvailableAgents', [entry]).resolve('listAgentInstallJobs', jobs)
    .resolve('inspectAgentInstallation', inspection)
  const controller = createAgentCatalogController(transport)
  return { transport, controller }
}
afterEach(() => vi.useRealTimers())

describe('Agent catalog application integration', () => {
  it('maps credentials to each agent without writing placeholders or erasing existing keys', () => {
    const previous = { DEEPSEEK_API_KEY: 'saved', CUSTOM: 'retain' }
    expect(applyAgentCredentials(previous, AGENT_SETUP['deepseek-acp'], { key: '', baseUrl: ' https://endpoint.test ', model: 'selected' }))
      .toEqual({ ...previous, DEEPSEEK_BASE_URL: 'https://endpoint.test', DEEPSEEK_ACP_MODEL: 'selected' })
    expect(previous).toEqual({ DEEPSEEK_API_KEY: 'saved', CUSTOM: 'retain' })
    expect(applyAgentCredentials({}, AGENT_SETUP.qoder, { key: 'token', baseUrl: 'https://ignored.test', model: 'auto' }))
      .toEqual({ QODER_PERSONAL_ACCESS_TOKEN: 'token', QODER_MODEL: 'auto' })
    expect(applyAgentCredentials({}, AGENT_SETUP.kimi, { key: 'key', baseUrl: '', model: '' }))
      .toEqual({ KIMI_MODEL_API_KEY: 'key' })
  })
  it('resumes a server-owned job after opening settings and inspects only once when it completes', async () => {
    vi.useFakeTimers()
    const { transport, controller } = harness([job])
    const dispose = controller.start()
    await flush()
    expect(get(controller).jobs[0].phase).toBe('installing')
    transport.resolve('listAgentInstallJobs', [{ ...job, phase: 'complete' }])
    await vi.advanceTimersByTimeAsync(500)
    expect(get(controller).jobs[0].phase).toBe('complete')
    expect(get(controller).inspections[entry.id]).toEqual(inspection)
    await vi.advanceTimersByTimeAsync(1000)
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(1)
    expect(transport.callsFor('listAgentInstallJobs')).toHaveLength(2)
    dispose()
  })

  it('leaving settings stops UI polling without cancelling an owned installation', async () => {
    vi.useFakeTimers()
    const { transport, controller } = harness([job])
    const dispose = controller.start()
    await flush()
    dispose()
    await vi.advanceTimersByTimeAsync(2000)
    await controller.install(entry.id)
    await controller.cancel(job.id)
    expect(transport.callsFor('listAgentInstallJobs')).toHaveLength(1)
    expect(transport.callsFor('cancelAgentInstall')).toHaveLength(0)
    expect(transport.callsFor('installAgent')).toHaveLength(0)
  })

  it('deduplicates version probes and ignores a response after the page closes', async () => {
    const { transport, controller } = harness()
    let finish!: (value: AgentInspection) => void
    transport.handle('inspectAgentInstallation', () => new Promise(resolve => { finish = resolve }))
    controller.start()
    await flush()
    const first = controller.inspect(entry.id)
    const second = controller.inspect(entry.id)
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(1)
    controller.dispose()
    finish(inspection)
    await Promise.all([first, second])
    expect(get(controller).inspections).toEqual({})
  })

  it('retains cancellation progress until the backend reports terminal cleanup', async () => {
    vi.useFakeTimers()
    const { transport, controller } = harness([job])
    controller.start()
    await flush()
    transport.resolve('cancelAgentInstall', undefined).resolve('listAgentInstallJobs', [{ ...job, cancel_requested: true }])
    await controller.cancel(job.id)
    expect(get(controller).jobs[0]).toMatchObject({ phase: 'installing', cancel_requested: true })
    transport.resolve('listAgentInstallJobs', [{ ...job, phase: 'cancelled', cancel_requested: true }])
    await vi.advanceTimersByTimeAsync(500)
    expect(get(controller).jobs[0].phase).toBe('cancelled')
    controller.dispose()
  })

  it('matches the agent package across generations without confusing agents sharing node and a host', () => {
    const config: AgentConfig = { ...catalogConfiguration(entry, inspection), id: 'old', command: 'C:/node.exe', args: ['C:\\agents\\old\\node_modules\\deepseek-acp\\index.js'], created_at: '', updated_at: '' }
    const other = { ...config, id: 'dsh', args: ['C:/agents/dsh/node_modules/@deepseek-ai/dsh/main.js', '--profile', 'acp'] }
    expect(configurationsForAgent(entry, [config, other], inspection)).toEqual([config])
    expect(() => catalogConfiguration(entry, { ...inspection, checks: [{ id: 'node', status: 'fail', message: 'Missing' }] })).toThrow('failed checks')
    expect(() => catalogConfiguration(entry, { ...inspection, command: null })).toThrow('Install')
  })
})
