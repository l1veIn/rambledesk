import type { ApplicationTransport } from './applicationTransport'
import type { ApplicationCommandInput, ApplicationCommandName, ApplicationCommandResult } from './contracts'
import { isSnapshotUnstableError } from './applicationEvents'
import { StaleHttpApplicationResponseError } from './httpApplicationTransport'
import { withApplicationReadTimeout } from './applicationReadTimeout'

const snapshotQueries = [
  'listAvailableAgents', 'listAgentInstallJobs', 'listManagedSessionActivity',
  'listAgentConfigs', 'getManagedSession', 'getManagedFeedbackStatus', 'getManagedWorkspaceInfo', 'listFeedbackInbox', 'listHostSessions',
  'listArchivedHostSessions', 'listHostProfiles', 'listFeedbackRequests',
  'getFeedbackWorkspace', 'readPublishedFeedback',
] as const satisfies readonly ApplicationCommandName[]

export type ApplicationSnapshotQuery = typeof snapshotQueries[number]
const allowedQueries: ReadonlySet<string> = new Set(snapshotQueries)

/** Recover an invalidated read projection without ever replaying its preceding mutation. */
export async function readApplicationSnapshot<Name extends ApplicationSnapshotQuery>(
  transport: ApplicationTransport,
  name: Name,
  input: ApplicationCommandInput<Name>,
): Promise<ApplicationCommandResult<Name>> {
  if (!allowedQueries.has(name)) throw new Error('Snapshot reads only accept read-only application queries.')
  let active = true
  try {
    return await withApplicationReadTimeout((async () => {
      for (let attempt = 0; ; attempt += 1) {
        try {
          return await transport.call(name, input)
        } catch (cause) {
          // A stream event or newer concurrent read may win while this response is decoding.
          // A late response must not start another read after this caller timed out.
          if (!active || attempt >= 2 || !(cause instanceof StaleHttpApplicationResponseError || isSnapshotUnstableError(cause))) throw cause
        }
      }
    })(), name)
  } finally {
    active = false
  }
}
