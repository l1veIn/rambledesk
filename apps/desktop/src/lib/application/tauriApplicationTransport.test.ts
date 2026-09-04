import { readdir, readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }))

import type { ApplicationCommandInput, ApplicationCommandName } from './contracts'
import { defineApplicationStream } from './applicationTransport'
import { APPLICATION_EVENTS_STREAM } from './applicationEvents'
import { createManagedSessionController } from '../agents/managedSessionController'
import type { ApplicationEvent, ManagedSessionSnapshot } from '../generated/feedback'
import {
  APPLICATION_CONFORMANCE_INPUTS,
  applicationConformanceResult,
  runApplicationTransportConformance,
} from './applicationTransportConformance'
import {
  TAURI_APPLICATION_COMMANDS,
  TauriApplicationTransport,
} from './tauriApplicationTransport'

function expectedArguments(name: ApplicationCommandName): Record<string, unknown> | undefined {
  const input = APPLICATION_CONFORMANCE_INPUTS[name]
  if (name === 'listAgentConfigs' || name === 'listFeedbackInbox' || name === 'listHostSessions' || name === 'listHostProfiles') {
    return undefined
  }
  if (name === 'addFeedbackAttachment') {
    return { input: { ...input, contents: [1, 2, 3] } }
  }
  return { input }
}

runApplicationTransportConformance('Tauri', () => {
  mocks.invoke.mockReset()
  mocks.listen.mockReset()
  mocks.listen.mockResolvedValue(vi.fn())
  let rejection: unknown
  const semanticNameByCommand = new Map<string, ApplicationCommandName>(
    Object.entries(TAURI_APPLICATION_COMMANDS).map(([name, command]) => [
      command,
      name as ApplicationCommandName,
    ]),
  )
  mocks.invoke.mockImplementation((command: string) => {
    if (rejection !== undefined) {
      const cause = rejection
      rejection = undefined
      return Promise.reject(cause)
    }
    const name = semanticNameByCommand.get(command)
    if (!name) return Promise.reject(new Error(`Unexpected command: ${command}`))
    return Promise.resolve(applicationConformanceResult(name))
  })

  return {
    transport: new TauriApplicationTransport(),
    expectWireCall: (index, name) => {
      const args = expectedArguments(name)
      expect(mocks.invoke).toHaveBeenNthCalledWith(
        index + 1,
        TAURI_APPLICATION_COMMANDS[name],
        ...(args === undefined ? [] : [args]),
      )
    },
    rejectNext: (error) => {
      rejection = error
    },
  }
})

async function sourceFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true })
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const target = path.join(directory, entry.name)
      if (entry.isDirectory()) return sourceFiles(target)
      return /\.(svelte|ts)$/.test(entry.name) ? [target] : []
    }),
  )
  return nested.flat()
}

function managedSnapshot(sessionId: string, sequence: number): ManagedSessionSnapshot {
  return {
    session: { session_id: sessionId, host_id: 'dsh', host_session_id: `host-${sessionId}`, title: sessionId,
      management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: `remote-${sessionId}` },
      created_at: '2026-09-04', updated_at: '2026-09-04' },
    runtime: { connection: 'connected', activity: sequence === 36 ? 'idle' : 'running', instance_id: `instance-${sessionId}`, config_updated_at: null,
      capabilities: { load_session: true, resume_session: false, http_mcp: true }, last_error: null },
    activities: Array.from({ length: sequence }, (_, index) => ({ id: `message-${index}`, session_id: sessionId, sequence: index + 1, turn_id: 'turn',
      kind: 'agent_message', text: `Chunk ${index + 1}`, tool_call_id: null, created_at: '2026-09-04' })),
    permissions: [], deliveries: [], deleting: false, recovery: null,
  }
}

async function flushNativeUpdates() { for (let i = 0; i < 20; i += 1) await Promise.resolve() }

describe('TauriApplicationTransport', () => {
  beforeEach(() => {
    mocks.invoke.mockReset()
    mocks.listen.mockReset()
    mocks.invoke.mockResolvedValue(undefined)
  })

  it('returns a synchronous idempotent unsubscribe before async listen resolves', async () => {
    let resolveListen: ((unlisten: () => void) => void) | undefined
    const nativeUnlisten = vi.fn()
    mocks.listen.mockReturnValue(
      new Promise<() => void>((resolve) => {
        resolveListen = resolve
      }),
    )
    const stream = defineApplicationStream<{ requestId: string }>('test:event')
    const transport = new TauriApplicationTransport()
    const unsubscribe = transport.subscribe(stream, vi.fn(), vi.fn())

    unsubscribe()
    unsubscribe()
    resolveListen?.(nativeUnlisten)
    await Promise.resolve()

    expect(nativeUnlisten).toHaveBeenCalledTimes(1)
  })

  it('delivers typed Tauri event payloads until unsubscribe', async () => {
    let nativeHandler: ((event: { payload: { requestId: string } }) => void) | undefined
    const nativeUnlisten = vi.fn()
    mocks.listen.mockImplementation(
      async (_stream: string, handler: (event: { payload: { requestId: string } }) => void) => {
        nativeHandler = handler
        return nativeUnlisten
      },
    )
    const stream = defineApplicationStream<{ requestId: string }>('test:event')
    const handler = vi.fn()
    const transport = new TauriApplicationTransport()
    const unsubscribe = transport.subscribe(stream, handler, vi.fn())
    await Promise.resolve()

    nativeHandler?.({ payload: { requestId: 'request-1' } })
    unsubscribe()
    unsubscribe()
    nativeHandler?.({ payload: { requestId: 'request-2' } })

    expect(handler).toHaveBeenCalledTimes(1)
    expect(handler).toHaveBeenCalledWith({ requestId: 'request-1' })
    expect(nativeUnlisten).toHaveBeenCalledTimes(1)
  })

  it('reports async listen registration failures while the subscription is active', async () => {
    const failure = new Error('listen failed')
    mocks.listen.mockRejectedValue(failure)
    const onError = vi.fn()
    const transport = new TauriApplicationTransport()

    transport.subscribe(defineApplicationStream('test:event'), vi.fn(), onError)
    await Promise.resolve()
    await Promise.resolve()

    expect(onError).toHaveBeenCalledWith(failure)
  })

  it('registers the native listener before a managed projection reads its initial snapshot', async () => {
    let register!: (unlisten: () => void) => void
    mocks.listen.mockReturnValue(new Promise<() => void>((resolve) => { register = resolve }))
    let current = managedSnapshot('website', 8)
    mocks.invoke.mockImplementation(async () => current)
    const controller = createManagedSessionController(new TauriApplicationTransport(), 'website')
    controller.start()
    await flushNativeUpdates()
    expect(mocks.invoke).not.toHaveBeenCalled()

    // This change precedes native registration and cannot deliver an event.
    // The first read must include it even if no further chunks arrive.
    current = managedSnapshot('website', 36)
    const unlisten = vi.fn()
    register(unlisten)
    await flushNativeUpdates()
    expect(get(controller).snapshot?.activities).toHaveLength(36)
    expect(get(controller).snapshot?.runtime.activity).toBe('idle')
    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    controller.dispose()
    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  it('keeps a failed native registration visible until that subscription is disposed', async () => {
    mocks.listen.mockRejectedValue(new Error('Native events unavailable'))
    const transport = new TauriApplicationTransport()
    const controller = createManagedSessionController(transport, 'website')
    controller.start()
    await flushNativeUpdates()
    expect(get(controller).error).toBe('Native events unavailable')
    expect(mocks.invoke).not.toHaveBeenCalled()
    await expect(transport.waitUntilReady()).rejects.toThrow('Native events unavailable')
    controller.dispose()
    await expect(transport.waitUntilReady()).resolves.toBeUndefined()
  })

  it('streams native invalidations into mounted managed sessions through the final turn without tab switching', async () => {
    const listeners = new Set<(event: { payload: ApplicationEvent }) => void>()
    const unlisten = vi.fn()
    mocks.listen.mockImplementation(async (stream: string, handler: (event: { payload: ApplicationEvent }) => void) => {
      expect(stream).toBe(APPLICATION_EVENTS_STREAM.id)
      listeners.add(handler)
      return () => { listeners.delete(handler); unlisten() }
    })
    let current = managedSnapshot('website', 0)
    let finishPrompt!: (snapshot: ManagedSessionSnapshot) => void
    mocks.invoke.mockImplementation((command: string, args: { input: { session_id: string } }) => {
      if (command === TAURI_APPLICATION_COMMANDS.sendManagedPrompt) {
        return new Promise<ManagedSessionSnapshot>((resolve) => { finishPrompt = resolve })
      }
      expect(command).toBe(TAURI_APPLICATION_COMMANDS.getManagedSession)
      return Promise.resolve(args.input.session_id === 'website' ? current : managedSnapshot('cli', 0))
    })
    const transport = new TauriApplicationTransport()
    const website = createManagedSessionController(transport, 'website')
    const cli = createManagedSessionController(transport, 'cli')
    website.start(); cli.start()
    await flushNativeUpdates()
    const sending = website.prompt('Stream the complete turn')
    for (let sequence = 1; sequence <= 36; sequence += 1) {
      current = managedSnapshot('website', sequence)
      for (const handler of listeners) handler({ payload: {
        type: 'invalidate', runtime_generation: 'desktop-runtime', revision: String(sequence),
        resources: [{ kind: 'managed_session', session_id: 'website' }],
      } })
      await flushNativeUpdates()
      expect(get(website).snapshot?.activities).toHaveLength(sequence)
      expect(get(cli).snapshot?.activities).toHaveLength(0)
    }
    expect(get(website).snapshot?.runtime.activity).toBe('idle')
    finishPrompt(managedSnapshot('website', 8))
    await sending
    await flushNativeUpdates()
    expect(get(website).snapshot?.activities).toHaveLength(36)
    expect(mocks.invoke.mock.calls.filter(([command]) => command === TAURI_APPLICATION_COMMANDS.sendManagedPrompt)).toHaveLength(1)
    expect(mocks.invoke.mock.calls.filter(([, args]) => args?.input.session_id === 'cli')).toHaveLength(1)

    website.dispose(); cli.dispose()
    expect(listeners.size).toBe(0)
    expect(unlisten).toHaveBeenCalledTimes(2)
  })

  it('keeps mapped commands and the Tauri implementation import localized', async () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
    const implementationSource = fileURLToPath(new URL('./tauriApplicationTransport.ts', import.meta.url))
    const implementationTest = fileURLToPath(new URL('./tauriApplicationTransport.test.ts', import.meta.url))
    const compositionRoot = fileURLToPath(new URL('../../main.ts', import.meta.url))
    const allowedImplementationReferences = new Set([
      implementationSource,
      implementationTest,
      compositionRoot,
    ])
    const violations: string[] = []

    for (const file of await sourceFiles(sourceRoot)) {
      const source = await readFile(file, 'utf8')
      if (file !== implementationSource) {
        for (const command of Object.values(TAURI_APPLICATION_COMMANDS)) {
          const quoted = new RegExp("(['\\\"`])" + command + '\\1')
          if (quoted.test(source)) violations.push(`${path.relative(sourceRoot, file)}: ${command}`)
        }
      }
      if (
        source.includes('tauriApplicationTransport') &&
        !allowedImplementationReferences.has(file)
      ) {
        violations.push(`${path.relative(sourceRoot, file)}: Tauri implementation reference`)
      }
    }

    expect(violations).toEqual([])
  })
})
