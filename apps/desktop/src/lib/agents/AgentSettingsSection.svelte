<script lang="ts">
  import { onMount } from 'svelte'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import { Button } from '$lib/components/ui/button'
  import { locale } from '$lib/preferences'
  import AgentSettings from './AgentSettings.svelte'
  import { createAgentSettingsController } from './agentSettingsController'
  import { agentText } from './agentI18n'

  export let transport: ApplicationTransport

  const controller = createAgentSettingsController(transport)
  onMount(() => controller.start())
</script>

{#if $controller.error}
  <div class="mb-4 flex items-center gap-3">
    <p role="alert" class="m-0 min-w-0 flex-1 break-words text-xs text-destructive">{agentText($locale, $controller.error)}</p>
    <Button variant="outline" size="sm" disabled={$controller.loading} onclick={() => void controller.refresh()}>{agentText($locale, 'Retry')}</Button>
  </div>
{/if}
<AgentSettings
  configs={$controller.configs}
  busy={$controller.loading && $controller.configs.length === 0}
  onSave={controller.save}
  onDelete={controller.remove}
  onCheck={controller.check}
/>
