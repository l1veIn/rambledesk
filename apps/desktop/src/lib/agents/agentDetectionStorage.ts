import type { ApplicationTransport } from '$lib/application/applicationTransport'
import type { AgentConfig, AgentConnectionCheck, AgentInspection } from '$lib/generated/feedback'

export type StoredAgentConnection = { revision: string; result: AgentConnectionCheck }
export type StoredAgentDetection = {
  inspections: Record<string, AgentInspection>
  connections: Record<string, StoredAgentConnection>
}

// The repository changes updated_at on every saved edit. Never write the launch
// signature here: commands, arguments and environment values can contain secrets.
export function agentConfigRevision(config: AgentConfig) {
  return JSON.stringify([config.id, config.created_at, config.updated_at, config.enabled])
}

function key(transport: ApplicationTransport) {
  return transport.persistenceScope ? `rambledesk.agent-detection.v1:${transport.persistenceScope}` : undefined
}

export function loadAgentDetection(transport: ApplicationTransport): StoredAgentDetection {
  const empty = { inspections: {}, connections: {} }
  try {
    const storageKey = key(transport)
    if (!storageKey) return empty
    const value = JSON.parse(localStorage.getItem(storageKey) ?? 'null')
    if (value?.version !== 1 || !value.inspections || !value.connections) return empty
    // Malformed records must never turn an unchecked profile into a usable one.
    for (const [id, inspection] of Object.entries(value.inspections) as [string, AgentInspection][]) {
      if (inspection?.agent_id !== id || !Array.isArray(inspection.checks) || !Array.isArray(inspection.dependencies)
        || !Array.isArray(inspection.args) || !['system', 'managed', 'missing'].includes(inspection.source)) return empty
    }
    for (const checked of Object.values(value.connections) as StoredAgentConnection[]) {
      if (typeof checked?.revision !== 'string' || typeof checked.result?.ok !== 'boolean'
        || typeof checked.result.message !== 'string' || !Array.isArray(checked.result.details)) return empty
    }
    return { inspections: value.inspections, connections: value.connections }
  } catch { return empty }
}

export function saveAgentDetection(transport: ApplicationTransport, value: StoredAgentDetection) {
  try {
    const storageKey = key(transport)
    if (storageKey) localStorage.setItem(storageKey, JSON.stringify({ version: 1, ...value }))
  } catch { /* Detection remains usable in memory when browser storage is unavailable. */ }
}
