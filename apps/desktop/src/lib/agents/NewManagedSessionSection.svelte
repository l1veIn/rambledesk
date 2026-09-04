<script lang="ts">
  import { onMount } from 'svelte'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { CreateManagedSessionInput, ManagedSessionSnapshot } from '$lib/generated/feedback'
  import { createAgentSettingsController } from './agentSettingsController'
  import NewManagedSessionForm from './NewManagedSessionForm.svelte'

  export let transport: ApplicationTransport
  export let onCreated: (snapshot: ManagedSessionSnapshot) => Promise<void> | void
  export let onConfigure: () => void
  export let onCreating: (creating: boolean) => void = () => {}
  const settings = createAgentSettingsController(transport)
  onMount(() => settings.start())

  async function create(input: CreateManagedSessionInput) {
    onCreating(true)
    try {
      const snapshot = await transport.call('createManagedSession', input)
      await onCreated(snapshot)
    } finally { onCreating(false) }
  }
</script>

<NewManagedSessionForm configs={$settings.configs} busy={$settings.loading} error={$settings.error} onCreate={create} {onConfigure} />
