// Environment bindings adapted from Codeg 3ebdfed commands/acp.rs::agent_env_keys
// (Apache-2.0). Inert placeholder variables are deliberately omitted. Values are
// saved only in the selected RambleDesk launch profile, not vendor global files.
export type AgentSetup = {
  description: [string, string]
  key?: string
  baseUrl?: string
  model?: string
  endpoint?: string
  guide?: string
  login?: string
}
const deepseek: AgentSetup = {
  description: ['使用 DeepSeek 模型完成项目任务。可直接填写 API 密钥开始使用。', 'Work on projects with DeepSeek. Enter an API key to get started.'],
  key: 'DEEPSEEK_API_KEY', baseUrl: 'DEEPSEEK_BASE_URL', endpoint: 'https://api.deepseek.com', guide: 'https://platform.deepseek.com/api_keys',
}
export const AGENT_SETUP: Readonly<Record<string, AgentSetup>> = {
  'deepseek-acp': { ...deepseek, model: 'DEEPSEEK_ACP_MODEL' },
  dsh: deepseek,
  'claude-acp': { description: ['使用 Claude Code 的编程能力。可复用 Claude 登录，或填写 API 认证信息。', 'Use Claude Code with an existing login or API credentials.'], key: 'ANTHROPIC_AUTH_TOKEN', baseUrl: 'ANTHROPIC_BASE_URL', model: 'ANTHROPIC_MODEL', endpoint: 'https://api.anthropic.com', guide: 'https://code.claude.com/docs/en/authentication' },
  'codex-acp': { description: ['使用 Codex 完成编程任务。连接会复用本机 Codex 的认证和服务商设置。', 'Work with Codex using your local Codex login and provider settings.'], login: 'codex login', guide: 'https://developers.openai.com/codex/auth' },
  gemini: { description: ['使用 Gemini CLI。可复用已有登录，或填写 Gemini API 密钥。', 'Use Gemini CLI with an existing login or Gemini API key.'], key: 'GEMINI_API_KEY', baseUrl: 'GOOGLE_GEMINI_BASE_URL', model: 'GEMINI_MODEL', guide: 'https://geminicli.com/docs/get-started/authentication/' },
  kimi: { description: ['使用 Kimi Code。填写服务商密钥、模型和地址，或复用已有设置。', 'Use Kimi Code with provider credentials or your existing settings.'], key: 'KIMI_MODEL_API_KEY', baseUrl: 'KIMI_MODEL_BASE_URL', model: 'KIMI_MODEL_NAME', guide: 'https://moonshotai.github.io/kimi-cli/en/' },
  grok: { description: ['使用 Grok 智能体，可填写 xAI API 密钥。', 'Use Grok with an xAI API key.'], key: 'XAI_API_KEY', baseUrl: 'GROK_XAI_API_BASE_URL', model: 'GROK_DEFAULT_MODEL', guide: 'https://console.x.ai/' },
  cursor: { description: ['使用 Cursor Agent，可复用登录或填写 Cursor API 密钥。', 'Use Cursor Agent with an existing login or Cursor API key.'], key: 'CURSOR_API_KEY', baseUrl: 'CURSOR_API_BASE_URL', login: 'cursor-agent login', guide: 'https://cursor.com/docs/cli/overview' },
  qoder: { description: ['使用 Qoder。可复用登录，或填写个人访问令牌。', 'Use Qoder with an existing login or a personal access token.'], key: 'QODER_PERSONAL_ACCESS_TOKEN', model: 'QODER_MODEL', login: 'qoder login', guide: 'https://qoder.com/cli' },
  'pi-acp': { description: ['使用 Pi 的模型和扩展环境。连接会复用 Pi 已有的服务商设置。', 'Use Pi with its configured model providers and extensions.'], guide: 'https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent' },
}

export function applyAgentCredentials(env: Record<string, string>, setup: AgentSetup | undefined, values: { key: string; baseUrl: string; model: string }) {
  const result = { ...env }
  for (const [field, value] of [[setup?.key, values.key], [setup?.baseUrl, values.baseUrl], [setup?.model, values.model]]) {
    if (field && value?.trim()) result[field] = value.trim()
  }
  return result
}
