import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from './contracts'
import type {
  ApplicationStream,
  ApplicationTransport,
  SubscriptionErrorHandler,
  Unsubscribe,
} from './applicationTransport'
import type { CapabilityManifest } from '../capabilities/capabilityManifest'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from '../capabilities/unavailableCapabilities'
import { APPLICATION_EVENTS_STREAM } from './applicationEvents'

export const TAURI_APPLICATION_COMMANDS = {
  listAvailableAgents: 'list_available_agents',
  inspectAgentInstallation: 'inspect_agent_installation',
  listAgentInstallJobs: 'list_agent_install_jobs',
  installAgent: 'install_agent',
  cancelAgentInstall: 'cancel_agent_install',
  listAgentConfigs: 'list_agent_configs',
  saveAgentConfig: 'save_agent_config',
  deleteAgentConfig: 'delete_agent_config',
  checkAgentConfig: 'check_agent_config',
  createManagedSession: 'create_managed_session',
  getManagedSession: 'get_managed_session',
  startManagedSession: 'start_managed_session',
  stopManagedSession: 'stop_managed_session',
  cancelManagedPrompt: 'cancel_managed_prompt',
  sendManagedPrompt: 'send_managed_prompt',
  respondManagedPermission: 'respond_managed_permission',
  resolveFeedbackDelivery: 'resolve_feedback_delivery',
  deleteManagedSession: 'delete_managed_session',
  listFeedbackInbox: 'list_feedback_inbox',
  listHostSessions: 'list_host_sessions',
  listArchivedHostSessions: 'list_archived_host_sessions',
  listHostProfiles: 'list_host_profiles',
  listFeedbackRequests: 'list_feedback_requests',
  getFeedbackWorkspace: 'get_feedback_workspace',
  readPublishedFeedback: 'read_published_feedback',
  saveFeedbackDraft: 'save_feedback_draft',
  addFeedbackAttachment: 'add_feedback_attachment',
  removeFeedbackAttachment: 'remove_feedback_attachment',
  reorderFeedbackAttachments: 'reorder_feedback_attachments',
  submitFeedback: 'submit_feedback',
  approveFeedbackRequest: 'approve_feedback_request',
  cancelFeedbackRequest: 'cancel_feedback_request',
  renameHostSession: 'rename_host_session',
  setHostSessionPinned: 'set_host_session_pinned',
  archiveHostSession: 'archive_host_session',
  unarchiveHostSession: 'unarchive_host_session',
  deleteHostSession: 'delete_host_session',
  deleteFeedbackRequest: 'delete_feedback_request',
  setHostPinned: 'set_host_pinned',
  readFeedbackAttachment: 'read_feedback_attachment',
  readRequestAttachment: 'read_request_attachment',
} as const satisfies Record<ApplicationCommandName, string>

const NO_ARGUMENT_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'listAvailableAgents', 'listAgentInstallJobs',
  'listAgentConfigs',
  'listFeedbackInbox',
  'listHostSessions',
  'listHostProfiles',
])

function tauriArguments<Name extends ApplicationCommandName>(
  name: Name,
  input: ApplicationCommandInput<Name>,
): Record<string, unknown> | undefined {
  if (NO_ARGUMENT_COMMANDS.has(name)) return undefined
  if (name === 'addFeedbackAttachment') {
    const attachment = input as ApplicationCommandInput<'addFeedbackAttachment'>
    return {
      input: {
        ...attachment,
        contents: Array.from(new Uint8Array(attachment.contents)),
      },
    }
  }
  return { input }
}

export class TauriApplicationTransport implements ApplicationTransport {
  private readonly pendingApplicationSubscriptions = new Set<Promise<void>>()

  constructor(
    private readonly capabilityManifest: CapabilityManifest = UNAVAILABLE_CAPABILITY_MANIFEST,
  ) {}

  call<Name extends ApplicationCommandName>(
    name: Name,
    input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>> {
    const command = TAURI_APPLICATION_COMMANDS[name]
    const args = tauriArguments(name, input)
    return args === undefined
      ? invoke<ApplicationCommandResult<Name>>(command)
      : invoke<ApplicationCommandResult<Name>>(command, args)
  }

  subscribe<Event>(
    stream: ApplicationStream<Event>,
    handler: (event: Event) => void,
    onError: SubscriptionErrorHandler,
  ): Unsubscribe {
    let active = true
    let unlisten: Unsubscribe | null = null

    const registration = listen<Event>(stream.id, ({ payload }) => {
      if (active) handler(payload)
    })
      .then((nextUnlisten) => {
        if (active) unlisten = nextUnlisten
        else nextUnlisten()
      })

    if (stream.id === APPLICATION_EVENTS_STREAM.id) {
      this.pendingApplicationSubscriptions.add(registration)
    }
    void registration.then(
      () => this.pendingApplicationSubscriptions.delete(registration),
      (cause) => {
        // Keep a failed active subscription as a readiness failure; a successful
        // snapshot must not erase the error and leave a silently frozen view.
        if (active) onError(cause)
      },
    )

    return () => {
      if (!active) return
      active = false
      this.pendingApplicationSubscriptions.delete(registration)
      unlisten?.()
      unlisten = null
    }
  }

  async waitUntilReady(): Promise<void> {
    // Native listen registers asynchronously. Read only after registration so
    // a change between mounting a projection and its first snapshot is covered.
    await Promise.all(this.pendingApplicationSubscriptions)
  }

  capabilities(): CapabilityManifest {
    return this.capabilityManifest
  }
}
