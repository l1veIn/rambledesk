<script lang="ts">
  import { onDestroy } from 'svelte'
  import { get, writable } from 'svelte/store'
  import { configureClientDiagnostics, type ClientDiagnosticEvent } from '$lib/diagnostics/clientDiagnostics'
  import AgentCatalog from '$lib/agents/AgentCatalog.svelte'
  import ManagedSessionSection from '$lib/agents/ManagedSessionSection.svelte'
  import DraftManagedSessionWorkspace from '$lib/agents/DraftManagedSessionWorkspace.svelte'
  import OnboardingWizard from '$lib/onboarding/OnboardingWizard.svelte'
  import SettingsPanel from '$lib/settings/SettingsPanel.svelte'
  import HostSessionRail from '$lib/components/navigation/HostSessionRail.svelte'
  import FeedbackStatusPreview from './FeedbackStatusPreview.svelte'
  import type { HostSessionSummary } from '$lib/generated/feedback'
  import { adapterPreviewCapabilities, adapterPreviewCalls } from './adapterPreviewCapabilities'
  import { createOnboardingPreviewCapabilities } from './onboardingPreviewCapabilities'
  import { createDraftManagedSessionController } from '$lib/agents/draftManagedSessionController'
  import { createManagedSessionDraftStorage } from '$lib/agents/managedSessionDrafts'
  import { setLocale, setOnboardingStep } from '$lib/preferences'
  import { transport, previewProbeCounts, projectsPreview, previewSessions, previewArchivedSessions, restorePreviewSession, previewProjectDirectory, previewHostProfile, pinPreviewSession, archivePreviewSession } from './agentPreviewFixtures'
  const diagnosticEvents = writable<ClientDiagnosticEvent[]>([])
  onDestroy(configureClientDiagnostics(event => diagnosticEvents.update(events => [...events.slice(-199), event])))
  let page = new URLSearchParams(location.search).has('feedback-status') ? 'feedback-status' : new URLSearchParams(location.search).has('draft') ? 'draft' : 'agents'
  let sessionId = 'preview'
  let activeHostId: string | null = null
  let requestSearch = ''
  let draftId = 'preview-draft'
  let notice = ''
  let agentConfigId: string | undefined
  let agentAdvanced = false
  function configureAgent(configId?: string, advanced = false) {
    agentConfigId = configId
    agentAdvanced = advanced
    page = 'agents'
  }
  $: selectedSession = $previewSessions.find(session => session.host_session_id === sessionId && session.host_id === activeHostId)
  let onboardingOpen = new URLSearchParams(location.search).has('onboarding')
  const onboardingCapabilities = createOnboardingPreviewCapabilities(new URLSearchParams(location.search).get('platform') === 'mac' ? 'macOS' : 'Windows')
  if (onboardingOpen) setOnboardingStep(0)
  const drafts = new Map<string, string>()
  const storage = createManagedSessionDraftStorage({ getItem: key => drafts.get(key) ?? null, setItem: (key, value) => { drafts.set(key, value) } })
  storage.save(draftId, { choice: projectsPreview ? 'config:project-claude-acp' : '', cwd: '', text: '' })
  function createDraft() {
    return createDraftManagedSessionController(transport, draftId, storage, snapshot => { sessionId = snapshot.session.session_id; activeHostId = snapshot.session.host_id; page = 'chat' })
  }
  let draft = createDraft()
  onDestroy(() => { void draft.close() })
  setLocale('zh-CN')
  function toggleTheme() {
    const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark'
    document.documentElement.dataset.theme = next
    document.documentElement.style.colorScheme = next
  }
  async function startFromOnboarding(configId?: string) {
    if (configId) draft.select(`config:${configId}`, '')
    page = 'draft'
  }
  async function openNewDraft(cwd = '') {
    const choice = get(draft).choice
    await draft.close()
    storage.remove(draftId)
    draftId = crypto.randomUUID()
    storage.save(draftId, { choice, cwd, text: '' })
    draft = createDraft()
    page = 'draft'
    activeHostId = null
    notice = ''
  }
  function openSession(hostId: string | null, hostSessionId: string | null) {
    activeHostId = hostId
    sessionId = hostSessionId ?? ''
    const row = $previewSessions.find(session => session.host_session_id === hostSessionId && session.host_id === hostId)
    page = 'requests'
    notice = row?.title ?? '全部请求'
  }
  function archiveSession(session: HostSessionSummary) {
    archivePreviewSession(session)
    if (session.session_id === sessionId) void openNewDraft()
  }
</script>
<div class="flex h-screen flex-col bg-background text-foreground">
  <nav class="flex shrink-0 flex-wrap items-center gap-4 border-b px-6 py-3 text-sm"><strong>RambleDesk</strong><span class="text-xs text-muted-foreground">隔离界面预览</span><button onclick={() => page = 'agents'}>智能体管理</button><button onclick={() => void openNewDraft()}>新建会话</button><button onclick={() => { sessionId = 'preview'; page = 'chat' }}>项目会话</button><button onclick={() => page = 'settings'}>设置</button><button onclick={() => { setOnboardingStep(0); onboardingOpen = true }}>新手引导</button>{#if $previewArchivedSessions.length}<button onclick={restorePreviewSession}>恢复上次归档（{$previewArchivedSessions.length}）</button>{/if}<button class="ml-auto" onclick={toggleTheme}>明暗主题</button></nav>
  <div class="flex min-h-0 flex-1">
  {#if projectsPreview}<HostSessionRail sessions={$previewSessions} activeHostId={page === 'chat' || page === 'requests' ? activeHostId : null} activeHostSessionId={page === 'chat' || page === 'requests' ? sessionId || null : null} inboxActive={page === 'requests' && activeHostId === null} {requestSearch} resolveHostProfile={previewHostProfile} onSelect={openSession} onRequestSearch={search => requestSearch = search} onSearchRequests={search => { activeHostId = null; sessionId = ''; notice = `请求搜索：${search}`; page = 'requests' }} onSettings={() => page = 'settings'} onNewSession={cwd => void openNewDraft(cwd)} onSetSessionPinned={pinPreviewSession} onArchiveSession={archiveSession} />{/if}
  {#if page === 'agents'}<main class="mx-auto w-full max-w-5xl flex-1 overflow-auto p-6"><AgentCatalog {transport} initialConfigId={agentConfigId} initialAdvanced={agentAdvanced} /></main>
  {:else if page === 'feedback-status'}<FeedbackStatusPreview />
  {:else if page === 'draft'}<main class="mx-auto flex min-h-0 min-w-0 w-full flex-1 flex-col">{#key draftId}<DraftManagedSessionWorkspace {transport} controller={draft} {draftId} onConfigure={() => configureAgent()} onConfigureAgent={configureAgent} onChooseDirectory={async () => previewProjectDirectory} />{/key}</main>
  {:else if page === 'settings'}<main class="min-h-0 flex-1"><SettingsPanel {transport} capabilities={adapterPreviewCapabilities} /></main>
  {:else if page === 'requests'}<main class="grid min-w-0 flex-1 place-items-center p-8"><div class="space-y-3 text-center"><h2 class="text-lg font-medium">{notice}</h2><p class="text-sm text-muted-foreground">此预览仅演示侧栏导航，所有数据与操作均保存在内存。</p>{#if selectedSession?.management.kind === 'managed'}<button class="rounded-md border px-3 py-2 text-sm" onclick={() => page = 'chat'}>查看 Agent</button>{/if}</div></main>
  {:else}<main class="mx-auto flex min-h-0 min-w-0 w-full max-w-5xl flex-1 flex-col border-x">{#key sessionId}<ManagedSessionSection {transport} {sessionId} onConfigureAgent={configureAgent} />{/key}</main>{/if}
  </div>
  <p class="m-0 shrink-0 border-t px-6 py-2 text-[10px] text-muted-foreground">模拟调用 · 程序检测 {$previewProbeCounts.discovery} · ACP 检查 {$previewProbeCounts.connection} · 外部适配器 {JSON.stringify($adapterPreviewCalls)} · 诊断事件 {$diagnosticEvents.length} · 失败 {$diagnosticEvents.filter(event => event.outcome === 'failed').length}</p>
  <details class="max-h-40 shrink-0 overflow-auto border-t px-6 py-2 text-[10px]"><summary>模拟诊断时间线（最近 200 条，仅内存）</summary><pre class="whitespace-pre-wrap break-all">{JSON.stringify($diagnosticEvents, null, 2)}</pre></details>
</div>
{#if onboardingOpen}<OnboardingWizard {transport} capabilities={onboardingCapabilities} bind:openWizard={onboardingOpen} onStartSession={startFromOnboarding} />{/if}
