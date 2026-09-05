import { describe, expect, it, vi } from 'vitest'

import { defineApplicationStream, type ApplicationTransport } from './applicationTransport'
import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from './contracts'

export const APPLICATION_CONFORMANCE_INPUTS = {
  setManagedSessionConfig: { session_id: 'local-session-1', change: { type: 'mode', mode_id: 'ask' } },
  sendManagedPromptContent: { session_id: 'local-session-1', text: 'Read this', content: [{ type: 'resource_link', uri: 'file:///project/main.ts', name: 'main.ts', mime_type: 'text/typescript' }] },
  listManagedSessionActivity: { session_id: 'local-session-1', before_sequence: 100, limit: 50 },
  listAvailableAgents: undefined,
  inspectAgentInstallation: { agent_id: 'deepseek' },
  resolveCatalogAgent: { agent_id: 'deepseek', enable: false },
  listAgentInstallJobs: undefined,
  installAgent: { agent_id: 'deepseek', version: null },
  cancelAgentInstall: { job_id: 'job-1' },
  listAgentConfigs: undefined,
  saveAgentConfig: {
    id: null, name: 'DeepSeek', host_id: 'dsh', protocol: 'acp', enabled: true,
    command: 'deepseek-acp', args: ['--label', 'project with spaces'], env: { TEST_ENV: 'test-value' },
  },
  deleteAgentConfig: { agent_config_id: 'config-1' },
  checkAgentConfig: { agent_config_id: 'config-1' },
  createManagedSession: { agent_config_id: 'config-1', cwd: '/project', title: 'Project' },
  prepareManagedSession: { agent_config_id: 'config-1', cwd: '/project' },
  discardPreparedSession: { session_id: 'local-session-1' },
  getManagedSession: { session_id: 'local-session-1' },
  getManagedFeedbackStatus: { session_id: 'local-session-1' },
  getManagedWorkspaceInfo: { session_id: 'local-session-1' },
  startManagedSession: { session_id: 'local-session-1' },
  stopManagedSession: { session_id: 'local-session-1' },
  cancelManagedPrompt: { session_id: 'local-session-1' },
  sendManagedPrompt: { session_id: 'local-session-1', text: 'Review this project.' },
  respondManagedPermission: { session_id: 'local-session-1', request_id: 'permission-1', option_id: null },
  resolveFeedbackDelivery: { session_id: 'local-session-1', request_id: 'request-1', action: 'acknowledge' },
  deleteManagedSession: { session_id: 'local-session-1' },
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
    body_markdown: 'draft',
    expected_revision: 1,
  },
  addFeedbackAttachment: {
    request_id: 'request-1',
    file_name: 'note.txt',
    contents: new Uint8Array([1, 2, 3]).buffer,
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

export const APPLICATION_COMMAND_NAMES = Object.freeze(
  Object.keys(APPLICATION_CONFORMANCE_INPUTS) as ApplicationCommandName[],
)

const SAFE_TERMINAL_PROJECTION = Object.freeze({
  request_id: 'request-1',
  status: 'completed',
  feedback: { available: true },
})

export function applicationConformanceResult<Name extends ApplicationCommandName>(
  name: Name,
): ApplicationCommandResult<Name> {
  let result: unknown
  if (name === 'readFeedbackAttachment' || name === 'readRequestAttachment') {
    result = new Uint8Array([4, 5, 6]).buffer
  } else if (name === 'discardPreparedSession' || name === 'cancelAgentInstall' || name === 'deleteHostSession' || name === 'deleteFeedbackRequest' || name === 'deleteAgentConfig' || name === 'deleteManagedSession') {
    result = undefined
  } else if (
    name === 'listAvailableAgents' || name === 'listAgentInstallJobs' || name === 'listAgentConfigs' ||
    name === 'listFeedbackInbox' ||
    name === 'listHostSessions' ||
    name === 'listArchivedHostSessions' ||
    name === 'listHostProfiles'
  ) {
    result = []
  } else if (name === 'submitFeedback') {
    result = SAFE_TERMINAL_PROJECTION
  } else {
    result = { operation: name }
  }
  return result as ApplicationCommandResult<Name>
}

export type ApplicationTransportConformanceFixture = Readonly<{
  transport: ApplicationTransport
  expectWireCall: <Name extends ApplicationCommandName>(
    index: number,
    name: Name,
    input: ApplicationCommandInput<Name>,
  ) => void | Promise<void>
  rejectNext: (error: unknown) => void
}>

function assertNoServerStorageLocations(value: unknown): void {
  const serialized = JSON.stringify(value)
  for (const forbidden of [
    'directory_path',
    'markdown_path',
    'manifest_path',
    'package_uri',
    'file://',
  ]) {
    expect(serialized).not.toContain(forbidden)
  }
}

export function runApplicationTransportConformance(
  implementationName: string,
  createFixture: () => ApplicationTransportConformanceFixture,
): void {
  describe(`${implementationName} ApplicationTransport conformance`, () => {
    it('maps all query mutation multipart binary and void operations', async () => {
      const fixture = createFixture()
      expect(APPLICATION_COMMAND_NAMES).toHaveLength(49)

      for (const [index, name] of APPLICATION_COMMAND_NAMES.entries()) {
        const input = APPLICATION_CONFORMANCE_INPUTS[name]
        const result = await fixture.transport.call(name, input)
        expect(result).toEqual(applicationConformanceResult(name))
        await fixture.expectWireCall(index, name, input)
      }
    })

    it('preserves typed CAS errors and safe terminal projections', async () => {
      const fixture = createFixture()
      const conflict = {
        code: 'DRAFT_CONFLICT',
        message: 'draft revision changed',
        retryable: false,
      } as const
      fixture.rejectNext(conflict)
      await expect(
        fixture.transport.call('saveFeedbackDraft', APPLICATION_CONFORMANCE_INPUTS.saveFeedbackDraft),
      ).rejects.toEqual(conflict)

      const terminal = await fixture.transport.call(
        'submitFeedback',
        APPLICATION_CONFORMANCE_INPUTS.submitFeedback,
      )
      expect(terminal).toEqual(SAFE_TERMINAL_PROJECTION)
      assertNoServerStorageLocations(terminal)
    })

    it('provides readiness and a synchronous idempotent unsubscribe', async () => {
      const fixture = createFixture()
      await expect(fixture.transport.waitUntilReady()).resolves.toBeUndefined()
      const unsubscribe = fixture.transport.subscribe(
        defineApplicationStream('conformance:unavailable'),
        vi.fn(),
        vi.fn(),
      )
      expect(unsubscribe).toEqual(expect.any(Function))
      expect(() => {
        unsubscribe()
        unsubscribe()
      }).not.toThrow()
      await Promise.resolve()
    })
  })
}
