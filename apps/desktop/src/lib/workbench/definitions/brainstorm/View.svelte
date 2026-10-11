<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { BrainstormData } from '../../../generated/feedback'
  import BrainstormWorkbench from '../../brainstorm/BrainstormWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>

<BrainstormWorkbench data={context.workspace.workbench!.data as BrainstormData}
  state={context.state?.type === 'brainstorm' ? context.state : null} requestId={context.host.requestId}
  disabled={context.disabled} readOnly={context.readOnly} onChange={context.host.updateState}
  onOpenFeedback={context.host.openFeedback} onOpenExpanded={context.host.openExpanded} />
