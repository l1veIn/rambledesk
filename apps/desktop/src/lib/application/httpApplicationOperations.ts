import type { ApplicationCommandInput, ApplicationCommandName } from './contracts'
import { isApplicationError } from './contracts'
import type { ApplicationResourceKey } from '../generated/feedback'
import {
  isRuntimeGenerationStaleError,
  isSnapshotUnstableError,
} from './applicationEvents'

type CatalogInput = { agent_id: string }

export const HTTP_APPLICATION_OPERATIONS = {
  listAvailableAgents: 'listAvailableAgents',
  inspectAgentInstallation: 'inspectAgentInstallation',
  resolveCatalogAgent: 'resolveCatalogAgent',
  listAgentInstallJobs: 'listAgentInstallJobs',
  installAgent: 'installAgent',
  cancelAgentInstall: 'cancelAgentInstall',
  listAgentConfigs: 'listAgentConfigs',
  saveAgentConfig: 'saveAgentConfig',
  deleteAgentConfig: 'deleteAgentConfig',
  checkAgentConfig: 'checkAgentConfig',
  createManagedSession: 'createManagedSession',
  prepareManagedSession: 'prepareManagedSession',
  discardPreparedSession: 'discardPreparedSession',
  getManagedSession: 'getManagedSession',
  getManagedFeedbackStatus: 'getManagedFeedbackStatus',
  getManagedWorkspaceInfo: 'getManagedWorkspaceInfo',
  startManagedSession: 'startManagedSession',
  stopManagedSession: 'stopManagedSession',
  cancelManagedPrompt: 'cancelManagedPrompt',
  sendManagedPrompt: 'sendManagedPrompt',
  listManagedSessionActivity: 'listManagedSessionActivity',
  sendManagedPromptContent: 'sendManagedPromptContent',
  setManagedSessionConfig: 'setManagedSessionConfig',
  respondManagedInteraction: 'respondManagedInteraction',
  resolveFeedbackDelivery: 'resolveFeedbackDelivery',
  deleteManagedSession: 'deleteManagedSession',
  listFeedbackInbox: 'listFeedbackInbox',
  listHostSessions: 'listHostSessions',
  listArchivedHostSessions: 'listArchivedHostSessions',
  listHostProfiles: 'listHostProfiles',
  listFeedbackRequests: 'listFeedbackRequests',
  getFeedbackWorkspace: 'getFeedbackWorkspace',
  readPublishedFeedback: 'readPublishedFeedback',
  saveFeedbackDraft: 'saveFeedbackDraft',
  addFeedbackAttachment: 'addFeedbackAttachment',
  removeFeedbackAttachment: 'removeFeedbackAttachment',
  reorderFeedbackAttachments: 'reorderFeedbackAttachments',
  submitFeedback: 'submitFeedback',
  approveFeedbackRequest: 'approveFeedbackRequest',
  cancelFeedbackRequest: 'cancelFeedbackRequest',
  renameHostSession: 'renameHostSession',
  setHostSessionPinned: 'setHostSessionPinned',
  archiveHostSession: 'archiveHostSession',
  unarchiveHostSession: 'unarchiveHostSession',
  deleteHostSession: 'deleteHostSession',
  deleteFeedbackRequest: 'deleteFeedbackRequest',
  setHostPinned: 'setHostPinned',
  readFeedbackAttachment: 'readFeedbackAttachment',
  readRequestAttachment: 'readRequestAttachment',
} as const satisfies Record<ApplicationCommandName, string>

export type HttpApplicationOperation =
  (typeof HTTP_APPLICATION_OPERATIONS)[ApplicationCommandName]

const NO_ARGUMENT_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'listAvailableAgents', 'listAgentInstallJobs',
  'listAgentConfigs',
  'listFeedbackInbox',
  'listHostSessions',
  'listHostProfiles',
])

export const VOID_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'discardPreparedSession',
  'cancelAgentInstall',
  'deleteManagedSession',
  'deleteAgentConfig',
  'deleteHostSession',
  'deleteFeedbackRequest',
])

export const BINARY_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'readFeedbackAttachment',
  'readRequestAttachment',
])

export const MUTATION_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'prepareManagedSession',
  'discardPreparedSession',
  'resolveCatalogAgent',
  'inspectAgentInstallation', 'installAgent', 'cancelAgentInstall',
  'saveAgentConfig',
  'deleteAgentConfig',
  // The check starts a real agent process and must never be replayed as a query.
  'checkAgentConfig',
  'createManagedSession',
  'startManagedSession',
  'stopManagedSession',
  'cancelManagedPrompt',
  'sendManagedPrompt',
  'sendManagedPromptContent',
  'setManagedSessionConfig',
  'respondManagedInteraction',
  'resolveFeedbackDelivery',
  'deleteManagedSession',
  'saveFeedbackDraft',
  'addFeedbackAttachment',
  'removeFeedbackAttachment',
  'reorderFeedbackAttachments',
  'submitFeedback',
  'approveFeedbackRequest',
  'cancelFeedbackRequest',
  'renameHostSession',
  'setHostSessionPinned',
  'archiveHostSession',
  'unarchiveHostSession',
  'deleteHostSession',
  'deleteFeedbackRequest',
  'setHostPinned',
])


export function parseRevision(revision: string): bigint {
  if (!/^\d+$/u.test(revision)) throw new Error('Application revision must be a decimal string.')
  return BigInt(revision)
}

export function isAuthenticationRejection(status: number): status is 401 | 403 {
  return status === 401 || status === 403
}

export function requestInit<Name extends ApplicationCommandName>(
  name: Name,
  input: ApplicationCommandInput<Name>,
): RequestInit {
  if (NO_ARGUMENT_COMMANDS.has(name)) return { method: 'POST' }

  if (name === 'addFeedbackAttachment') {
    const attachment = input as ApplicationCommandInput<'addFeedbackAttachment'>
    const form = new FormData()
    form.set('request_id', attachment.request_id)
    form.set('file_name', attachment.file_name)
    form.set('expected_revision', String(attachment.expected_revision))
    form.set('file', new Blob([attachment.contents]))
    return { method: 'POST', body: form }
  }

  return {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  }
}

function requestResource(requestId: string): ApplicationResourceKey {
  return { kind: 'feedback_workspace', request_id: canonicalUuid(requestId) }
}

function publishedResource(requestId: string): ApplicationResourceKey {
  return { kind: 'published_feedback', request_id: canonicalUuid(requestId) }
}

function hostSessionResource(
  input: Readonly<{ host_id: string; host_session_id: string }>,
  trim = true,
): ApplicationResourceKey {
  return {
    kind: 'host_session_resources',
    host_id: trim ? input.host_id.trim() : input.host_id,
    host_session_id: trim ? input.host_session_id.trim() : input.host_session_id,
  }
}

function canonicalUuid(value: string): string {
  let candidate = value
  if (candidate.startsWith('urn:uuid:')) candidate = candidate.slice('urn:uuid:'.length)
  if (candidate.startsWith('{') && candidate.endsWith('}')) {
    candidate = candidate.slice(1, -1)
  }
  const simple = /^[0-9a-f]{32}$/iu.test(candidate)
  const hyphenated = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(
    candidate,
  )
  if (!simple && !hyphenated) return value
  const hex = candidate.replaceAll('-', '').toLowerCase()
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

function canonicalSearch(search: string | null): string | null {
  const trimmed = search?.trim() ?? ''
  return trimmed === '' ? null : trimmed
}

function canonicalCursor(cursor: string | null): string | null {
  if (cursor === null || !/^(?:[0-9a-f]{2})+$/iu.test(cursor)) return cursor
  try {
    const bytes = Uint8Array.from(cursor.match(/.{2}/gu) ?? [], (byte) => Number.parseInt(byte, 16))
    const decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes)
    const separator = decoded.indexOf('\0')
    if (separator < 1 || decoded.indexOf('\0', separator + 1) !== -1) return cursor
    const updatedAt = decoded.slice(0, separator)
    const requestId = canonicalUuid(decoded.slice(separator + 1))
    if (requestId === decoded.slice(separator + 1) && !isCanonicalUuid(requestId)) return cursor
    return Array.from(new TextEncoder().encode(`${updatedAt}\0${requestId}`), (byte) =>
      byte.toString(16).padStart(2, '0'),
    ).join('')
  } catch {
    return cursor
  }
}

function isCanonicalUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(value)
}

export function applicationCommandResponseResources<Name extends ApplicationCommandName>(
  name: Name,
  input: ApplicationCommandInput<Name>,
): readonly ApplicationResourceKey[] {
  switch (name) {
    case 'listAvailableAgents':
    case 'inspectAgentInstallation':
    case 'resolveCatalogAgent':
    case 'listAgentInstallJobs':
    case 'installAgent':
    case 'cancelAgentInstall':
    case 'listAgentConfigs':
    case 'saveAgentConfig':
    case 'deleteAgentConfig':
    case 'checkAgentConfig':
      return [{ kind: 'agent_configurations' }]
    case 'createManagedSession':
    case 'prepareManagedSession':
      return [{ kind: 'navigation' }]
    case 'listManagedSessionActivity':
    case 'getManagedSession':
    case 'getManagedFeedbackStatus':
    case 'getManagedWorkspaceInfo':
    case 'startManagedSession':
    case 'stopManagedSession':
    case 'cancelManagedPrompt':
    case 'sendManagedPrompt':
    case 'sendManagedPromptContent':
    case 'setManagedSessionConfig':
    case 'respondManagedInteraction':
    case 'resolveFeedbackDelivery':
    case 'deleteManagedSession':
    case 'discardPreparedSession':
      return [{ kind: 'managed_session', session_id: canonicalUuid((input as { session_id: string }).session_id.trim()) }]
    case 'getFeedbackWorkspace':
    case 'saveFeedbackDraft':
    case 'addFeedbackAttachment':
    case 'removeFeedbackAttachment':
    case 'reorderFeedbackAttachments':
    case 'approveFeedbackRequest':
    case 'readFeedbackAttachment':
    case 'readRequestAttachment':
      return [requestResource((input as { request_id: string }).request_id)]
    case 'readPublishedFeedback':
      return [publishedResource((input as { request_id: string }).request_id)]
    case 'submitFeedback':
    case 'cancelFeedbackRequest': {
      const requestId = (input as { request_id: string }).request_id
      return [requestResource(requestId), publishedResource(requestId)]
    }
    case 'deleteFeedbackRequest': {
      const requestId = (input as { request_id: string }).request_id
      return [requestResource(requestId), publishedResource(requestId)]
    }
    case 'deleteHostSession':
      return [hostSessionResource(input as { host_id: string; host_session_id: string })]
    case 'listFeedbackRequests': {
      const listInput = input as ApplicationCommandInput<'listFeedbackRequests'>
      return listInput.host_id && listInput.host_session_id
        ? [{ kind: 'navigation' }, hostSessionResource({
            host_id: listInput.host_id,
            host_session_id: listInput.host_session_id,
          }, false)]
        : [{ kind: 'navigation' }]
    }
    case 'listFeedbackInbox':
    case 'listHostSessions':
    case 'listArchivedHostSessions':
    case 'listHostProfiles':
    case 'renameHostSession':
    case 'setHostSessionPinned':
    case 'archiveHostSession':
    case 'unarchiveHostSession':
    case 'setHostPinned':
      return [{ kind: 'navigation' }]
  }
}

export function projectionKey(name: ApplicationCommandName, ...scope: unknown[]): string {
  return JSON.stringify([name, ...scope])
}

export function applicationCommandProjectionKey<Name extends ApplicationCommandName>(
  name: Name,
  input: ApplicationCommandInput<Name>,
): string {
  switch (name) {
    case 'listManagedSessionActivity': {
      const page = input as ApplicationCommandInput<'listManagedSessionActivity'>
      return projectionKey(name, page.session_id, String(page.before_sequence), String(page.limit ?? 100))
    }
    case 'listAvailableAgents':
    case 'listAgentInstallJobs':
    case 'listAgentConfigs':
      return projectionKey(name)
    case 'resolveCatalogAgent': {
      const resolve = input as ApplicationCommandInput<'resolveCatalogAgent'>
      return projectionKey(name, resolve.agent_id, resolve.agent_config_id ?? null)
    }
    case 'inspectAgentInstallation':
    case 'installAgent':
      return projectionKey(name, (input as CatalogInput).agent_id)
    case 'cancelAgentInstall':
      return projectionKey(name, (input as { job_id: string }).job_id)
    case 'saveAgentConfig':
      return projectionKey(name, (input as ApplicationCommandInput<'saveAgentConfig'>).id)
    case 'deleteAgentConfig':
    case 'checkAgentConfig':
      return projectionKey(name, canonicalUuid((input as { agent_config_id: string }).agent_config_id.trim()))
    case 'prepareManagedSession':
    case 'createManagedSession': {
      const createInput = input as ApplicationCommandInput<'createManagedSession'>
      return projectionKey(name, canonicalUuid(createInput.agent_config_id.trim()), createInput.cwd.trim())
    }
    case 'getManagedSession':
    case 'getManagedFeedbackStatus':
    case 'getManagedWorkspaceInfo':
    case 'startManagedSession':
    case 'stopManagedSession':
    case 'cancelManagedPrompt':
    case 'sendManagedPrompt':
    case 'sendManagedPromptContent':
    case 'setManagedSessionConfig':
    case 'respondManagedInteraction':
    case 'resolveFeedbackDelivery':
    case 'deleteManagedSession':
    case 'discardPreparedSession':
      return projectionKey(name, canonicalUuid((input as { session_id: string }).session_id.trim()))
    case 'listFeedbackInbox':
    case 'listHostSessions':
    case 'listHostProfiles':
      return projectionKey(name)
    case 'listArchivedHostSessions': {
      const listInput = input as ApplicationCommandInput<'listArchivedHostSessions'>
      return projectionKey(name, canonicalSearch(listInput.search))
    }
    case 'listFeedbackRequests': {
      const listInput = input as ApplicationCommandInput<'listFeedbackRequests'>
      const statuses = [...new Set(listInput.status ?? ['waiting', 'in_progress'])].sort()
      return projectionKey(
        name,
        listInput.host_id,
        listInput.host_session_id,
        statuses,
        listInput.archived ?? false,
        canonicalSearch(listInput.search),
        listInput.limit ?? 50,
        canonicalCursor(listInput.cursor),
      )
    }
    case 'readFeedbackAttachment':
    case 'readRequestAttachment': {
      const readInput = input as ApplicationCommandInput<'readFeedbackAttachment'>
      return projectionKey(
        name,
        canonicalUuid(readInput.request_id),
        canonicalUuid(readInput.attachment_id),
      )
    }
    case 'getFeedbackWorkspace':
    case 'readPublishedFeedback':
    case 'saveFeedbackDraft':
    case 'addFeedbackAttachment':
    case 'removeFeedbackAttachment':
    case 'reorderFeedbackAttachments':
    case 'submitFeedback':
    case 'approveFeedbackRequest':
    case 'cancelFeedbackRequest':
    case 'deleteFeedbackRequest':
      return projectionKey(name, canonicalUuid((input as { request_id: string }).request_id))
    case 'renameHostSession':
    case 'setHostSessionPinned':
    case 'archiveHostSession':
    case 'unarchiveHostSession':
    case 'deleteHostSession': {
      const sessionInput = input as { host_id: string; host_session_id: string }
      return projectionKey(name, sessionInput.host_id.trim(), sessionInput.host_session_id.trim())
    }
    case 'setHostPinned':
      return projectionKey(
        name,
        (input as ApplicationCommandInput<'setHostPinned'>).host_id.trim(),
      )
  }
}

