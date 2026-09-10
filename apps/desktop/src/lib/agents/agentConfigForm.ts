import type { AgentConfig, SaveAgentConfigInput } from '$lib/generated/feedback'

export type AgentDraft = {
  id: string | null
  catalogId?: string
  name: string
  hostId: string
  enabled: boolean
  command: string
  argsText: string
  envText: string
}

export function newAgentDraft(): AgentDraft {
  return { id: null, name: '', hostId: 'generic', enabled: true, command: '', argsText: '', envText: '' }
}
export function agentConfigDraft(config: AgentConfig): AgentDraft {
  return {
    id: config.id,
    catalogId: config.catalog_id,
    name: config.name,
    hostId: config.host_id,
    enabled: config.enabled,
    command: config.command,
    argsText: config.args.join('\n'),
    envText: Object.entries(config.env).map(([key, value]) => `${key}=${value}`).join('\n'),
  }
}

export function parseAgentEnvironment(text: string): Record<string, string> {
  const entries = new Map<string, string>()
  for (const [index, line] of text.split(/\r?\n/u).entries()) {
    if (!line.trim() || line.trimStart().startsWith('#')) continue
    const separator = line.indexOf('=')
    const key = line.slice(0, separator).trim()
    if (separator < 1 || !/^[A-Za-z_][A-Za-z0-9_]*$/u.test(key) || line.includes('\0')) {
      throw new Error(`Invalid environment variable on line ${index + 1}. Use KEY=VALUE.`)
    }
    if (entries.has(key)) throw new Error(`Duplicate environment variable on line ${index + 1}.`)
    entries.set(key, line.slice(separator + 1))
  }
  return Object.fromEntries(entries)
}

export function agentDraftInput(draft: AgentDraft): SaveAgentConfigInput {
  if (!draft.name.trim()) throw new Error('Enter a configuration name.')
  if (!draft.hostId.trim()) throw new Error('Enter an agent backend identifier.')
  if (!draft.command.trim()) throw new Error('Enter an executable command.')
  if (/[\r\n\0]/u.test(draft.command)) throw new Error('The command must be a single executable path or name.')
  if (draft.argsText.includes('\0')) throw new Error('Arguments cannot contain null characters.')
  return {
    id: draft.id,
    ...(draft.catalogId ? { catalog_id: draft.catalogId } : {}),
    name: draft.name.trim(),
    host_id: draft.hostId.trim(),
    protocol: 'acp',
    enabled: draft.enabled,
    command: draft.command.trim(),
    args: draft.argsText.split(/\r?\n/u).filter((line) => line.length > 0),
    env: parseAgentEnvironment(draft.envText),
  }
}

/** Never display an exception or diagnostic containing the current env values. */
export function redactAgentMessage(message: string, envText: string): string {
  const values = envText.split(/\r?\n/u)
    .filter((line) => line.includes('='))
    .map((line) => line.slice(line.indexOf('=') + 1))
    .filter((value) => value.length > 0)
    .sort((left, right) => right.length - left.length)
  return values.reduce((text, value) => text.split(value).join('[redacted]'), message)
}

export function isAbsoluteAgentDirectory(value: string): boolean {
  const path = value.trim()
  if (/[\r\n\0]/u.test(path)) return false
  return path.startsWith('/') || /^[A-Za-z]:[\\/]/u.test(path) || /^\\\\[^\\]+\\[^\\]+/u.test(path)
}

export class AgentDraftCache {
  readonly #drafts = new Map<string, AgentDraft>()
  readonly #saved = new Map<string, AgentDraft>()

  select(id: string | null, configs: readonly AgentConfig[]): AgentDraft {
    const key = id ?? 'new'
    const remembered = this.#drafts.get(key)
    if (remembered) return { ...remembered }
    const config = configs.find((item) => item.id === id)
    const draft = config ? agentConfigDraft(config) : newAgentDraft()
    if (config) this.#saved.set(key, { ...draft })
    this.#drafts.set(key, draft)
    return { ...draft }
  }

  remember(draft: AgentDraft): void {
    this.#drafts.set(draft.id ?? 'new', { ...draft })
  }

  /** Refresh untouched fields after a credential/enable save while retaining local edits. */
  reconcile(config: AgentConfig): AgentDraft {
    const next = agentConfigDraft(config)
    const current = this.#drafts.get(config.id)
    const previous = this.#saved.get(config.id)
    const merged = current && previous ? Object.fromEntries(
      Object.entries(next).map(([key, value]) => [key,
        current[key as keyof AgentDraft] === previous[key as keyof AgentDraft] ? value : current[key as keyof AgentDraft]]),
    ) as AgentDraft : next
    this.#saved.set(config.id, { ...next })
    this.#drafts.set(config.id, { ...merged })
    return { ...merged }
  }

  remove(id: string | null): void {
    this.#drafts.delete(id ?? 'new')
    this.#saved.delete(id ?? 'new')
  }
}
