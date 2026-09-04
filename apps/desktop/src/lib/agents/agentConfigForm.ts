import type { AgentConfig, CreateManagedSessionInput, SaveAgentConfigInput } from '$lib/generated/feedback'

export type AgentDraft = {
  id: string | null
  name: string
  hostId: string
  enabled: boolean
  command: string
  argsText: string
  envText: string
}

export type AgentPreset = Readonly<{
  id: string
  name: string
  hostId: string
  command: string
  args: readonly string[]
  note: string
}>

/** Executables must already be installed; choosing a preset never launches it. */
export const AGENT_PRESETS: readonly AgentPreset[] = [
  { id: 'deepseek', name: 'DeepSeek ACP', hostId: 'dsh', command: 'deepseek-acp', args: [], note: 'Community bridge · versions 0.7 / 0.8' },
  { id: 'dsh', name: 'DeepSeek Harness', hostId: 'dsh', command: 'dsh', args: ['--profile', 'acp'], note: 'Official profile · version 0.1.2-rc.1' },
  { id: 'pi', name: 'Pi ACP', hostId: 'pi', command: 'pi-acp', args: [], note: 'Version 0.0.33 lacks HTTP MCP support required for managed feedback' },
  { id: 'codex', name: 'Codex ACP', hostId: 'codex', command: 'codex-acp', args: [], note: 'Bridge and feedback support need checking' },
]

export function newAgentDraft(preset?: AgentPreset): AgentDraft {
  return {
    id: null,
    name: preset?.name ?? '',
    hostId: preset?.hostId ?? 'generic',
    enabled: true,
    command: preset?.command ?? '',
    argsText: preset?.args.join('\n') ?? '',
    envText: '',
  }
}

export function agentConfigDraft(config: AgentConfig): AgentDraft {
  return {
    id: config.id,
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

export type CreateManagedSessionDraftInput = Readonly<CreateManagedSessionInput>

export function managedSessionDraftInput(
  configId: string,
  cwd: string,
  title: string,
  configs: readonly AgentConfig[],
): CreateManagedSessionDraftInput {
  if (!configs.some((config) => config.id === configId && config.enabled)) {
    throw new Error('Choose an enabled agent configuration.')
  }
  if (!isAbsoluteAgentDirectory(cwd)) throw new Error('Enter an absolute project directory.')
  const directory = cwd.trim()
  const directoryName = directory.replace(/[\\/]+$/u, '').split(/[\\/]/u).at(-1)
  const fallbackTitle = directoryName && !/^[A-Za-z]:$/u.test(directoryName) ? directoryName : 'New session'
  return { agent_config_id: configId, cwd: directory, title: title.trim() || fallbackTitle }
}

export class AgentDraftCache {
  readonly #drafts = new Map<string, AgentDraft>()

  select(id: string | null, configs: readonly AgentConfig[]): AgentDraft {
    const key = id ?? 'new'
    const remembered = this.#drafts.get(key)
    if (remembered) return { ...remembered }
    const config = configs.find((item) => item.id === id)
    const draft = config ? agentConfigDraft(config) : newAgentDraft()
    this.#drafts.set(key, draft)
    return { ...draft }
  }

  remember(draft: AgentDraft): void {
    this.#drafts.set(draft.id ?? 'new', { ...draft })
  }

  remove(id: string | null): void {
    this.#drafts.delete(id ?? 'new')
  }
}
