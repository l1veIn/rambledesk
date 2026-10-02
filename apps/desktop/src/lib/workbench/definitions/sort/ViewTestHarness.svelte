<script lang="ts">
  import type { Readable } from 'svelte/store'
  import type { WorkbenchViewContext } from '../contracts'
  import View from './View.svelte'
  export let context: Readable<WorkbenchViewContext>
</script>
<View context={$context} />
