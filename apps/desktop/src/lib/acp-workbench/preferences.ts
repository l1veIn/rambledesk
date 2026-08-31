import { writable } from 'svelte/store'
import type { AccessMode } from './types'

export type AcpClientDefaults = {
  agentId: string
  model: string
  reasoningEffort: string
  accessMode: AccessMode
}

const STORAGE_KEY = 'rambledesk.acp-client.defaults.v1'
const fallback: AcpClientDefaults = {
  agentId: 'codex',
  model: '',
  reasoningEffort: 'high',
  accessMode: 'workspace_write',
}

function readDefaults(): AcpClientDefaults {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (!stored) return fallback
    const parsed = JSON.parse(stored) as Partial<AcpClientDefaults>
    return {
      agentId: parsed.agentId || fallback.agentId,
      model: parsed.model || '',
      reasoningEffort: parsed.reasoningEffort || fallback.reasoningEffort,
      accessMode:
        parsed.accessMode === 'read_only' ||
        parsed.accessMode === 'workspace_write' ||
        parsed.accessMode === 'yolo'
          ? parsed.accessMode
          : fallback.accessMode,
    }
  } catch {
    return fallback
  }
}

export const acpClientDefaults = writable<AcpClientDefaults>(readDefaults())

export function setAcpClientDefaults(next: AcpClientDefaults) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
  acpClientDefaults.set(next)
}
