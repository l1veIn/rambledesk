<script lang="ts">
  import { LoaderCircle, MessageSquare, Play, Send, ShieldQuestion, Square, Trash2 } from '@lucide/svelte'
  import { onDestroy, tick } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Badge } from '$lib/components/ui/badge'
  import type { AgentConfig, FeedbackDelivery, FeedbackRequestSummary, ResolveDeliveryAction, SessionRecovery } from '$lib/generated/feedback'
  import FeedbackDeliveryStatus from './FeedbackDeliveryStatus.svelte'
  import SessionRecoveryNotice from './SessionRecoveryNotice.svelte'
  import { locale } from '$lib/preferences'
  import { redactAgentMessage } from './agentConfigForm'
  import { agentText } from './agentI18n'
  import {
    activitiesForSession, activityLabel, feedbackForSession, managedSessionActions,
    permissionsForSession, sessionConfigurationChanged, sessionPromptDrafts,
    type ManagedSessionViewSnapshot, type SessionActivity, type SessionPermission,
  } from './managedSessionUi'

  export let snapshot: ManagedSessionViewSnapshot
  export let activities: readonly SessionActivity[] = []
  export let permissions: readonly SessionPermission[] = []
  export let feedbackRequests: readonly FeedbackRequestSummary[] = []
  export let deliveries: readonly FeedbackDelivery[] = []
  export let recovery: SessionRecovery | null = null
  export let onResolveDelivery: ((requestId: string, action: ResolveDeliveryAction) => Promise<void> | void) | undefined = undefined
  export let config: AgentConfig | null = null
  export let busy = false
  export let error = ''
  export let onPrompt: (text: string) => Promise<void> | void
  export let onCancel: () => Promise<void> | void
  export let onStart: () => Promise<void> | void
  export let onStop: () => Promise<void> | void
  export let onRespondPermission: (requestId: string, optionId: string | null) => Promise<void> | void
  export let onDelete: (() => Promise<void> | void) | undefined = undefined
  export let onOpenFeedback: (requestId: string) => Promise<void> | void

  let activeSessionId = ''
  let prompt = ''
  let pending = new Set<string>()
  let errors: Record<string, string> = {}
  let activityViewport: HTMLDivElement | undefined
  let stickToBottom = true
  let destroyed = false

  $: if (snapshot.session.session_id !== activeSessionId) selectSession(snapshot.session.session_id)
  $: if (activeSessionId) sessionPromptDrafts.write(activeSessionId, prompt)
  $: visibleActivities = activitiesForSession(snapshot.session.session_id, activities)
  $: visiblePermissions = permissionsForSession(snapshot.session.session_id, permissions)
  $: permission = visiblePermissions[0] ?? null
  $: visibleFeedback = feedbackForSession(snapshot.session, feedbackRequests)
  $: actions = managedSessionActions(snapshot, visiblePermissions.length)
  $: configurationChanged = sessionConfigurationChanged(snapshot, config)
  $: envText = Object.entries(config?.env ?? {}).map(([key, value]) => `${key}=${value}`).join('\n')
  $: visibleError = redactAgentMessage(errors[snapshot.session.session_id] || error || snapshot.runtime.last_error || '', envText)
  $: permissionPending = permission ? pending.has(`${activeSessionId}:permission:${permission.request_id}`) : false
  $: lifecyclePending = pending.has(`${activeSessionId}:start`) || pending.has(`${activeSessionId}:stop`) || pending.has(`${activeSessionId}:delete`)
  $: sendPending = pending.has(`${activeSessionId}:prompt`)
  $: if (visibleActivities.length > 0) void followActivity(visibleActivities)

  function tr(source: string) { return agentText($locale, source) }

  function selectSession(id: string) {
    if (activeSessionId) sessionPromptDrafts.write(activeSessionId, prompt)
    activeSessionId = id
    prompt = sessionPromptDrafts.read(id)
    stickToBottom = true
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

  async function run(name: string, operation: () => Promise<void> | void): Promise<boolean> {
    const id = activeSessionId
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

  async function send() {
    if (busy || lifecyclePending || sendPending || !actions.canPrompt || !prompt.trim()) return
    const id = activeSessionId
    const submitted = prompt
    const sendPrompt = onPrompt
    sessionPromptDrafts.write(id, submitted)
    if (await run('prompt', () => sendPrompt(submitted.trim()))) {
      sessionPromptDrafts.accepted(id, submitted)
      if (activeSessionId === id && prompt === submitted) prompt = sessionPromptDrafts.read(id)
    }
  }

  function respond(requestId: string, optionId: string | null) {
    if (busy || lifecyclePending || permissionPending || !actions.canCancel) return
    const respondToPermission = onRespondPermission
    void run(`permission:${requestId}`, () => respondToPermission(requestId, optionId))
  }

  async function remove() {
    if (busy || lifecyclePending || !onDelete) return
    const id = activeSessionId
    if (await run('delete', onDelete)) {
      sessionPromptDrafts.forgetSession(id)
      if (activeSessionId === id) prompt = ''
    }
  }

  function connectionLabel(connection: ManagedSessionViewSnapshot['runtime']['connection']) {
    switch (connection) {
      case 'connecting': return 'Connecting…'
      case 'connected': return 'Connected'
      case 'disconnected': return 'Disconnected'
      case 'failed': return 'Connection failed'
      case 'stopped': return 'Stopped'
    }
  }

  function currentActivityLabel(activity: ManagedSessionViewSnapshot['runtime']['activity']) {
    switch (activity) {
      case 'running': return 'Agent is working'
      case 'waiting_permission': return 'Waiting for permission'
      case 'idle': return 'Idle'
    }
  }

  onDestroy(() => {
    destroyed = true
    if (activeSessionId) sessionPromptDrafts.write(activeSessionId, prompt)
  })
</script>

<section class="flex h-full min-h-0 flex-col bg-background @container" aria-label={tr('Agent session')} data-managed-session-id={snapshot.session.session_id}>
  <header class="flex flex-wrap items-center gap-3 border-b px-5 py-3">
    <div class="min-w-0 flex-1">
      <h2 class="m-0 truncate text-sm font-medium">{snapshot.session.title}</h2>
      <p class="m-0 mt-1 truncate text-[11px] text-muted-foreground">{config?.name ?? snapshot.session.host_id}{#if snapshot.session.management.kind === 'managed'} · {snapshot.session.management.cwd}{/if}</p>
    </div>
    <div class="flex items-center gap-2 text-xs" role="status">
      {#if snapshot.runtime.connection === 'connecting'}<LoaderCircle class="size-3.5 animate-spin" />{/if}
      <Badge variant={snapshot.runtime.connection === 'failed' ? 'destructive' : 'outline'}>{tr(connectionLabel(snapshot.runtime.connection))}</Badge>
      <span class="text-muted-foreground">{tr(currentActivityLabel(snapshot.runtime.activity))}</span>
    </div>
    <div class="flex items-center gap-1">
      {#if actions.canStart}<Button variant="outline" size="sm" disabled={busy || lifecyclePending} onclick={() => void run('start', onStart)}><Play class="size-3.5" />{tr(actions.startLabel)}</Button>{/if}
      {#if actions.canStop}<Button variant="ghost" size="sm" disabled={busy || lifecyclePending} onclick={() => void run('stop', onStop)}><Square class="size-3.5" />{tr('Stop agent')}</Button>{/if}
      {#if onDelete}<Button variant="ghost" size="icon-sm" class="text-muted-foreground hover:text-destructive" disabled={busy || lifecyclePending} aria-label={tr('Delete session')} title={tr('Delete session')} onclick={() => void remove()}><Trash2 class="size-3.5" /></Button>{/if}
    </div>
  </header>
  {#if snapshot.deleting}<p role="status" class="m-0 border-b border-destructive/25 bg-destructive/5 px-5 py-3 text-xs">{tr('This session is being deleted. Retry deletion to finish cleanup.')}</p>{/if}
  <SessionRecoveryNotice {snapshot} {recovery} {envText} />
  {#if onResolveDelivery}
    <FeedbackDeliveryStatus sessionId={snapshot.session.session_id} {deliveries} requests={visibleFeedback} {envText} disabled={busy || snapshot.deleting} onResolve={onResolveDelivery} {onOpenFeedback} />
  {/if}

  {#if configurationChanged}<p role="status" class="m-0 border-b border-amber-500/25 bg-amber-500/5 px-5 py-2 text-xs leading-5">{tr('This agent is using an earlier configuration. Saved changes apply on its next start.')}</p>{/if}
  {#if visibleError}<p role="alert" class="m-0 break-words border-b border-destructive/25 bg-destructive/5 px-5 py-3 text-xs text-destructive">{tr(visibleError)}</p>{/if}

  <div bind:this={activityViewport} onscroll={rememberScroll} class="min-h-0 flex-1 space-y-5 overflow-y-auto overscroll-contain px-5 py-5" aria-label={tr('Session activity')}>
    {#each visibleActivities as activity (activity.id)}
      {#if activity.kind === 'agent_thought'}
        <details class="rounded-md border bg-muted/15 px-3 py-2 text-xs text-muted-foreground"><summary class="cursor-pointer select-none">{tr(activityLabel(activity.kind))}</summary><p class="mb-0 whitespace-pre-wrap break-words leading-6">{activity.text}</p></details>
      {:else}
        <article class={`space-y-1.5 ${activity.kind === 'user_message' ? 'ml-6 rounded-lg border bg-muted/35 p-3' : activity.kind === 'tool_call' ? 'rounded-lg border border-dashed p-3' : activity.kind === 'error' ? 'rounded-lg border border-destructive/25 p-3 text-destructive' : ''}`}>
          <p class="m-0 text-[10px] font-medium text-muted-foreground">{tr(activityLabel(activity.kind))}</p>
          <div class={`whitespace-pre-wrap break-words text-xs leading-6 ${activity.kind === 'tool_call' ? 'font-mono' : ''}`}>{activity.text}</div>
        </article>
      {/if}
    {/each}
    {#if visibleActivities.length === 0}
      <div class="mx-auto flex min-h-48 max-w-md flex-col items-center justify-center text-center"><MessageSquare class="mb-3 size-6 text-muted-foreground/50" /><strong class="text-sm font-medium">{tr('No messages yet')}</strong><p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{tr(actions.canStart ? 'Start the agent, then describe what you want to work on.' : 'Describe what you want to work on. Feedback requests will appear in this session.')}</p></div>
    {/if}
  </div>

  {#if visibleFeedback.length > 0}
    <div class="flex shrink-0 gap-2 overflow-x-auto border-t bg-muted/15 px-5 py-2" aria-label={tr('Feedback requests')}>
      {#each visibleFeedback as request (request.request_id)}<Button variant="outline" size="sm" class="max-w-72 shrink-0" onclick={() => void run(`feedback:${request.request_id}`, () => onOpenFeedback(request.request_id))}><MessageSquare class="size-3.5 shrink-0" /><span class="truncate">{request.title}</span>{#if request.status === 'waiting' || request.status === 'in_progress'}<span class="size-1.5 shrink-0 rounded-full bg-primary"></span>{/if}</Button>{/each}
    </div>
  {/if}

  {#if permission}
    <section class="max-h-72 shrink-0 overflow-y-auto border-t border-amber-500/25 bg-amber-500/5 px-5 py-3" aria-label={tr('Agent permission')}>
      <div class="flex items-start gap-2"><ShieldQuestion class="mt-0.5 size-4 shrink-0 text-amber-600" /><div class="min-w-0 flex-1"><h3 class="m-0 whitespace-pre-wrap break-words text-xs font-medium">{permission.title}</h3>{#if visiblePermissions.length > 1}<p class="mb-0 mt-1 text-[11px] text-muted-foreground">{tr('More permissions waiting')}: {visiblePermissions.length - 1}</p>{/if}</div></div>
      <div class="mt-3 flex flex-wrap gap-2">
        {#each permission.options as option (option.option_id)}<Button variant={option.kind.startsWith('reject') ? 'outline' : 'secondary'} size="sm" disabled={busy || lifecyclePending || permissionPending || !actions.canCancel} onclick={() => respond(permission.request_id, option.option_id)}>{option.name}</Button>{/each}
        <Button variant="ghost" size="sm" disabled={busy || lifecyclePending || permissionPending || !actions.canCancel} onclick={() => respond(permission.request_id, null)}>{tr('Cancel permission')}</Button>
        {#if permissionPending}<LoaderCircle class="size-4 self-center animate-spin text-muted-foreground" />{/if}
      </div>
    </section>
  {/if}

  <form class="shrink-0 space-y-2 border-t px-5 py-3" onsubmit={(event) => { event.preventDefault(); void send() }}>
    <label class="sr-only" for={`managed-session-prompt-${snapshot.session.session_id}`}>{tr('Message the agent')}</label>
    <textarea id={`managed-session-prompt-${snapshot.session.session_id}`} bind:value={prompt} rows="3" class="max-h-56 min-h-20 w-full resize-y rounded-lg border bg-background px-3 py-2 text-xs leading-5 outline-none focus:ring-2 focus:ring-ring" placeholder={tr('Message the agent')} disabled={busy || snapshot.deleting || pending.has(`${activeSessionId}:delete`)}></textarea>
    <div class="flex items-center justify-between gap-3">
      <p class="m-0 text-[10px] text-muted-foreground">{tr('Closing this view keeps the agent running.')}</p>
      <div class="flex items-center gap-2">
        {#if actions.canCancel}<Button type="button" variant="outline" size="sm" disabled={busy || lifecyclePending || pending.has(`${activeSessionId}:cancel`)} onclick={() => void run('cancel', onCancel)}><Square class="size-3.5" />{tr('Cancel turn')}</Button>{/if}
        <Button type="submit" size="sm" disabled={busy || lifecyclePending || sendPending || !actions.canPrompt || !prompt.trim()}>{#if sendPending}<LoaderCircle class="size-3.5 animate-spin" />{:else}<Send class="size-3.5" />{/if}{tr('Send')}</Button>
      </div>
    </div>
  </form>
</section>
