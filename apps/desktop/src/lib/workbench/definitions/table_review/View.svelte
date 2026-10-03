<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { TableReviewData } from '../../../generated/feedback'
  import TableReviewWorkbench from '../../table-review/TableReviewWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<TableReviewWorkbench data={context.workspace.workbench!.data as TableReviewData}
  state={context.state?.type === 'table_review' ? context.state : null} requestId={context.host.requestId}
  disabled={context.disabled} readOnly={context.readOnly} onChange={context.host.updateState} onOpenExpanded={context.host.openExpanded} />
