<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import RambleWorkbench from '../../RambleWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<RambleWorkbench workspace={context.workspace} readOnly={context.disabled} activeActionId={context.activeActionId} onSelectAction={context.host.selectAction} />
