<script lang="ts">
  import { onDestroy } from 'svelte'
  import WorkspaceHeader from '$lib/workbench/WorkspaceHeader.svelte'
  import ManagedFeedbackRequestStatus from '$lib/agents/ManagedFeedbackRequestStatus.svelte'
  import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
  import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
  import type { ManagedFeedbackStatus } from '$lib/generated/feedback'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import { previewFixtures } from '$lib/previewFixtures'
  import { previewHostProfile, transport as agentTransport } from './agentPreviewFixtures'

  type Scenario = 'normal' | 'uncertain' | 'loading' | 'error' | 'deleting' | 'running' | 'permission'
  const options: { value: Scenario; label: string }[] = [
    { value: 'normal', label: '正常' }, { value: 'uncertain', label: '投递不确定（长错误）' },
    { value: 'loading', label: '正在读取' }, { value: 'error', label: '读取失败' },
    { value: 'deleting', label: '正在删除' }, { value: 'running', label: '正在运行' }, { value: 'permission', label: '等待权限' },
  ]
  const requested = new URLSearchParams(location.search).get('state')
  let scenario: Scenario = options.find(option => option.value === requested)?.value ?? 'normal'
  const sessionId = 'preview'
  const workspace: FeedbackWorkspaceView = structuredClone(previewFixtures.workspace)
  workspace.request = { ...workspace.request, managed_session_id: sessionId, host_id: 'dsh', host_session_id: 'ACP-project-review-session', title: '检查项目会话与反馈投递状态', status: 'completed' }
  const requestId = workspace.request.request_id
  let reads = 0
  let resolves = 0
  let opened = 0
  let revision = 0
  let instance = 0
  const pendingReads = new Set<() => void>()
  const longError = 'Agent disconnected before confirming the feedback request. The request may already have been accepted, but RambleDesk did not receive an acknowledgement. Check the original Agent conversation before sending this feedback again.'
  function currentStatus(): ManagedFeedbackStatus {
    return { session_id: sessionId, deleting: scenario === 'deleting', connection: scenario === 'deleting' ? 'stopped' : 'connected',
      activity: scenario === 'running' ? 'running' : scenario === 'permission' ? 'waiting_input' : 'idle',
      deliveries: [{ session_id: sessionId, request_id: requestId, resolution: 'feedback_submitted', state: scenario === 'uncertain' ? 'uncertain' : 'delivered', attempt_id: null, created_at: '2026-09-06T09:00:00Z', updated_at: '2026-09-06T09:00:00Z', last_error: scenario === 'uncertain' ? longError : null }],
    }
  }
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .handle('getManagedFeedbackStatus', async () => {
      reads += 1
      if (scenario === 'loading') await new Promise<void>(resolve => { pendingReads.add(resolve) })
      if (scenario === 'error') throw new Error('Synthetic feedback status read failure')
      return currentStatus()
    })
    .handle('resolveFeedbackDelivery', async () => {
      resolves += 1
      changeScenario('normal')
      const snapshot = await agentTransport.call('getManagedSession', { session_id: sessionId })
      return { ...snapshot, deliveries: currentStatus().deliveries }
    })
  function changeScenario(next: Scenario) {
    pendingReads.forEach(resolve => resolve())
    pendingReads.clear()
    scenario = next
    if (next === 'loading') instance += 1
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'feedback-status-preview', revision: String(++revision), resources: [{ kind: 'managed_session', session_id: sessionId }] })
  }
  onDestroy(() => { pendingReads.forEach(resolve => resolve()); pendingReads.clear() })
</script>

{#snippet agentStatus()}
  {#key instance}<ManagedFeedbackRequestStatus {transport} {sessionId} {requestId} onOpenAgent={() => opened += 1} />{/key}
{/snippet}

<section class="flex min-h-0 min-w-0 flex-1 flex-col overflow-auto" aria-label="反馈状态布局预览">
  <div class="flex shrink-0 flex-wrap items-center gap-2 border-b bg-muted/10 px-4 py-3">
    {#each options as option}<button class="rounded border px-2 py-1 text-xs aria-pressed:bg-primary aria-pressed:text-primary-foreground" aria-pressed={scenario === option.value} onclick={() => changeScenario(option.value)}>{option.label}</button>{/each}
    <span class="ml-auto text-[10px] text-muted-foreground">读取 {reads} · 处理 {resolves} · 查看 Agent {opened}</span>
  </div>
  <WorkspaceHeader {workspace} resolveHostProfile={previewHostProfile} {agentStatus} />
  <div class="h-px shrink-0 bg-primary" data-status-layout-anchor></div>
  <div class="min-h-0 flex-1 bg-background p-6">
    <h2 class="m-0 text-lg font-medium">反馈工作区内容</h2>
    <p class="mt-3 max-w-2xl text-sm leading-7 text-muted-foreground">切换右侧状态后，上方参考线和这段内容应保持在同一位置。状态的完整说明和处理操作通过真实组件展示。</p>
    <div class="mt-6 h-48 rounded-xl border bg-muted/20 p-5 text-sm text-muted-foreground">这里模拟编辑区域，方便观察状态提示是否挤占工作区。</div>
    <p class="mt-4 text-xs text-muted-foreground">所有状态、错误和处理结果均为内存模拟，不连接 Agent 或写入数据。</p>
  </div>
</section>
