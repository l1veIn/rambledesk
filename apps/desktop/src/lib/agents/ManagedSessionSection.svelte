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

  export let transport: ApplicationTransport
  export let sessionId: string
  export let feedbackRequests: readonly FeedbackRequestSummary[] = []
  export let onOpenFeedback: (requestId: string) => Promise<void> | void
  export let onDelete: (() => Promise<void> | void) | undefined = undefined

  // The parent keys this component by local session ID; handlers never follow another tab's selection.
  const session = createManagedSessionController(transport, sessionId)
  const settings = createAgentSettingsController(transport)
  $: management = $session.snapshot?.session.management
  $: config = management?.kind === 'managed'
    ? $settings.configs.find((candidate) => candidate.id === management.agent_config_id) ?? null
    : null
  $: error = redactAgentMessage($session.error || $settings.error, Object.entries(config?.env ?? {}).map(([key, value]) => `${key}=${value}`).join('\n'))

  onMount(() => {
    const stopSession = session.start()
    const stopSettings = settings.start()
    return () => { stopSession(); stopSettings() }
  })
</script>

{#if $session.snapshot}
  <ManagedSessionWorkspace
    snapshot={$session.snapshot}
    activities={$session.snapshot.activities}
    permissions={$session.snapshot.permissions}
    {feedbackRequests}
    {config}
    {error}
    onPrompt={session.prompt}
    onCancel={session.cancel}
    onStart={session.startAgent}
    onStop={session.stopAgent}
    onRespondPermission={session.respondPermission}
    {onDelete}
    {onOpenFeedback}
  />
{:else}
  <div class="flex h-full flex-col items-center justify-center gap-4 p-6 text-sm text-muted-foreground">
    {#if $session.loading}<LoaderCircle class="size-5 animate-spin" /><span>{agentText($locale, 'Loading agent session…')}</span>{/if}
    {#if error}<p role="alert" class="m-0 max-w-lg break-words text-destructive">{error}</p><Button variant="outline" size="sm" onclick={session.refresh}>{agentText($locale, 'Retry')}</Button>{/if}
  </div>
{/if}
