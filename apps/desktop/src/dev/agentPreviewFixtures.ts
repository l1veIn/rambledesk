// Isolated UI fixture. No native invocation, external credentials or real agents.
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import type { AgentCatalogEntry, AgentConfig, AgentInspection, AgentInstallJob, ManagedSessionSnapshot, SessionActivity, SessionConfigChange, SessionPromptContent, SessionContentBlock } from '$lib/generated/feedback'

const names = [['deepseek-acp', 'DeepSeek ACP', 'dsh'], ['dsh', 'DeepSeek Harness', 'dsh'], ['claude-acp', 'Claude Code', 'claude'], ['codex-acp', 'Codex CLI', 'codex'], ['gemini', 'Gemini CLI', 'gemini'], ['pi-acp', 'Pi', 'pi']]
const entries: AgentCatalogEntry[] = names.map(([id, name, host_id]) => ({ id, name, host_id, description: '', connection_kind: 'bridge', distribution: { kind: 'npm', package: id, pinned_version: '0.8.0', command: id, node_required: '22.0.0' }, args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: 'Fixture' } }))
let configs: AgentConfig[] = new URLSearchParams(location.search).has('profiles') ? [
  { id: 'work', catalog_id: 'deepseek-acp', name: 'DeepSeek · Work', host_id: 'dsh', protocol: 'acp', enabled: true, command: 'custom-launcher', args: ['--work'], env: { CUSTOM: 'keep' }, created_at: '', updated_at: '' },
  { id: 'personal', catalog_id: 'deepseek-acp', name: 'DeepSeek · Personal', host_id: 'dsh', protocol: 'acp', enabled: false, command: 'deepseek-acp', args: [], env: {}, created_at: '', updated_at: '' },
  { id: 'custom', name: 'My ACP agent', host_id: 'generic', protocol: 'acp', enabled: true, command: 'my-agent', args: ['--acp'], env: {}, created_at: '', updated_at: '' },
] : []
let jobs: AgentInstallJob[] = []
let revision = 0
let promptEpoch = 0
export const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))
function changed() { transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'fixture', revision: String(++revision), resources: [{ kind: 'all' }] }) }
const activities: SessionActivity[] = []
function activity(kind: SessionActivity['kind'], text: string, content?: SessionActivity['content']) {
  const row: SessionActivity = { id: crypto.randomUUID(), session_id: 'preview', sequence: activities.length + 1, turn_id: 'turn-1', kind, text, tool_call_id: null, created_at: new Date().toISOString(), content }
  activities.push(row)
  return row
}
if (new URLSearchParams(location.search).has('history')) {
  for (let index = 0; index < 1100; index++) activity('agent_message', `历史消息 ${index + 1}`)
}
activity('user_message', '请检查项目的欢迎页面，并改进状态提示。')
activity('agent_thought', '先读取页面与样式，再检查状态切换是否正确。', { type: 'message', blocks: [{ type: 'text', text: '先读取页面与样式，再检查状态切换是否正确。' }], truncated: false })
activity('tool_call', '读取页面', { type: 'tool_call', tool: { id: 'tool-read', name: 'read_file', title: '读取 src/App.svelte', kind: 'read', status: 'completed', raw_input: '{"path":"src/App.svelte"}', raw_output: '{"lines":128}', content: [{ type: 'text', text: '已读取欢迎页面与状态控件。' }], locations: [{ path: 'src/App.svelte', line: 12 }], truncated: false } })
activity('tool_call', '更新提示', { type: 'tool_call', tool: { id: 'tool-edit', name: 'edit_file', title: '更新状态提示', kind: 'edit', status: 'completed', raw_input: '{"path":"src/App.svelte"}', raw_output: null, content: [{ type: 'diff', path: 'src/App.svelte', old_text: '<p>Connecting...</p>\n<button>Go</button>', new_text: '<p>正在连接智能体…</p>\n<button>开始会话</button>' }], locations: [], truncated: false } })
activity('agent_message', '欢迎页已经更新。\n\n- 连接状态更清晰\n- 操作按钮使用一致的名称\n\n```ts\nconst ready = connection === "connected"\n```\n\n可以继续检查其他页面。', { type: 'message', blocks: [{ type: 'text', text: '欢迎页已经更新。\n\n- 连接状态更清晰\n- 操作按钮使用一致的名称\n\n```ts\nconst ready = connection === "connected"\n```\n\n可以继续检查其他页面。' }], truncated: false })
const snapshot: ManagedSessionSnapshot = {
  session: { session_id: 'preview', host_id: 'dsh', host_session_id: 'preview', title: '项目欢迎页面', created_at: '', updated_at: '', management: { kind: 'managed', protocol: 'acp', agent_config_id: 'preview-config', cwd: 'C:/Projects/welcome', remote_session_id: 'remote-preview' } },
  runtime: { connection: 'connected', activity: 'idle', instance_id: 'preview-instance', config_updated_at: null, capabilities: { prompt: { image: true, audio: false, embedded_context: true, resource_links: true }, load_session: true, resume_session: true, http_mcp: true }, last_error: null, configuration: { options: [{ id: 'model', name: '模型', description: null, category: 'model', kind: { type: 'select', current_value: 'deepseek-chat', options: [{ value: 'deepseek-chat', name: 'DeepSeek Chat', description: null, group: null }, { value: 'deepseek-reasoner', name: 'DeepSeek Reasoner', description: null, group: null }] } }], modes: null, models: null } }, activities, permissions: [], deliveries: [], recovery: null, deleting: false,
}
transport.handle('listAvailableAgents', () => entries).handle('listAgentConfigs', () => configs)
  .handle('listAgentInstallJobs', () => structuredClone(jobs))
  .handle('inspectAgentInstallation', async ({ agent_id }): Promise<AgentInspection> => { await delay(150); return { agent_id, source: 'managed', version: '0.8.0', command: `C:/Agents/${agent_id}.cmd`, args: [], dependencies: [], checks: [{ id: 'node', status: 'pass', message: 'Node.js 22.18.0' }, { id: 'agent', status: 'pass', message: '智能体已安装，可用于新会话。' }] } })
  .handle('resolveCatalogAgent', ({ agent_id, agent_config_id, enable }) => {
    const profiles = configs.filter(config => config.catalog_id === agent_id)
    const existing = agent_config_id ? profiles.find(config => config.id === agent_config_id) : profiles[0]
    if (!agent_config_id && profiles.length > 1) throw new Error('Choose a specific configuration for this Agent')
    if (existing) {
      if (!existing.enabled && !enable) throw new Error('This Agent is disabled')
      existing.enabled = existing.enabled || enable
      return existing
    }
    const entry = entries.find(entry => entry.id === agent_id)!
    const config: AgentConfig = { id: crypto.randomUUID(), catalog_id: entry.id, name: entry.name, host_id: entry.host_id, protocol: 'acp', enabled: true, command: `C:/Agents/${agent_id}.cmd`, args: [], env: {}, created_at: '', updated_at: '' }
    configs.push(config); changed(); return config
  })
  .handle('saveAgentConfig', input => { const config = { ...input, id: input.id ?? crypto.randomUUID(), created_at: '', updated_at: '' }; configs = [...configs.filter(item => item.id !== config.id), config]; changed(); return config })
  .resolve('checkAgentConfig', { ok: true, message: '连接成功。', details: [] })
  .handle('deleteAgentConfig', ({ agent_config_id }) => { configs = configs.filter(config => config.id !== agent_config_id); changed() })
  .handle('installAgent', ({ agent_id }) => { const job: AgentInstallJob = { id: crypto.randomUUID(), agent_id, phase: 'installing', messages: ['正在下载安装包…'], result: null, cancel_requested: false }; jobs.push(job); setTimeout(() => { if (!job.cancel_requested) { job.phase = 'complete'; job.messages.push('安装完成，启动入口检查通过。'); changed() } }, 1600); return structuredClone(job) })
  .handle('cancelAgentInstall', ({ job_id }) => { const job = jobs.find(job => job.id === job_id); if (job) { job.cancel_requested = true; job.phase = 'cancelled'; changed() } })
  .handle('getManagedSession', () => structuredClone({ ...snapshot, activities: snapshot.activities.slice(-1000) }))
  .handle('listManagedSessionActivity', ({ before_sequence, limit }) => { const older = activities.filter(row => row.sequence < before_sequence); return { activities: structuredClone(older.slice(-(limit ?? 100))), has_more: older.length > (limit ?? 100) } })
  .handle('setManagedSessionConfig', async ({ change }) => { await delay(350); const value = change as Extract<SessionConfigChange, { type: 'option' }>; const option = snapshot.runtime.configuration.options.find(option => option.id === value.config_id); if (option?.kind.type === 'select' && value.value.type === 'select') option.kind.current_value = value.value.value; changed(); return structuredClone(snapshot) })
  .handle('sendManagedPrompt', ({ text }) => send(text))
  .handle('sendManagedPromptContent', ({ text, content }) => send(text, content))
  .handle('cancelManagedPrompt', () => { promptEpoch++; snapshot.runtime.activity = 'idle'; changed(); return structuredClone(snapshot) })

async function send(text: string, content: SessionPromptContent[] = []) {
    const epoch = ++promptEpoch
    const blocks: SessionContentBlock[] = [{ type: 'text', text }, ...content.map((block): SessionContentBlock => {
      if (block.type === 'image') return { ...block, uri: null }
      if (block.type === 'resource_link') return { type: 'resource', uri: block.uri, name: block.name, mime_type: block.mime_type, text: null }
      if (block.type === 'resource') return { ...block, name: null }
      return block
    })]
    activity('user_message', text, { type: 'message', blocks, truncated: false }); snapshot.runtime.activity = 'running'; changed()
    const row = activity('agent_message', '', { type: 'message', blocks: [{ type: 'text', text: '' }], truncated: false })
    for (const chunk of ['收到你的消息。', '\n\n正在逐步输出，', '你可以测试切换页面后返回。', '\n\n**本轮已完成。**']) { await delay(350); if (promptEpoch !== epoch) return structuredClone(snapshot); row.text += chunk; if (row.content?.type === 'message') row.content.blocks = [{ type: 'text', text: row.text }]; changed() }
    snapshot.runtime.activity = 'idle'; changed(); return structuredClone(snapshot)
  }
