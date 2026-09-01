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

export const TAURI_APPLICATION_COMMANDS = {
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

export class TauriApplicationTransport<CapabilityManifest = unknown>
  implements ApplicationTransport<CapabilityManifest>
{
  constructor(
    private readonly capabilityManifest: CapabilityManifest = undefined as CapabilityManifest,
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

    void listen<Event>(stream.id, ({ payload }) => {
      if (active) handler(payload)
    })
      .then((nextUnlisten) => {
        if (active) unlisten = nextUnlisten
        else nextUnlisten()
      })
      .catch((cause) => {
        if (active) onError(cause)
      })

    return () => {
      if (!active) return
      active = false
      unlisten?.()
      unlisten = null
    }
  }

  waitUntilReady(): Promise<void> {
    return Promise.resolve()
  }

  capabilities(): CapabilityManifest {
    return this.capabilityManifest
  }
}
