<script lang="ts">
  import { ArrowUpRight, Folder, GitBranch, LoaderCircle, MessageSquare, RefreshCw, ShieldQuestion } from '@lucide/svelte'
  import { onDestroy, tick } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import type { AgentConfig, AgentPromptCapabilities, SessionConfigChange, SessionPromptContent, SessionRecovery } from '$lib/generated/feedback'
  import SessionRecoveryNotice from './SessionRecoveryNotice.svelte'
  import AgentComposer from './composer/AgentComposer.svelte'
  import SessionTimeline from './chat/SessionTimeline.svelte'
  import { TimelineWindow } from './chat/timeline-window'
  import { captureActivityAnchor, restoreActivityAnchor } from './chat/scroll-anchor'
  import { chatText } from './chat/chat-text'
  import SessionConfigurationControls from './configuration/SessionConfigurationControls.svelte'
  import SessionContextUsage from './SessionContextUsage.svelte'
  import { attachmentAccept, canAttachFiles, readPromptFiles, validatePromptAttachments, type PromptAttachment } from './attachments/promptAttachments'
  import { attachmentText } from './attachments/attachmentText'
  import { locale } from '$lib/preferences'
  import { redactAgentMessage } from './agentConfigForm'
  import { agentText } from './agentI18n'
  import {
    activitiesForSession, managedSessionActions, managedSessionComposerState,
    permissionsForSession, sessionConfigurationChanged, sessionPromptDrafts,
    type ManagedSessionViewSnapshot, type SessionActivity, type SessionPermission,
  } from './managedSessionUi'

  export let snapshot: ManagedSessionViewSnapshot
  export let activities: readonly SessionActivity[] = []
  export let historyLoading = false
  export let historyHasMore = false
  export let historyError = ''
  export let onLoadOlder: (() => Promise<void> | void) | undefined = undefined
  export let permissions: readonly SessionPermission[] = []
  export let recovery: SessionRecovery | null = null
  export let config: AgentConfig | null = null
  export let busy = false
  export let error = ''
  export let connecting = false
  export let connectionError = ''
  export let branch: string | null = null
  export let onPrompt: (text: string) => Promise<void> | void
  export let onPromptContent: ((text: string, content: SessionPromptContent[]) => Promise<void> | void) | undefined = undefined
  export let onSetConfiguration: ((change: SessionConfigChange) => Promise<void> | void) | undefined = undefined
  export let onCancel: () => Promise<void> | void
  export let onStart: () => Promise<void> | void
  export let onRefresh: (() => Promise<void> | void) | undefined = undefined
  export let onRespondPermission: (requestId: string, optionId: string | null) => Promise<void> | void
  export let onOpenRamble: (() => Promise<void> | void) | undefined = undefined

  let activeSessionId = ''
  let prompt = ''
  let pending = new Set<string>()
  let errors: Record<string, string> = {}
  let activityViewport: HTMLDivElement | undefined
  let stickToBottom = true
  let destroyed = false
  let attachments: readonly PromptAttachment[] = []
  let fileInput: HTMLInputElement | undefined
  let chooserTarget: { sessionId: string; capabilities: AgentPromptCapabilities } | null = null
  const timelineWindow = new TimelineWindow()
  let windowRevision = 0
  let historyRequestId = ''
  let historyErrors: Record<string, string> = {}
  let renderedActivities: readonly SessionActivity[] = []

  $: if (snapshot.session.session_id !== activeSessionId) selectSession(snapshot.session.session_id)
  $: if (activeSessionId) sessionPromptDrafts.write(activeSessionId, prompt)
  $: visibleActivities = activitiesForSession(snapshot.session.session_id, activities)
  $: {
    windowRevision
    renderedActivities = timelineWindow.read(snapshot.session.session_id, visibleActivities, stickToBottom)
  }
  $: localHistory = renderedActivities.length < visibleActivities.length
  $: loadingHistory = historyLoading || historyRequestId === activeSessionId
  $: visiblePermissions = permissionsForSession(snapshot.session.session_id, permissions)
  $: permission = visiblePermissions[0] ?? null
  $: actions = managedSessionActions(snapshot, visiblePermissions.length)
  $: configurationChanged = sessionConfigurationChanged(snapshot, config)
  $: envText = Object.entries(config?.env ?? {}).map(([key, value]) => `${key}=${value}`).join('\n')
  $: permissionDetails = redactAgentMessage(permission?.details ?? '', envText)
  $: runtimeConnectionFailed = snapshot.runtime.connection === 'failed' || snapshot.runtime.connection === 'disconnected'
  $: visibleQueryError = redactAgentMessage(error, envText)
  $: visibleConnectionError = redactAgentMessage(connectionError || (runtimeConnectionFailed ? snapshot.runtime.last_error : '') || '', envText)
  $: visibleOperationError = redactAgentMessage(errors[snapshot.session.session_id] || (!runtimeConnectionFailed ? snapshot.runtime.last_error : '') || '', envText)
  $: cwd = snapshot.session.management.kind === 'managed' ? snapshot.session.management.cwd : ''
  $: visibleHistoryError = redactAgentMessage(historyErrors[activeSessionId] || historyError, envText)
  $: permissionPending = permission ? pending.has(`${activeSessionId}:permission:${permission.request_id}`) : false
  $: lifecyclePending = connecting || pending.has(`${activeSessionId}:start`)
  $: sendPending = pending.has(`${activeSessionId}:prompt`)
  $: configurationPending = pending.has(`${activeSessionId}:configuration`)
  $: attachmentPending = pending.has(`${activeSessionId}:attachments`)
  $: composerState = managedSessionComposerState(snapshot, visiblePermissions.length, { busy, lifecycle: lifecyclePending || configurationPending || attachmentPending, prompt: sendPending })
  $: promptCapabilities = snapshot.runtime.capabilities.prompt
  $: acceptsFiles = Boolean(onPromptContent) && canAttachFiles(promptCapabilities)
  $: runActive = snapshot.runtime.connection === 'connected' && snapshot.runtime.activity !== 'idle'
  $: if (visibleActivities.length > 0) void followActivity(visibleActivities)

  function tr(source: string) {
    const zh: Record<string, string> = {
      'View Ramble': '查看 Ramble', 'Describe what you want to work on.': '描述你想完成的任务。',
      'This session needs an explicit connection retry before continuing.': '此会话需要手动重试连接后才能继续。',
      'The agent session is still loading.': '正在读取 Agent 会话。',
      'Could not connect to the agent.': '无法连接智能体。',
      'Agent connection ended. Retry to reconnect.': 'Agent 连接已断开，请重试连接。',
      'Reload session': '重新读取会话', 'Retry connection': '重试连接',
    }
    return $locale === 'zh-CN' && zh[source] ? zh[source] : chatText($locale, attachmentText($locale, agentText($locale, source)))
  }

  function selectSession(id: string) {
    if (activeSessionId) sessionPromptDrafts.write(activeSessionId, prompt)
    activeSessionId = id
    prompt = sessionPromptDrafts.read(id)
    attachments = sessionPromptDrafts.readAttachments(id)
    stickToBottom = true
  }

  function editPrompt(text: string) {
    prompt = text
    sessionPromptDrafts.write(activeSessionId, text)
  }

  async function followActivity(_activities: readonly SessionActivity[]) {
    const id = activeSessionId
    await tick()
    if (!destroyed && id === activeSessionId && stickToBottom && activityViewport) {
      activityViewport.scrollTop = activityViewport.scrollHeight
    }
  }

  function rememberScroll() {
    if (!activityViewport) return
    stickToBottom = activityViewport.scrollHeight - activityViewport.scrollTop - activityViewport.clientHeight < 80
  }

  async function loadEarlier() {
    if (loadingHistory || (!localHistory && (!historyHasMore || !onLoadOlder))) return
    const id = activeSessionId
    const load = onLoadOlder
    historyRequestId = id
    historyErrors = { ...historyErrors, [id]: '' }
    stickToBottom = false
    try {
      if (!localHistory && load) { await load(); await tick() }
      if (destroyed || activeSessionId !== id || !activityViewport) return
      const viewport = activityViewport
      const anchor = captureActivityAnchor(viewport)
      timelineWindow.revealOlder(visibleActivities)
      windowRevision += 1
      await tick()
      if (destroyed || activeSessionId !== id) return
      if (anchor) {
        restoreActivityAnchor(viewport, anchor)
        const restoredTop = viewport.scrollTop
        // Newly mounted Markdown editors finish their first layout on this frame.
        requestAnimationFrame(() => {
          if (!destroyed && activeSessionId === id && Math.abs(viewport.scrollTop - restoredTop) < 1) restoreActivityAnchor(viewport, anchor)
        })
      }
    } catch {
      historyErrors = { ...historyErrors, [id]: tr('Could not load earlier messages.') }
    } finally { if (historyRequestId === id) historyRequestId = '' }
  }

  async function run(name: string, operation: () => Promise<void> | void, id = activeSessionId): Promise<boolean> {
    const key = `${id}:${name}`
    if (pending.has(key)) return false
    const operationEnv = envText
    pending = new Set([...pending, key])
    errors = { ...errors, [id]: '' }
    try {
      await operation()
      return true
    } catch (cause) {
      const message = cause instanceof Error ? cause.message
        : typeof cause === 'object' && cause !== null && 'message' in cause ? String(cause.message)
          : 'Something went wrong'
      errors = { ...errors, [id]: redactAgentMessage(message, operationEnv) }
      return false
    } finally {
      const next = new Set(pending)
      next.delete(key)
      pending = next
    }
  }

  async function send(text: string) {
    if (busy || lifecyclePending || configurationPending || attachmentPending || sendPending || !actions.canPrompt || (!text.trim() && attachments.length === 0)) return
    const id = activeSessionId
    const sendPromptContent = onPromptContent
    try {
      if (attachments.length && !sendPromptContent) throw new Error('This session does not support typed attachments.')
      validatePromptAttachments(text, attachments, promptCapabilities)
    } catch (cause) {
      errors = { ...errors, [id]: cause instanceof Error ? cause.message : 'Could not send these attachments.' }
      return
    }
    const submission = sessionPromptDrafts.beginSubmission(id, prompt)
    const sendPrompt = onPrompt
    prompt = ''
    attachments = []
    if (!await run('prompt', () => submission.attachments.length
      ? sendPromptContent!(text, submission.attachments.map((attachment) => attachment.content)) : sendPrompt(text))) {
      if (sessionPromptDrafts.restoreSubmission(submission) && activeSessionId === id) {
        prompt = sessionPromptDrafts.read(id)
        attachments = sessionPromptDrafts.readAttachments(id)
      }
    }
  }

  function chooseFiles() {
    if (!acceptsFiles || composerState.disabled || attachmentPending || !fileInput) return
    chooserTarget = { sessionId: activeSessionId, capabilities: { ...promptCapabilities } }
    fileInput.value = ''
    fileInput.click()
  }

  function selectedFiles(event: Event) {
    const files = Array.from((event.currentTarget as HTMLInputElement).files ?? [])
    const target = chooserTarget
    chooserTarget = null
    if (files.length && target) void addFiles(files, target)
  }

  async function addFiles(files: readonly File[], target = { sessionId: activeSessionId, capabilities: { ...promptCapabilities } }) {
    if (!files.length) return
    await run('attachments', async () => {
      const added = await readPromptFiles(files, target.capabilities)
      const next = [...sessionPromptDrafts.readAttachments(target.sessionId), ...added]
      validatePromptAttachments(sessionPromptDrafts.read(target.sessionId), next, target.capabilities)
      sessionPromptDrafts.writeAttachments(target.sessionId, next)
      if (activeSessionId === target.sessionId) attachments = sessionPromptDrafts.readAttachments(target.sessionId)
    }, target.sessionId)
  }

  function removeAttachment(id: string) {
    if (composerState.disabled) return
    attachments = attachments.filter((attachment) => attachment.id !== id)
    sessionPromptDrafts.writeAttachments(activeSessionId, attachments)
  }

  async function setConfiguration(change: SessionConfigChange) {
    if (busy || lifecyclePending || configurationPending || sendPending || !actions.canPrompt || !onSetConfiguration) return
    const updateConfiguration = onSetConfiguration
    await run('configuration', () => updateConfiguration(change))
  }

  function respond(requestId: string, optionId: string | null) {
    if (busy || lifecyclePending || permissionPending || !actions.canCancel) return
    const respondToPermission = onRespondPermission
    void run(`permission:${requestId}`, () => respondToPermission(requestId, optionId))
  }

  onDestroy(() => {
    destroyed = true
    if (activeSessionId) sessionPromptDrafts.write(activeSessionId, prompt)
  })
</script>

<section class="flex h-full min-h-0 flex-col bg-background @container" aria-label={tr('Agent session')} data-managed-session-id={snapshot.session.session_id}>
  <header class="flex min-h-12 shrink-0 items-center gap-3 border-b px-5 py-2">
    <h2 class="m-0 min-w-0 flex-1 truncate text-sm font-medium">{snapshot.session.title}</h2>
    {#if onOpenRamble}<Button variant="ghost" size="sm" class="h-7 shrink-0 gap-1.5 text-xs" onclick={() => void run('ramble', onOpenRamble!)}>{tr('View Ramble')}<ArrowUpRight class="size-3.5" /></Button>{/if}
  </header>
  {#if snapshot.deleting}<p role="status" class="m-0 border-b border-destructive/25 bg-destructive/5 px-5 py-3 text-xs">{tr('This session is being deleted. Retry deletion to finish cleanup.')}</p>{/if}
  <SessionRecoveryNotice {snapshot} {recovery} {envText} />

  {#if configurationChanged}<p role="status" class="m-0 border-b border-amber-500/25 bg-amber-500/5 px-5 py-2 text-xs leading-5">{tr('This agent is using an earlier configuration. Saved changes apply on its next start.')}</p>{/if}
  {#if visibleConnectionError}
    <div class="flex shrink-0 items-center gap-3 border-b border-destructive/25 bg-destructive/5 px-5 py-2 text-xs">
      <p role="alert" class="m-0 min-w-0 flex-1 break-words text-destructive">{tr(visibleConnectionError)}</p>
      {#if actions.canStart}<Button variant="outline" size="sm" aria-label={tr('Retry connection')} disabled={busy || lifecyclePending} onclick={() => void run('start', onStart)}><RefreshCw class="size-3.5" />{tr('Retry')}</Button>{/if}
    </div>
  {/if}
  {#if visibleQueryError}
    <div class="flex shrink-0 items-center gap-3 border-b border-destructive/25 bg-destructive/5 px-5 py-2 text-xs">
      <p role="alert" class="m-0 min-w-0 flex-1 break-words text-destructive">{tr(visibleQueryError)}</p>
      {#if onRefresh}<Button variant="outline" size="sm" aria-label={tr('Reload session')} disabled={pending.has(`${activeSessionId}:refresh`)} onclick={() => void run('refresh', onRefresh!)}><RefreshCw class="size-3.5" />{tr('Retry')}</Button>{/if}
    </div>
  {/if}
  {#if visibleOperationError && visibleOperationError !== visibleQueryError && visibleOperationError !== visibleConnectionError}
    <p role="alert" class="m-0 shrink-0 break-words border-b border-destructive/25 bg-destructive/5 px-5 py-3 text-xs text-destructive">{tr(visibleOperationError)}</p>
  {/if}

  <div bind:this={activityViewport} onscroll={rememberScroll} class="min-h-0 flex-1 space-y-5 overflow-y-auto overscroll-contain px-5 py-5" aria-label={tr('Session activity')}>
    {#if localHistory || historyHasMore || visibleHistoryError}
      <div class="mx-auto flex max-w-4xl flex-col items-center gap-2 pb-1">
        <Button variant="ghost" size="sm" disabled={loadingHistory || (!localHistory && !onLoadOlder)} onclick={() => void loadEarlier()}>{#if loadingHistory}<LoaderCircle class="size-3.5 animate-spin" />{/if}{tr(loadingHistory ? 'Loading earlier messages…' : 'Load earlier messages')}</Button>
        {#if visibleHistoryError}<p class="m-0 text-xs text-destructive" role="alert">{visibleHistoryError}</p>{/if}
      </div>
    {/if}
    <SessionTimeline sessionId={snapshot.session.session_id} activities={renderedActivities} {runActive} onResize={() => void followActivity(visibleActivities)} />
    {#if visibleActivities.length === 0}
      <div class="mx-auto flex min-h-48 max-w-md flex-col items-center justify-center text-center"><MessageSquare class="mb-3 size-6 text-muted-foreground/50" /><strong class="text-sm font-medium">{tr('No messages yet')}</strong><p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{tr('Describe what you want to work on.')}</p></div>
    {/if}
  </div>

  {#if permission}
    <section class="max-h-72 shrink-0 overflow-y-auto border-t border-amber-500/25 bg-amber-500/5 px-5 py-3" aria-label={tr('Agent permission')}>
      <div class="mx-auto w-full max-w-4xl">
      <div class="flex items-start gap-2"><ShieldQuestion class="mt-0.5 size-4 shrink-0 text-amber-600" /><div class="min-w-0 flex-1"><h3 class="m-0 whitespace-pre-wrap break-words text-xs font-medium">{permission.title}</h3>{#if visiblePermissions.length > 1}<p class="mb-0 mt-1 text-[11px] text-muted-foreground">{tr('More permissions waiting')}: {visiblePermissions.length - 1}</p>{/if}</div></div>
      {#if permissionDetails.trim()}
        {#key permission.request_id}
          <details open class="mt-3 rounded-md border border-amber-500/25 bg-background/50 px-3 py-2 text-xs">
            <summary class="cursor-pointer select-none font-medium">{tr('Operation details')}</summary>
            <pre class="mb-0 mt-2 max-h-40 overflow-y-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-5">{permissionDetails}</pre>
          </details>
        {/key}
      {/if}
      <div class="mt-3 flex flex-wrap gap-2">
        {#each permission.options as option (option.option_id)}<Button variant={option.kind.startsWith('reject') ? 'outline' : 'secondary'} size="sm" disabled={busy || lifecyclePending || permissionPending || !actions.canCancel} onclick={() => respond(permission.request_id, option.option_id)}>{option.name}</Button>{/each}
        <Button variant="ghost" size="sm" disabled={busy || lifecyclePending || permissionPending || !actions.canCancel} onclick={() => respond(permission.request_id, null)}>{tr('Cancel permission')}</Button>
        {#if permissionPending}<LoaderCircle class="size-4 self-center animate-spin text-muted-foreground" />{/if}
      </div>
      </div>
    </section>
  {/if}

  <div class="shrink-0 px-5 pb-3 pt-2">
    <div class="mx-auto w-full max-w-4xl space-y-2" data-agent-composer>
    <input bind:this={fileInput} type="file" class="hidden" tabindex="-1" multiple accept={attachmentAccept(promptCapabilities)} onchange={selectedFiles} aria-label={tr('Attach files')} />
    {#key snapshot.session.session_id}
      <AgentComposer value={prompt} draftKey={snapshot.session.session_id}
        onchange={editPrompt} onsubmit={send}
        {attachments} onAddAttachments={acceptsFiles ? chooseFiles : undefined} onRemoveAttachment={removeAttachment}
        onPasteFiles={acceptsFiles ? (files) => addFiles(files) : undefined}
        disabled={composerState.disabled} busy={composerState.busy} sendDisabled={composerState.sendDisabled}
        oncancel={composerState.canCancel ? async () => { await run('cancel', onCancel) } : undefined}>
        <svelte:fragment slot="footer">
          <span class="max-w-32 truncate px-1 text-[10px] text-muted-foreground" title={config?.name ?? snapshot.session.host_id}>{config?.name ?? snapshot.session.host_id}</span>
          {#if onSetConfiguration}<SessionConfigurationControls configuration={snapshot.runtime.configuration}
            disabled={busy || lifecyclePending || configurationPending || sendPending || !actions.canPrompt} onChange={setConfiguration} />{/if}
          {#if connecting || snapshot.runtime.connection === 'connecting'}<span class="flex items-center gap-1 text-[10px] text-muted-foreground" role="status"><LoaderCircle class="size-3 animate-spin" />{tr('Connecting…')}</span>{/if}
        </svelte:fragment>
      </AgentComposer>
    {/key}
    <div class="flex min-w-0 items-center gap-3 px-1 text-[10px] text-muted-foreground" data-workspace-metadata>
      {#if cwd}<span class="flex min-w-0 items-center gap-1.5" title={cwd}><Folder class="size-3 shrink-0" /><span class="truncate">{cwd}</span></span>{/if}
      {#if branch}<span class="flex min-w-0 max-w-[35%] items-center gap-1.5" title={branch}><GitBranch class="size-3 shrink-0" /><span class="truncate">{branch}</span></span>{/if}
      <span class="flex-1"></span>
      <SessionContextUsage usage={snapshot.runtime.context_usage} />
    </div>
    </div>
  </div>
</section>
