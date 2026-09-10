import type { AgentConfig, AgentInspection } from '$lib/generated/feedback'

type Localized = readonly [string, string]
export type SetupPlatform = 'windows' | 'posix'
type SetupMetadata = {
  name: string
  guide: string
  command?: string
  package?: string
  args?: readonly string[]
  note?: Localized
}

/** User-facing entry points, never ACP transport arguments or account settings. */
export const AGENT_SETUP: Readonly<Record<string, SetupMetadata>> = {
  'claude-acp': { name: 'Claude Code', guide: 'https://code.claude.com/docs/en/setup', note: ['在本机当前用户的 Claude Code 中登录，或为当前连接配置 API key。若修改过配置目录，请确认认证信息保存在当前连接使用的目录中。', 'Sign in through Claude Code on the same machine and user account, or configure an API key for this connection. If you changed the configuration directory, make sure the credentials are stored in the directory used by this connection.'] },
  'codex-acp': { name: 'Codex CLI', guide: 'https://developers.openai.com/codex/cli/', note: ['在本机当前用户的 Codex CLI 中登录，或为当前连接配置 API key。若修改过配置目录，请确认认证信息保存在当前连接使用的目录中。', 'Sign in through Codex CLI on the same machine and user account, or configure an API key for this connection. If you changed the configuration directory, make sure the credentials are stored in the directory used by this connection.'] },
  gemini: { name: 'Gemini CLI', command: 'gemini', package: '@google/gemini-cli', guide: 'https://geminicli.com/docs/get-started/authentication/' },
  'openclaw-acp': { name: 'OpenClaw', guide: 'https://docs.openclaw.ai/', note: ['按 OpenClaw 的说明配置其网关和智能体，再回来检查连接。', 'Follow the OpenClaw guide to configure its gateway and agent, then return to check the connection.'] },
  cline: { name: 'Cline', command: 'cline', package: 'cline', guide: 'https://www.npmjs.com/package/cline' },
  codebuddy: { name: 'CodeBuddy', command: 'codebuddy', package: '@tencent-ai/codebuddy-code', guide: 'https://www.npmjs.com/package/@tencent-ai/codebuddy-code' },
  kimi: { name: 'Kimi Code', command: 'kimi', package: '@moonshot-ai/kimi-code', guide: 'https://moonshotai.github.io/kimi-cli/en/' },
  'pi-acp': { name: 'Pi', guide: 'https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent', note: ['在连接组件使用的 Pi 中完成服务商与模型设置。', 'Configure providers and models in the Pi used by the connector.'] },
  grok: { name: 'Grok', command: 'grok', package: '@xai-official/grok', guide: 'https://www.npmjs.com/package/@xai-official/grok' },
  'deepseek-acp': { name: 'DeepSeek (DSH)', command: 'deepseek-acp', package: 'deepseek-acp', args: ['--setup'], guide: 'https://github.com/xintaofei/deepseek-acp', note: ['使用社区 deepseek-acp 连接组件，组件自带 DSH 运行依赖，无需单独安装或启动 dsh web。可复用 ~/.dsh 中的认证；如未配置，请运行此设置向导。', 'Uses the community deepseek-acp component with its own DSH runtime dependencies. No separate dsh install or running web UI is needed. Existing ~/.dsh credentials can be reused; otherwise run this setup wizard.'] },
  dsh: { name: 'DeepSeek Harness', command: 'dsh', package: '@deepseek-ai/dsh', args: ['web'], guide: 'https://github.com/deepseek-ai/deepseek-harness', note: ['通过 DSH 自己的网页界面完成设置。', 'Complete setup in DSH’s own web interface.'] },
  qoder: { name: 'Qoder', command: 'qoder', package: '@qoder-ai/qodercli', guide: 'https://qoder.com/cli' },
  opencode: { name: 'OpenCode', command: 'opencode', guide: 'https://opencode.ai/docs/' },
  cursor: { name: 'Cursor', command: 'agent', guide: 'https://cursor.com/docs/cli/installation', note: ['请在 Cursor CLI 中完成设置。Cursor 桌面应用与这里连接的 CLI 入口不同；仅打开桌面应用不能确认 CLI 已可用。', 'Complete setup in Cursor CLI. The desktop application and the CLI connected here have different entry points; opening the desktop application does not confirm CLI availability.'] },
  hermes: { name: 'Hermes Agent', command: 'hermes', guide: 'https://github.com/NousResearch/hermes-agent' },
  antigravity: { name: 'Google Antigravity', guide: 'https://github.com/agentclientprotocol/registry', note: ['打开 Antigravity 桌面应用完成设置，并按 ACP 注册表说明准备连接组件。', 'Open the Antigravity desktop application to complete setup, and prepare its connector using the ACP registry instructions.'] },
}

export type AgentSetupInput = {
  catalogId?: string
  hostId?: string
  name?: string
  config?: AgentConfig | null
  inspection?: AgentInspection
  platform?: SetupPlatform
}
export type AgentSetupGuidance = {
  name: string
  guide?: string
  note?: Localized
  command?: string
  platform: SetupPlatform
  hasLaunchOverrides: boolean
}

function basename(path: string): string { return path.replaceAll('\\', '/').split('/').at(-1)?.toLowerCase() ?? '' }
function absolute(path: string): boolean { return path.startsWith('/') || /^[a-z]:[\\/]/iu.test(path) || /^\\\\[^\\]+\\[^\\]+/u.test(path) }
function clean(value: string): boolean { return !!value && !/[\u0000-\u001f\u007f]/u.test(value) }
function commandName(value: string): string { return basename(value).replace(/\.(?:exe|com|cmd|bat|ps1|m?js|cjs)$/u, '') }
function nodeCommand(value: string): boolean { return ['node', 'node.exe'].includes(basename(value)) }
function script(value: string): boolean { return absolute(value) && /\.(?:mjs|cjs|js)$/iu.test(value) }

/** The displayed text targets PowerShell on Windows and a POSIX shell elsewhere. */
export function formatSetupCommand(tokens: readonly string[], platform: SetupPlatform): string | undefined {
  if (!tokens.length || tokens.some(token => !clean(token))) return undefined
  const quote = (value: string) => /^[a-zA-Z0-9_./:@+=-]+$/u.test(value) ? value
    : platform === 'windows' ? `'${value.replaceAll("'", "''")}'` : `'${value.replaceAll("'", "'\\''")}'`
  const rendered = tokens.map(quote).join(' ')
  return platform === 'windows' && quote(tokens[0]).startsWith("'") ? `& ${rendered}` : rendered
}

function launchTokens(command: string | null | undefined, args: readonly string[], names: readonly string[], packageName?: string): string[] | undefined {
  if (!command || !clean(command)) return undefined
  if (nodeCommand(command)) {
    const entry = args[0]
    if (!entry || !clean(entry) || !script(entry)) return undefined
    const normalized = entry.replaceAll('\\', '/')
    if (packageName && !normalized.includes(`/node_modules/${packageName}/`)) return undefined
    return [command, entry]
  }
  if (!names.includes(commandName(command))) return undefined
  return [command]
}

function dependencyTokens(input: AgentSetupInput, name: string): string[] | undefined {
  const dependency = input.inspection?.dependencies.find(value => value.command === name && value.path)
  if (!dependency?.path || !clean(dependency.path)) return undefined
  if (script(dependency.path)) {
    const runtime = input.config?.command ?? input.inspection?.command
    return runtime && nodeCommand(runtime) ? [runtime, dependency.path] : undefined
  }
  return commandName(dependency.path) === name ? [dependency.path] : undefined
}

/** Only derive known interactive entry points. Never reuse arbitrary ACP args or serialize env. */
export function agentSetupGuidance(input: AgentSetupInput): AgentSetupGuidance {
  const host = input.hostId ?? input.config?.host_id
  const hostCatalog: Record<string, string> = { claude: 'claude-acp', codex: 'codex-acp', pi: 'pi-acp', openclaw: 'openclaw-acp' }
  const id = input.catalogId ?? input.config?.catalog_id ?? input.inspection?.agent_id ?? hostCatalog[host ?? ''] ?? host ?? ''
  const metadata = AGENT_SETUP[id]
  const command = input.config?.command ?? input.inspection?.command
  const args = input.config?.args ?? input.inspection?.args ?? []
  const platform = input.platform ?? (/^[a-z]:[\\/]|^\\\\/iu.test(command ?? '') ? 'windows' : 'posix')
  const internalDefaults = new Set(['PATH', 'PI_ACP_PI_COMMAND', 'PI_ACP_ENABLE_EMBEDDED_CONTEXT', 'PI_ACP_DEFAULT'])
  const hasLaunchOverrides = Object.keys(input.config?.env ?? {}).some(key => !internalDefaults.has(key))
  const result: AgentSetupGuidance = { name: input.name ?? metadata?.name ?? input.config?.name ?? 'Agent', guide: metadata?.guide, note: metadata?.note, platform, hasLaunchOverrides }
  let tokens: string[] | undefined
  if (id === 'claude-acp' || id === 'codex-acp') {
    tokens = dependencyTokens(input, id === 'claude-acp' ? 'claude' : 'codex')
  } else if (id === 'pi-acp') {
    // This is the one environment value whose contract is an executable location, not an account setting.
    const pi = input.config?.env.PI_ACP_PI_COMMAND ?? input.inspection?.env?.PI_ACP_PI_COMMAND
    if (pi && clean(pi) && commandName(pi) === 'pi' && (absolute(pi) || pi === 'pi')) {
      tokens = script(pi) && command && nodeCommand(command) ? [command, pi] : !script(pi) ? [pi] : undefined
    }
    tokens ??= dependencyTokens(input, 'pi')
  } else if (metadata?.command) {
    const names = id === 'cursor' ? ['agent', 'cursor-agent'] : [metadata.command]
    tokens = launchTokens(command, args, names, metadata.package)
    // A missing installation has a documented native entry point; an unrecognized custom launcher does not.
    if (!command) tokens = [metadata.command]
    if (tokens) tokens.push(...metadata.args ?? [])
  }
  const sensitive = [...Object.entries(input.inspection?.env ?? {}), ...Object.entries(input.config?.env ?? {})]
    .filter(([key]) => !internalDefaults.has(key)).map(([, value]) => value).filter(Boolean)
  // A Web Access browser may run on a different OS from the Agent's host.
  const actualPath = tokens?.find(absolute) ?? (command && absolute(command) ? command : undefined)
  if (actualPath) result.platform = /^[a-z]:[\\/]|^\\\\/iu.test(actualPath) ? 'windows' : 'posix'
  if (tokens && !tokens.some(token => sensitive.some(value => token.includes(value)))) result.command = formatSetupCommand(tokens, result.platform)
  return result
}
