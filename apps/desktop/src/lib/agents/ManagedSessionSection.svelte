<script lang="ts">
  import { onMount } from 'svelte'
  import { LoaderCircle } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import { locale } from '$lib/preferences'
  import { agentText } from './agentI18n'
  import { redactAgentMessage } from './agentConfigForm'
  import { createAgentSettingsController } from './agentSettingsController'
  import { createManagedSessionController } from './managedSessionController'
  import { createManagedWorkspaceInfoController } from './managedWorkspaceInfoController'
  import ManagedSessionWorkspace from './ManagedSessionWorkspace.svelte'

  export let transport: ApplicationTransport
  export let sessionId: string
  export let onOpenRamble: (() => Promise<void> | void) | undefined = undefined
  export let deletionPending = false
  export let onDeletingChange: (sessionId: string, deleting: boolean) => void = () => {}
  let reportedDeleting: boolean | undefined
  let wasDeletionPending = false

  // The parent keys this component by local session ID; handlers never follow another tab's selection.
  const session = createManagedSessionController(transport, sessionId, { autoConnectBlocked: () => deletionPending })
  const workspaceInfo = createManagedWorkspaceInfoController(transport, sessionId)
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

  async function refresh() {
    session.refresh()
    await settings.refresh()
  }

  onMount(() => {
    const stopSession = session.start()
    const stopSettings = settings.start()
    const stopWorkspaceInfo = workspaceInfo.start()
    return () => { stopSession(); stopSettings(); stopWorkspaceInfo() }
  })
</script>

{#if $session.snapshot}
  <ManagedSessionWorkspace
    snapshot={$session.snapshot}
    activities={$session.snapshot.activities}
    historyLoading={$session.historyLoading}
    historyHasMore={$session.historyHasMore}
    historyError={$session.historyError}
    onLoadOlder={session.loadOlder}
    permissions={$session.snapshot.permissions}
    recovery={$session.snapshot.recovery}
    connecting={$session.connecting}
    connectionError={$session.connectionError}
    branch={$workspaceInfo?.branch ?? null}
    {config}
    {error}
    busy={deletionPending}
    onPrompt={session.prompt}
    onSetConfiguration={session.setConfiguration}
    onCancel={session.cancel}
    onStart={session.startAgent}
    onRefresh={refresh}
    onRespondPermission={session.respondPermission}
    {onOpenRamble}
  />
{:else}
  <div class="flex h-full flex-col items-center justify-center gap-4 p-6 text-sm text-muted-foreground">
    {#if $session.loading}<LoaderCircle class="size-5 animate-spin" /><span>{agentText($locale, 'Loading agent session…')}</span>{/if}
    {#if error}<p role="alert" class="m-0 max-w-lg break-words text-destructive">{error}</p><Button variant="outline" size="sm" onclick={refresh}>{agentText($locale, 'Retry')}</Button>{/if}
  </div>
{/if}
