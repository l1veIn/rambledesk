import type { AgentCatalogState, AgentListItem } from './agentCatalogController'
import type { AgentCatalogEntry, AgentInspection } from '$lib/generated/feedback'
import { agentLaunchSignature } from './agentDetectionCache'

export type AgentDiagnosis = {
  connection: 'unchecked' | 'missing' | 'prepare' | 'checking' | 'connected' | 'failed'
  reason: 'none' | 'bridge_missing' | 'agent_missing' | 'runtime' | 'dependency' | 'launch' | 'connection' | 'authentication' | 'session' | 'feedback' | 'install'
  canInstall: boolean
}

export function connectionPreparationAvailable(entry: AgentCatalogEntry | undefined, inspection: AgentInspection | undefined): boolean {
  return !!(entry?.connection_kind === 'bridge' && entry.distribution.kind === 'npm' && inspection
    && !inspection.checks.some(check => (check.id === 'node' && check.status === 'fail') || (check.id === 'npm' && check.status !== 'pass'))
    && !inspection.dependencies.some(dependency => dependency.required && !dependency.path
      && !entry.dependencies.some(item => item.command === dependency.command && item.package && item.pinned_version)))
}

/** Project evidence into the next useful action. Error prose is never authentication evidence. */
export function agentDiagnosis(row: AgentListItem, state: AgentCatalogState): AgentDiagnosis {
  const { entry, config } = row
  const inspection = entry && state.inspections[entry.id]
  const canInstall = connectionPreparationAvailable(entry, inspection)
  const result: AgentDiagnosis = { connection: 'unchecked', reason: 'none', canInstall }
  const cached = config && state.connections[config.id]
  const checked = cached && cached.signature === agentLaunchSignature(config!) ? cached.result : undefined
  if (checked) {
    result.connection = checked.connection ?? (checked.ok ? 'connected' : 'failed')
    result.reason = checked.ok ? 'none' : checked.failure?.reason === 'authentication' ? 'authentication'
      : checked.connection === 'connected' ? 'feedback' : checked.failure?.stage === 'launch' ? 'launch' : 'connection'
  }
  if (state.connecting.includes(row.key) || (config && state.connecting.includes(`config:${config.id}`))
    || (entry && state.checking.includes(entry.id))) return { ...result, connection: 'checking' }
  const job = entry && state.jobs.filter(job => job.agent_id === entry.id).at(-1)
  if (job && ['preparing', 'installing', 'verifying'].includes(job.phase)) return { ...result, connection: 'checking', reason: 'install' }
  if (job?.phase === 'failed') return { ...result, connection: checked?.ok ? 'connected' : 'failed', reason: 'install' }
  // The saved launch has been tested directly; catalog discovery must not override its evidence.
  if (checked) return result
  if (!inspection || !entry) return result
  if (inspection.source === 'missing' && entry.connection_kind === 'native') return { ...result, connection: 'missing', reason: 'agent_missing' }
  if (inspection.checks.some(check => ['node', 'npm'].includes(check.id) && check.status === 'fail')) return { ...result, connection: 'failed', reason: 'runtime' }
  const missingDependency = inspection.dependencies.some(dependency => dependency.required && !dependency.path)
  if (canInstall && (!inspection.command || missingDependency)) return { ...result, connection: 'prepare', reason: missingDependency ? 'dependency' : 'bridge_missing' }
  if (missingDependency) return { ...result, connection: 'failed', reason: 'dependency' }
  if (!inspection.command) return { ...result, connection: 'missing', reason: entry.connection_kind === 'bridge' ? 'bridge_missing' : 'agent_missing' }
  if (inspection.checks.some(check => check.status === 'fail')) return { ...result, connection: 'failed', reason: 'launch' }
  return result
}
