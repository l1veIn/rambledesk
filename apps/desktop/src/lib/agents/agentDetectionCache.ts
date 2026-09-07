import type { ApplicationTransport } from '$lib/application/applicationTransport'
import type { AgentConfig, AgentConnectionCheck, AgentInspection } from '$lib/generated/feedback'
import { redactAgentMessage } from './agentConfigForm'

export type CachedAgentConnection = { signature: string; result: AgentConnectionCheck }
type DetectionSnapshot = { inspections: Record<string, AgentInspection>; connections: Record<string, CachedAgentConnection> }
type DetectionCache = DetectionSnapshot & {
  generation?: string
  inspectionAttempts: Map<string, symbol>
  connectionAttempts: Map<string, { signature: string; token: symbol }>
  listeners: Set<(snapshot: DetectionSnapshot) => void>
}
// Detection is an explicit operation. Keep its results in memory for settings,
// onboarding, and new-session tabs sharing the same application connection.
// Launch signatures contain environment values, so this cache must not be serialized.
const caches = new WeakMap<ApplicationTransport, DetectionCache>()
function cacheFor(transport: ApplicationTransport): DetectionCache {
  let cache = caches.get(transport)
  if (!cache) {
    cache = { inspections: {}, connections: {}, inspectionAttempts: new Map(), connectionAttempts: new Map(), listeners: new Set() }
    caches.set(transport, cache)
  }
  return cache
}
function publish(transport: ApplicationTransport) {
  const snapshot = readAgentDetectionCache(transport)
  for (const listener of cacheFor(transport).listeners) listener(snapshot)
}
export function agentLaunchSignature(config: AgentConfig) {
  return JSON.stringify([config.id, config.host_id, config.protocol, config.enabled, config.command, config.args, Object.entries(config.env).sort(([a], [b]) => a.localeCompare(b))])
}
export function readAgentDetectionCache(transport: ApplicationTransport) {
  const cache = cacheFor(transport)
  return { inspections: { ...cache.inspections }, connections: { ...cache.connections } }
}
export function subscribeAgentDetectionCache(transport: ApplicationTransport, listener: (snapshot: DetectionSnapshot) => void) {
  const cache = cacheFor(transport)
  cache.listeners.add(listener)
  listener(readAgentDetectionCache(transport))
  return () => { cache.listeners.delete(listener) }
}
export function rememberAgentInspection(transport: ApplicationTransport, inspection: AgentInspection) {
  cacheFor(transport).inspections[inspection.agent_id] = inspection
  publish(transport)
}
export function rememberAgentConnection(transport: ApplicationTransport, config: AgentConfig, result: AgentConnectionCheck) {
  const environment = Object.entries(config.env).map(([key, value]) => `${key}=${value}`).join('\n')
  cacheFor(transport).connections[config.id] = { signature: agentLaunchSignature(config), result: {
    ...result, message: redactAgentMessage(result.message, environment), details: result.details.map(detail => redactAgentMessage(detail, environment)),
  } }
  publish(transport)
}
/** The latest explicit attempt owns its result, even after its settings view closes. */
export function beginAgentInspection(transport: ApplicationTransport, agentId: string) {
  const cache = cacheFor(transport)
  const token = Symbol()
  cache.inspectionAttempts.set(agentId, token)
  delete cache.inspections[agentId]
  publish(transport)
  return (inspection: AgentInspection) => {
    if (cache.inspectionAttempts.get(agentId) !== token) return false
    cache.inspectionAttempts.delete(agentId)
    rememberAgentInspection(transport, inspection)
    return true
  }
}
export function beginAgentConnection(transport: ApplicationTransport, config: AgentConfig) {
  const cache = cacheFor(transport)
  const token = Symbol()
  cache.connectionAttempts.set(config.id, { signature: agentLaunchSignature(config), token })
  return (result: AgentConnectionCheck) => {
    if (cache.connectionAttempts.get(config.id)?.token !== token) return false
    cache.connectionAttempts.delete(config.id)
    rememberAgentConnection(transport, config, result)
    return true
  }
}
export function forgetAgentConnection(transport: ApplicationTransport, id: string) {
  const cache = cacheFor(transport)
  delete cache.connections[id]
  cache.connectionAttempts.delete(id)
  publish(transport)
}
/** Saved launch edits and deletions invalidate both completed and pending checks. */
export function reconcileAgentConnections(transport: ApplicationTransport, configs: readonly AgentConfig[]) {
  const cache = cacheFor(transport)
  const signatures = new Map(configs.map(config => [config.id, agentLaunchSignature(config)]))
  let changed = false
  for (const [id, checked] of Object.entries(cache.connections)) {
    if (checked.signature !== signatures.get(id)) { delete cache.connections[id]; changed = true }
  }
  for (const [id, attempt] of cache.connectionAttempts) {
    if (attempt.signature !== signatures.get(id)) cache.connectionAttempts.delete(id)
  }
  if (changed) publish(transport)
}
export function resetAgentDetectionCache(transport: ApplicationTransport) {
  const cache = cacheFor(transport)
  cache.inspections = {}; cache.connections = {}
  cache.inspectionAttempts.clear(); cache.connectionAttempts.clear()
  publish(transport)
}
export function observeAgentRuntime(transport: ApplicationTransport, generation: string) {
  const cache = cacheFor(transport)
  if (cache.generation !== undefined && cache.generation !== generation) resetAgentDetectionCache(transport)
  cache.generation = generation
}
