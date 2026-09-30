<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { TerminalData } from '../../../generated/feedback'
  import type { TerminalWorkbenchController } from '../../terminal/terminalWorkbenchController'
  import TerminalWorkbench from '../../terminal/TerminalWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<TerminalWorkbench data={context.workspace.workbench!.data as TerminalData}
  state={context.state?.type === 'terminal' ? context.state : null}
  runtime={context.host.controller as TerminalWorkbenchController | undefined}
  disabled={context.disabled} readOnly={context.readOnly} onOpenReview={context.host.openExpanded} onQuote={context.host.quote} />
