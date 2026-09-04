<script lang="ts">
  import { onMount } from 'svelte'
  import { LoaderCircle } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { FeedbackRequestSummary } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { agentText } from './agentI18n'
  import { redactAgentMessage } from './agentConfigForm'
  import { createAgentSettingsController } from './agentSettingsController'
  import { createManagedSessionController } from './managedSessionController'
  import ManagedSessionWorkspace from './ManagedSessionWorkspace.svelte'
  import FeedbackDeliveryStatus from './FeedbackDeliveryStatus.svelte'

  export let transport: ApplicationTransport
  export let sessionId: string
  export let showWorkspace = true
  export let feedbackRequests: readonly FeedbackRequestSummary[] = []
  export let onOpenFeedback: (requestId: string) => Promise<void> | void
  export let onDelete: (() => Promise<void> | void) | undefined = undefined
  export let deletionPending = false
  export let onDeletingChange: (sessionId: string, deleting: boolean) => void = () => {}
  let reportedDeleting: boolean | undefined
  let wasDeletionPending = false

  // The parent keys this component by local session ID; handlers never follow another tab's selection.
  const session = createManagedSessionController(transport, sessionId)
  const settings = createAgentSettingsController(transport)
  $: management = $session.snapshot?.session.management
  $: config = management?.kind === 'managed'
    ? $settings.configs.find((candidate) => candidate.id === management.agent_config_id) ?? null
    : null
  $: envText = Object.entries(config?.env ?? {}).map(([key, value]) => `${key}=${value}`).join('\n')
  $: error = redactAgentMessage($session.error || $settings.error, envText)
  $: if ($session.snapshot && $session.snapshot.deleting !== reportedDeleting) {
    reportedDeleting = $session.snapshot.deleting
    onDeletingChange(sessionId, reportedDeleting)
  }
  $: if (wasDeletionPending !== deletionPending) {
    wasDeletionPending = deletionPending
    if (!deletionPending) session.refresh()
  }

  async function remove() {
    try { await onDelete?.() }
    finally { session.refresh() }
  }

  onMount(() => {
    const stopSession = session.start()
    const stopSettings = settings.start()
    return () => { stopSession(); stopSettings() }
  })
</script>

{#if $session.snapshot && showWorkspace}
  <ManagedSessionWorkspace
    snapshot={$session.snapshot}
    activities={$session.snapshot.activities}
    permissions={$session.snapshot.permissions}
    deliveries={$session.snapshot.deliveries}
    onResolveDelivery={session.resolveDelivery}
    {feedbackRequests}
    {config}
    {error}
    busy={deletionPending}
    onPrompt={session.prompt}
    onCancel={session.cancel}
    onStart={session.startAgent}
    onStop={session.stopAgent}
    onRespondPermission={session.respondPermission}
    onDelete={onDelete ? remove : undefined}
    {onOpenFeedback}
  />
{:else if $session.snapshot}
  {#if $session.snapshot.deleting}<p role="status" class="m-0 shrink-0 border-b border-destructive/25 bg-destructive/5 px-5 py-3 text-xs">{agentText($locale, 'This session is being deleted. Retry deletion to finish cleanup.')}</p>{/if}
  <FeedbackDeliveryStatus {sessionId} deliveries={$session.snapshot.deliveries} requests={feedbackRequests} {envText} disabled={deletionPending || $session.snapshot.deleting} onResolve={session.resolveDelivery} {onOpenFeedback} />
  {#if error}<div class="flex shrink-0 items-center gap-3 border-b px-5 py-2 text-xs"><p role="alert" class="m-0 min-w-0 flex-1 break-words text-destructive">{error}</p><Button variant="ghost" size="sm" onclick={session.refresh}>{agentText($locale, 'Retry')}</Button></div>{/if}
{:else if showWorkspace}
  <div class="flex h-full flex-col items-center justify-center gap-4 p-6 text-sm text-muted-foreground">
    {#if $session.loading}<LoaderCircle class="size-5 animate-spin" /><span>{agentText($locale, 'Loading agent session…')}</span>{/if}
    {#if error}<p role="alert" class="m-0 max-w-lg break-words text-destructive">{error}</p><Button variant="outline" size="sm" onclick={session.refresh}>{agentText($locale, 'Retry')}</Button>{/if}
  </div>
{:else if error}
  <div class="flex shrink-0 items-center gap-3 border-b px-5 py-2 text-xs"><p role="alert" class="m-0 min-w-0 flex-1 break-words text-destructive">{error}</p><Button variant="ghost" size="sm" onclick={session.refresh}>{agentText($locale, 'Retry')}</Button></div>
{/if}
