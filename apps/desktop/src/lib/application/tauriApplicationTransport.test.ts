import { readdir, readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }))

import type {
  ApplicationCommandInput,
  ApplicationCommandName,
} from './contracts'
import { defineApplicationStream } from './applicationTransport'
import {
  TAURI_APPLICATION_COMMANDS,
  TauriApplicationTransport,
} from './tauriApplicationTransport'

const commandInputs = {
  listFeedbackInbox: undefined,
  listHostSessions: undefined,
  listArchivedHostSessions: { search: null },
  listHostProfiles: undefined,
  listFeedbackRequests: {
    host_id: null,
    host_session_id: null,
    status: null,
    archived: null,
    search: null,
    limit: null,
    cursor: null,
  },
  getFeedbackWorkspace: { request_id: 'request-1' },
  readPublishedFeedback: { request_id: 'request-1' },
  saveFeedbackDraft: {
    request_id: 'request-1',
    document_json: '{}',
    body_markdown: '',
    expected_revision: 1,
  },
  addFeedbackAttachment: {
    request_id: 'request-1',
    file_name: 'note.txt',
    contents: [1, 2, 3],
    expected_revision: 1,
  },
  removeFeedbackAttachment: {
    request_id: 'request-1',
    attachment_id: 'attachment-1',
    expected_revision: 1,
  },
  reorderFeedbackAttachments: {
    request_id: 'request-1',
    attachment_ids: ['attachment-1'],
    expected_revision: 1,
  },
  submitFeedback: { request_id: 'request-1', expected_revision: 1 },
  approveFeedbackRequest: { request_id: 'request-1' },
  cancelFeedbackRequest: { request_id: 'request-1', reason: 'cancelled' },
  renameHostSession: {
    host_id: 'codex',
    host_session_id: 'session-1',
    title: 'Renamed',
  },
  setHostSessionPinned: {
    host_id: 'codex',
    host_session_id: 'session-1',
    pinned: true,
  },
  archiveHostSession: { host_id: 'codex', host_session_id: 'session-1' },
  unarchiveHostSession: { host_id: 'codex', host_session_id: 'session-1' },
  deleteHostSession: { host_id: 'codex', host_session_id: 'session-1' },
  deleteFeedbackRequest: { request_id: 'request-1' },
  setHostPinned: { host_id: 'codex', pinned: true },
  readFeedbackAttachment: { request_id: 'request-1', attachment_id: 'attachment-1' },
  readRequestAttachment: { request_id: 'request-1', attachment_id: 'attachment-1' },
} satisfies { [Name in ApplicationCommandName]: ApplicationCommandInput<Name> }

function expectedArguments(name: ApplicationCommandName): Record<string, unknown> | undefined {
  const input = commandInputs[name]
  if (name === 'listFeedbackInbox' || name === 'listHostSessions' || name === 'listHostProfiles') {
    return undefined
  }
  return { input }
}

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

describe('TauriApplicationTransport', () => {
  beforeEach(() => {
    mocks.invoke.mockReset()
    mocks.listen.mockReset()
    mocks.invoke.mockResolvedValue(undefined)
  })

  it('maps every semantic command to its complete Tauri command and payload shape', async () => {
    const transport = new TauriApplicationTransport()
    const names = Object.keys(TAURI_APPLICATION_COMMANDS) as ApplicationCommandName[]

    for (const name of names) {
      await transport.call(name, commandInputs[name])
      const args = expectedArguments(name)
      expect(mocks.invoke).toHaveBeenLastCalledWith(
        TAURI_APPLICATION_COMMANDS[name],
        ...(args === undefined ? [] : [args]),
      )
    }
    expect(mocks.invoke).toHaveBeenCalledTimes(names.length)
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
