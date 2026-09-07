<script lang="ts">
  import App from '../App.svelte'
  import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
  import { adapterPreviewCapabilities, adapterPreviewCalls } from './adapterPreviewCapabilities'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackRequestSummary, HostSessionSummary } from '$lib/generated/feedback'
  import { transport as agentTransport, previewHostProfile } from './agentPreviewFixtures'

  let reads = 0
  let exports = 0
  let diagnosticsEnabled = true
  let diagnosticClears = 0
  let agentReads = 0
  let agentStarts = 0
  let agentConnected = false
  let releaseRestoreLookup: (() => void) | null = null
  const params = new URLSearchParams(location.search)
  const cancelledRestore = params.has('cancelled-agent-restore')
  const sessionRouting = params.has('session-routing') || cancelledRestore
  const cancelledRequest: FeedbackRequestSummary = {
    request_id: 'cancelled-feedback', managed_session_id: 'preview', host_id: 'dsh', host_session_id: 'preview',
    title: '已取消的欢迎页面反馈', source_hint: null, what_happened: '此反馈已取消。主动打开 Agent 对话后才恢复连接。',
    status: params.get('latest') === 'completed' ? 'completed' : 'cancelled',
    resolution: params.get('latest') === 'completed' ? 'feedback_submitted' : 'cancelled', allow_finish: false, final_summary: null,
    revision: 1, created_at: '2026-09-06T09:00:00Z', updated_at: '2026-09-06T09:00:00Z',
  }
  const routingSessions: HostSessionSummary[] = [{
    session_id: 'preview', host_id: 'dsh', host_session_id: 'preview', title: '项目欢迎页面', cwd: 'C:/Projects/welcome', source_hint: 'C:/Projects/welcome',
    management: { kind: 'managed', protocol: 'acp', agent_config_id: 'preview-config', cwd: 'C:/Projects/welcome', remote_session_id: 'remote-preview' },
    request_count: cancelledRestore ? 1 : 0, pending_count: 0, updated_at: '2026-09-06T09:00:00Z', pinned_at: null, archived_at: null, host_pinned_at: null,
  }]
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .resolve('listFeedbackInbox', [])
    .resolve('listHostProfiles', sessionRouting ? [previewHostProfile('dsh')] : [])
    .handle('listFeedbackRequests', async input => {
      if (cancelledRestore && input.limit === 1 && params.has('hold-restore')) {
        await new Promise<void>(resolve => { releaseRestoreLookup = () => { releaseRestoreLookup = null; resolve() } })
      }
      if (cancelledRestore && input.limit === 1 && params.has('lookup-error')) throw new Error('模拟错误：无法读取已取消会话的恢复状态。')
      return { requests: cancelledRestore ? [cancelledRequest] : [], next_cursor: null }
    })
    .resolve('listArchivedHostSessions', [])
    .handle('listHostSessions', () => {
      reads += 1
      if (sessionRouting) return structuredClone(routingSessions)
      if (reads > 1) return []
      if (params.has('timeout')) return new Promise<never>(() => {})
      throw { code: 'storage_error', message: '模拟错误：数据库版本 20 高于当前应用支持的版本 10。请使用兼容版本打开数据。' }
    })
  if (sessionRouting) {
    transport
      .handle('getManagedSession', async input => {
        agentReads += 1
        const snapshot = await agentTransport.call('getManagedSession', input)
        return cancelledRestore && !agentConnected ? { ...snapshot, runtime: { ...snapshot.runtime, connection: 'stopped', instance_id: null } } : snapshot
      })
      .handle('startManagedSession', input => { agentStarts += 1; agentConnected = true; return agentTransport.call('getManagedSession', input) })
      .handle('getManagedWorkspaceInfo', input => agentTransport.call('getManagedWorkspaceInfo', input))
      .handle('listAvailableAgents', input => agentTransport.call('listAvailableAgents', input))
      .resolve('listAgentConfigs', [{ id: 'preview-config', catalog_id: 'deepseek-acp', name: 'DeepSeek ACP', host_id: 'dsh', protocol: 'acp', enabled: true, command: 'deepseek-acp', args: [], env: {}, created_at: '', updated_at: '' }])
      .handle('listManagedSessionActivity', input => agentTransport.call('listManagedSessionActivity', input))
      .handle('setManagedSessionConfig', input => agentTransport.call('setManagedSessionConfig', input))
      .handle('sendManagedPrompt', input => agentTransport.call('sendManagedPrompt', input))
      .handle('sendManagedPromptContent', input => agentTransport.call('sendManagedPromptContent', input))
      .handle('cancelManagedPrompt', input => agentTransport.call('cancelManagedPrompt', input))
    if (cancelledRestore) {
      transport.resolve('getFeedbackWorkspace', {
        request: cancelledRequest, actions: [], context_refs: [], request_attachments: [], attachments: [], feedback: null,
        draft: { document_json: null, body_markdown: '', saved_revision: 1, updated_at: '2026-09-06T09:00:00Z' },
      }).handle('getManagedFeedbackStatus', () => ({ session_id: 'preview', deleting: false,
        connection: agentConnected ? 'connected' : 'stopped', activity: 'idle', deliveries: [] }))
    }
  }
  const status = { availability: 'available' as const, source: 'native' as const }
  const capabilities: WorkbenchCapabilities = {
    ...adapterPreviewCapabilities,
    diagnostics: { status, implementation: {
      readSettings: async () => ({ enabled: diagnosticsEnabled }),
      setEnabled: async (enabled) => { diagnosticsEnabled = enabled; return { enabled } },
      clear: async () => { diagnosticClears += 1 },
      export: async () => {
      exports += 1
      return { path: '/preview/diagnostics.zip', event_count: 3, request_count: 0, log_file_count: 1 }
    } } },
    serverPaths: { status, implementation: {
      ...adapterPreviewCapabilities.serverPaths.implementation,
      chooseSaveFile: async () => '/preview/diagnostics.zip',
      reveal: async () => {},
    } },
  }
</script>

<div class="flex h-screen flex-col bg-background text-foreground">
  <div class="min-h-0 flex-1">
    <App applicationTransport={transport} {capabilities} publishedFeedbackAction={{ label: 'Open feedback package', run: async () => {} }} />
  </div>
  {#if releaseRestoreLookup}
    <button class="border-t px-4 py-2 text-xs" onclick={() => releaseRestoreLookup?.()}>恢复查询已暂停 · 继续完成启动恢复</button>
  {/if}
  <p class="m-0 border-t px-4 py-2 text-xs">{sessionRouting ? '隔离会话导航预览' : '隔离启动预览'} · 会话读取 {reads} · Agent 读取 {agentReads} · ACP 连接 {agentStarts} · 模拟诊断导出 {exports} · 诊断记录 {diagnosticsEnabled ? '开启' : '关闭'} · 清除 {diagnosticClears} · 外部适配器 {JSON.stringify($adapterPreviewCalls)}</p>
</div>
