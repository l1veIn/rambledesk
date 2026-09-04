import type { ApplicationTransport } from './applicationTransport'
import type { ApplicationCommandInput, ApplicationCommandName, ApplicationCommandResult } from './contracts'
import { isSnapshotUnstableError } from './applicationEvents'
import { StaleHttpApplicationResponseError } from './httpApplicationTransport'

const snapshotQueries = [
  'listAvailableAgents', 'listAgentInstallJobs', 'listManagedSessionActivity',
  'listAgentConfigs', 'getManagedSession', 'listFeedbackInbox', 'listHostSessions',
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
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await transport.call(name, input)
    } catch (cause) {
      // A stream event or newer concurrent read may win while this response is decoding.
      // Authentication changes and all other failures still require the caller's normal handling.
      if (attempt >= 2 || !(cause instanceof StaleHttpApplicationResponseError || isSnapshotUnstableError(cause))) throw cause
    }
  }
}
