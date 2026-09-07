// Isolated UI fixture. No native invocation, external credentials or real agents.
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { get, writable } from 'svelte/store'
import type { AgentCatalogEntry, AgentConfig, AgentInspection, AgentInstallJob, HostSessionSummary, ManagedSessionSnapshot, SessionActivity, SessionPromptContent, SessionContentBlock } from '$lib/generated/feedback'
import type { HostProfile } from '$lib/workbench/types'

const names = [['deepseek-acp', 'DeepSeek ACP', 'dsh'], ['dsh', 'DeepSeek Harness', 'dsh'], ['claude-acp', 'Claude Code', 'claude'], ['codex-acp', 'Codex CLI', 'codex'], ['gemini', 'Gemini CLI', 'gemini'], ['pi-acp', 'Pi', 'pi']]
const entries: AgentCatalogEntry[] = names.map(([id, name, host_id]) => ({ id, name, host_id, description: '', connection_kind: ['dsh', 'gemini'].includes(id) ? 'native' : 'bridge', distribution: { kind: 'npm', package: id, pinned_version: '0.8.0', command: id === 'claude-acp' ? 'claude-agent-acp' : id, node_required: '22.0.0' }, args: id === 'dsh' ? ['--profile', 'acp'] : id === 'gemini' ? ['--acp'] : [], dependencies: [], verification: { status: 'unverified', versions: [], note: 'Fixture' } }))
const setupPreview = new URLSearchParams(location.search).has('setup')
export const projectsPreview = new URLSearchParams(location.search).has('projects')
export const previewProjectDirectory = 'C:/Projects/rambledesk'
const missingAgents = new Set(setupPreview ? ['claude-acp', 'gemini'] : [])
const preparedSnapshots = new Map<string, ManagedSessionSnapshot>()
let configs: AgentConfig[] = new URLSearchParams(location.search).has('profiles') ? [
  { id: 'work', catalog_id: 'deepseek-acp', name: 'DeepSeek · Work', host_id: 'dsh', protocol: 'acp', enabled: true, command: 'custom-launcher', args: ['--work'], env: { CUSTOM: 'keep' }, created_at: '', updated_at: '' },
  { id: 'personal', catalog_id: 'deepseek-acp', name: 'DeepSeek · Personal', host_id: 'dsh', protocol: 'acp', enabled: false, command: 'deepseek-acp', args: [], env: {}, created_at: '', updated_at: '' },
  { id: 'custom', name: 'My ACP agent', host_id: 'generic', protocol: 'acp', enabled: true, command: 'my-agent', args: ['--acp'], env: {}, created_at: '', updated_at: '' },
] : projectsPreview ? entries.filter(entry => ['claude-acp', 'codex-acp', 'deepseek-acp'].includes(entry.id)).map(entry => ({ id: `project-${entry.id}`, catalog_id: entry.id, name: entry.name, host_id: entry.host_id, protocol: 'acp', enabled: true, command: entry.distribution.command, args: entry.args, env: {}, created_at: '', updated_at: '' })) : []
function projectSession(id: string, title: string, catalogId: string, cwd: string): HostSessionSummary {
  const entry = entries.find(entry => entry.id === catalogId)!
  return { session_id: id, host_id: entry.host_id, host_session_id: id, title, cwd, source_hint: cwd,
    management: { kind: 'managed', protocol: 'acp', agent_config_id: `project-${catalogId}`, cwd, remote_session_id: `remote-${id}` },
    request_count: 0, pending_count: 0, updated_at: '2026-09-06T09:00:00Z', pinned_at: null, archived_at: null, host_pinned_at: null }
}
export const previewSessions = writable<HostSessionSummary[]>(projectsPreview ? [
  projectSession('project-claude', '简化 Agents 配置流程', 'claude-acp', previewProjectDirectory),
  { ...projectSession('project-codex', '检查项目会话与归档', 'codex-acp', previewProjectDirectory), pinned_at: '2026-09-06T10:00:00Z' },
  projectSession('welcome-dsh', '优化欢迎页面的状态提示', 'deepseek-acp', 'D:/Work/welcome'),
  projectSession('welcome-claude', '补齐语音设置引导', 'claude-acp', 'D:/Work/welcome'),
  projectSession('archive-dsh', '验证旧版本数据兼容性', 'deepseek-acp', 'D:/Archive/rambledesk'),
  { session_id: 'external-preview', host_id: 'pi', host_session_id: 'external-preview', title: '审阅外部适配器反馈', source_hint: 'C:/Notes/review.md', management: { kind: 'external' }, request_count: 1, pending_count: 0, updated_at: '2026-09-05T09:00:00Z', pinned_at: null, archived_at: null, host_pinned_at: null },
] : [])
export const previewArchivedSessions = writable<HostSessionSummary[]>([])
export function previewHostProfile(hostId: string): HostProfile {
  return { id: hostId, label: names.find(([, , host]) => host === hostId)?.[1] ?? hostId,
    icon_svg: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="4" width="18" height="16" rx="4"/><path d="m7 9 3 3-3 3m6 0h4"/></svg>', default_adapter: 'generic_mcp', continuation_mode: 'manual' }
}
export function pinPreviewSession(session: HostSessionSummary, pinned: boolean) {
  previewSessions.update(rows => rows.map(row => row.session_id === session.session_id ? { ...row, pinned_at: pinned ? new Date().toISOString() : null } : row))
}
export function archivePreviewSession(session: HostSessionSummary) {
  previewArchivedSessions.update(rows => [...rows, { ...session, pinned_at: null, archived_at: new Date().toISOString() }])
  previewSessions.update(rows => rows.filter(row => row.session_id !== session.session_id))
}
export function restorePreviewSession() {
  const session = get(previewArchivedSessions).at(-1)
  if (!session) return
  previewArchivedSessions.update(rows => rows.filter(row => row.session_id !== session.session_id))
  previewSessions.update(rows => [...rows, { ...session, archived_at: null }])
}
export function publishPreviewSession(snapshot: ManagedSessionSnapshot) {
  const record = snapshot.session
  if (record.management.kind !== 'managed') return
  const row: HostSessionSummary = { ...record, cwd: record.management.cwd, source_hint: record.management.cwd, request_count: 0, pending_count: 0, pinned_at: null, archived_at: null, host_pinned_at: null }
  previewSessions.update(rows => [row, ...rows.filter(existing => existing.session_id !== row.session_id)])
}
let jobs: AgentInstallJob[] = []
let revision = 0
let promptEpoch = 0
export const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
export const previewProbeCounts = writable({ discovery: 0, connection: 0 })
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))
function changed() { transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'fixture', revision: String(++revision), resources: [{ kind: 'all' }] }) }
const activities: SessionActivity[] = []
let activeTurnId = 'turn-1'
function activity(kind: SessionActivity['kind'], text: string, content?: SessionActivity['content']) {
  const row: SessionActivity = { id: crypto.randomUUID(), session_id: 'preview', sequence: activities.length + 1, turn_id: activeTurnId, kind, text, tool_call_id: null, created_at: new Date().toISOString(), content }
  activities.push(row)
  return row
}
if (new URLSearchParams(location.search).has('history')) {
  for (let index = 1; index <= 60; index++) {
    activeTurnId = `history-${index}`
    activity('user_message', `历史任务 ${index}：检查第 ${index} 个页面的布局。`)
    activity('status', 'Turn started')
    activity('agent_thought', `先检查页面 ${index}，再验证结果。`)
    const calls = index === 60 ? (new URLSearchParams(location.search).has('longTurn') ? 1500 : 120) : 3
    for (let call = 0; call < calls; call++) activity('tool_call', `页面 ${index} · 检查步骤 ${call + 1}`)
    activity('agent_message', `任务 ${index} 已完成。\n\n页面布局与状态检查通过，可以继续检查下一项。`)
    activity('status', 'Turn finished: EndTurn')
  }
}
activeTurnId = 'turn-1'
activity('user_message', '请检查项目的欢迎页面，并改进状态提示。')
activity('status', 'Turn started')
activity('agent_thought', '先读取页面与样式，再检查状态切换是否正确。', { type: 'message', blocks: [{ type: 'text', text: '先读取页面与样式，再检查状态切换是否正确。' }], truncated: false })
activity('tool_call', '读取页面', { type: 'tool_call', tool: { id: 'tool-read', name: 'read_file', title: '读取 src/App.svelte', kind: 'read', status: 'completed', raw_input: '{"path":"src/App.svelte"}', raw_output: '{"lines":128}', content: [{ type: 'text', text: '已读取欢迎页面与状态控件。' }], locations: [{ path: 'src/App.svelte', line: 12 }], truncated: false } })
activity('tool_call', '更新提示', { type: 'tool_call', tool: { id: 'tool-edit', name: 'edit_file', title: '更新状态提示', kind: 'edit', status: 'completed', raw_input: '{"path":"src/App.svelte"}', raw_output: null, content: [{ type: 'diff', path: 'src/App.svelte', old_text: '<p>Connecting...</p>\n<button>Go</button>', new_text: '<p>正在连接智能体…</p>\n<button>开始会话</button>' }], locations: [], truncated: false } })
activity('agent_message', '欢迎页已经更新。\n\n- 连接状态更清晰\n- 操作按钮使用一致的名称\n\n```ts\nconst ready = connection === "connected"\n```\n\n可以继续检查其他页面。', { type: 'message', blocks: [{ type: 'text', text: '欢迎页已经更新。\n\n- 连接状态更清晰\n- 操作按钮使用一致的名称\n\n```ts\nconst ready = connection === "connected"\n```\n\n可以继续检查其他页面。' }], truncated: false })
activity('status', 'Turn finished: EndTurn')
const snapshot: ManagedSessionSnapshot = {
  session: { session_id: 'preview', host_id: 'dsh', host_session_id: 'preview', title: '项目欢迎页面', created_at: '', updated_at: '', management: { kind: 'managed', protocol: 'acp', agent_config_id: 'preview-config', cwd: 'C:/Projects/welcome', remote_session_id: 'remote-preview' } },
  runtime: { connection: 'connected', activity: 'idle', instance_id: 'preview-instance', config_updated_at: null, capabilities: { prompt: { image: true, audio: false, embedded_context: true, resource_links: true }, load_session: true, resume_session: true, http_mcp: true }, last_error: null, configuration: { options: [{ id: 'model', name: '模型', description: null, category: 'model', kind: { type: 'select', current_value: 'deepseek-chat', options: [{ value: 'deepseek-chat', name: 'DeepSeek Chat', description: null, group: null }, { value: 'deepseek-reasoner', name: 'DeepSeek Reasoner', description: null, group: null }] } }] } }, activities, interactions: [], deliveries: [], recovery: null, deleting: false,
}
if (new URLSearchParams(location.search).has('question')) {
  snapshot.runtime.activity = 'waiting_input'
  snapshot.interactions = [{ request_id: 'preview-question', session_id: 'preview', title: '继续之前，需要你确认实现方向', details: null, kind: 'question', input: {
    schema: { type: 'object', required: ['direction', 'checks'], properties: {
      direction: { type: 'string', title: '这次先解决哪个问题？', oneOf: [{ const: 'cache', title: '连接状态缓存', description: '保留检测结果，切换页面后直接查看。' }, { const: 'questions', title: '会话授权与问答', description: '在当前会话处理需要你参与的请求。' }], 'x-rambledesk-allow-other': true },
      checks: { type: 'array', title: '需要验证哪些场景？', items: { type: 'string', enum: ['切换会话', '取消请求', '多选回答'] }, minItems: 1 },
      notes: { type: 'string', title: '补充说明（可选）' },
    } },
  } }]
}
for (const row of get(previewSessions)) {
  if (row.management.kind === 'managed') preparedSnapshots.set(row.session_id, structuredClone({ ...snapshot,
    session: { session_id: row.session_id, host_id: row.host_id, host_session_id: row.host_session_id, title: row.title, lifecycle: 'active', management: row.management, created_at: row.updated_at, updated_at: row.updated_at },
    activities: activities.map(activity => ({ ...activity, session_id: row.session_id })),
    interactions: snapshot.interactions.map(interaction => ({ ...interaction, session_id: row.session_id })),
  }))
}
transport.handle('listAvailableAgents', () => entries).handle('listAgentConfigs', () => configs)
  .handle('getManagedWorkspaceInfo', ({ session_id }) => { const record = (preparedSnapshots.get(session_id) ?? snapshot).session; return { cwd: record.management.kind === 'managed' ? record.management.cwd : '', branch: 'codex/project-sessions' } })
  .handle('listAgentInstallJobs', () => structuredClone(jobs))
  .handle('inspectAgentInstallation', async ({ agent_id }): Promise<AgentInspection> => { previewProbeCounts.update(count => ({ ...count, discovery: count.discovery + 1 })); await delay(150); const missing = missingAgents.has(agent_id); const entry = entries.find(entry => entry.id === agent_id)!; return { agent_id, source: missing ? 'missing' : 'managed', version: missing ? null : '0.8.0', command: missing ? null : `C:/Agents/${entry.distribution.command}.cmd`, args: entry.args, dependencies: agent_id === 'claude-acp' ? [{ command: 'claude', required: false, path: 'C:/Agents/claude.cmd', version: '2.0.0' }] : [], checks: [{ id: 'node', status: 'pass', message: 'Node.js 22.23.0' }, { id: 'npm', status: 'pass', message: 'npm command found' }, { id: 'entry', status: missing ? 'fail' : 'pass', message: missing ? 'Agent entry point was not found' : 'Agent entry point found' }] } })
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
    const config: AgentConfig = { id: crypto.randomUUID(), catalog_id: entry.id, name: entry.name, host_id: entry.host_id, protocol: 'acp', enabled: true, command: `C:/Agents/${entry.distribution.command}.cmd`, args: entry.args, env: {}, created_at: '', updated_at: '' }
    configs.push(config); changed(); return config
  })
  .handle('saveAgentConfig', input => { const config = { ...input, id: input.id ?? crypto.randomUUID(), created_at: '', updated_at: '' }; configs = [...configs.filter(item => item.id !== config.id), config]; changed(); return config })
  .handle('checkAgentConfig', async ({ agent_config_id }) => { previewProbeCounts.update(count => ({ ...count, connection: count.connection + 1 })); await delay(100); const failed = setupPreview && configs.find(config => config.id === agent_config_id)?.catalog_id === 'pi-acp'; return { ok: !failed, message: failed ? 'Agent could not start. Review its configuration and retry.' : 'ACP connection check passed.', details: [] } })
  .handle('deleteAgentConfig', ({ agent_config_id }) => { configs = configs.filter(config => config.id !== agent_config_id); changed() })
  .handle('installAgent', ({ agent_id }) => { const job: AgentInstallJob = { id: crypto.randomUUID(), agent_id, phase: 'installing', messages: ['正在下载连接组件…'], result: null, cancel_requested: false }; jobs.push(job); setTimeout(() => { if (!job.cancel_requested) { job.phase = 'complete'; missingAgents.delete(agent_id); job.messages.push('连接组件安装完成。'); changed() } }, 1600); return structuredClone(job) })
  .handle('cancelAgentInstall', ({ job_id }) => { const job = jobs.find(job => job.id === job_id); if (job) { job.cancel_requested = true; job.phase = 'cancelled'; changed() } })
  .handle('prepareManagedSession', async ({ agent_config_id, cwd }) => {
    if (!cwd.trim()) throw new Error('Choose a project directory before connecting.')
    await delay(200)
    const config = configs.find(config => config.id === agent_config_id)!
    const id = crypto.randomUUID()
    const prepared: ManagedSessionSnapshot = structuredClone({ ...snapshot,
      session: { ...snapshot.session, session_id: id, host_id: config.host_id, host_session_id: id, lifecycle: 'prepared', management: { kind: 'managed', protocol: 'acp', agent_config_id, cwd, remote_session_id: `remote-${id}` } },
      activities: [], runtime: { ...snapshot.runtime, connection: setupPreview && config.catalog_id === 'pi-acp' ? 'failed' : 'connected', last_error: setupPreview && config.catalog_id === 'pi-acp' ? 'Agent could not start. Review its configuration and retry.' : null },
    })
    preparedSnapshots.set(id, prepared)
    return structuredClone(prepared)
  })
  .handle('discardPreparedSession', ({ session_id }) => { preparedSnapshots.delete(session_id) })
  .handle('startManagedSession', ({ session_id }) => { const prepared = preparedSnapshots.get(session_id)!; prepared.runtime.connection = 'connected'; prepared.runtime.last_error = null; return structuredClone(prepared) })
  .handle('getManagedSession', ({ session_id }) => structuredClone(preparedSnapshots.get(session_id) ?? { ...snapshot, activities: snapshot.activities.slice(-100) }))
  .handle('listManagedSessionActivity', async ({ before_sequence, limit, turn_limit }) => {
    await delay(180)
    const older = activities.filter(row => row.sequence < before_sequence)
    const prompts = older.filter(row => row.kind === 'user_message')
    const start = turn_limit ? prompts.at(-turn_limit)?.sequence ?? 1 : 1
    const page = older.filter(row => row.sequence >= start).slice(-(limit ?? 100))
    return { activities: structuredClone(page), has_more: (page[0]?.sequence ?? 1) > 1 }
  })
  .handle('setManagedSessionConfig', async ({ session_id, change }) => { await delay(350); const target = preparedSnapshots.get(session_id) ?? snapshot; const option = target.runtime.configuration.options.find(option => option.id === change.config_id); if (option?.kind.type === 'select' && change.value.type === 'select') option.kind.current_value = change.value.value; changed(); return structuredClone(target) })
  .handle('sendManagedPrompt', ({ session_id, text }) => {
    const prepared = preparedSnapshots.get(session_id)
    if (!prepared) return send(text)
    prepared.session.lifecycle = 'active'
    prepared.session.title = text
    prepared.session.updated_at = new Date().toISOString()
    prepared.activities.push({ id: crypto.randomUUID(), session_id, sequence: prepared.activities.length + 1, turn_id: null, kind: 'user_message', text, tool_call_id: null, created_at: new Date().toISOString() })
    publishPreviewSession(prepared)
    return structuredClone(prepared)
  })
  .handle('sendManagedPromptContent', ({ text, content }) => send(text, content))
  .handle('respondManagedInteraction', ({ session_id, request_id, response }) => {
    const target = preparedSnapshots.get(session_id) ?? snapshot
    const request = target.interactions.find(item => item.request_id === request_id)
    if (!request) throw new Error('Request is no longer pending')
    if (request.kind !== response.kind) throw new Error('Response does not match the request')
    target.interactions = target.interactions.filter(item => item.request_id !== request_id)
    target.runtime.activity = target.interactions.length ? 'waiting_input' : 'idle'
    activity('agent_message', response.kind === 'permission' ? `请求已处理：${response.option_id ?? 'cancel'}` : response.response.action === 'accept' ? `已收到你的回答：${JSON.stringify(response.response.content)}` : `请求已处理：${response.response.action}`)
    changed()
    return structuredClone(target)
  })
  .handle('cancelManagedPrompt', ({ session_id }) => { promptEpoch++; const target = preparedSnapshots.get(session_id) ?? snapshot; target.interactions = []; target.runtime.activity = 'idle'; changed(); return structuredClone(target) })

async function send(text: string, content: SessionPromptContent[] = []) {
    const epoch = ++promptEpoch
    activeTurnId = `sent-${epoch}`
    const blocks: SessionContentBlock[] = [{ type: 'text', text }, ...content.map((block): SessionContentBlock => {
      if (block.type === 'image') return { ...block, uri: null }
      if (block.type === 'resource_link') return { type: 'resource', uri: block.uri, name: block.name, mime_type: block.mime_type, text: null }
      if (block.type === 'resource') return { ...block, name: null }
      return block
    })]
    activity('user_message', text, { type: 'message', blocks, truncated: false }); activity('status', 'Turn started'); snapshot.runtime.activity = 'running'; changed()
    const row = activity('agent_message', '', { type: 'message', blocks: [{ type: 'text', text: '' }], truncated: false })
    for (const chunk of ['收到你的消息。', '\n\n正在逐步输出，', '你可以测试切换页面后返回。', '\n\n**本轮已完成。**']) { await delay(350); if (promptEpoch !== epoch) return structuredClone(snapshot); row.text += chunk; if (row.content?.type === 'message') row.content.blocks = [{ type: 'text', text: row.text }]; changed() }
    activity('status', 'Turn finished: EndTurn'); snapshot.runtime.activity = 'idle'; changed(); return structuredClone(snapshot)
  }
