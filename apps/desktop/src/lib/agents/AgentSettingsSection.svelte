<script lang="ts">
  import AgentCatalog from './AgentCatalog.svelte'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  export let transport: ApplicationTransport
</script>

<AgentCatalog {transport} />
