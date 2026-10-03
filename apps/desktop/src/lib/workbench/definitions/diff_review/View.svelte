<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { DiffReviewData } from '../../../generated/feedback'
  import DiffReviewWorkbench from '../../diff-review/DiffReviewWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<DiffReviewWorkbench data={context.workspace.workbench!.data as DiffReviewData}
  state={context.state?.type === 'diff_review' ? context.state : null} requestId={context.host.requestId}
  disabled={context.disabled} readOnly={context.readOnly} onChange={context.host.updateState} onOpenExpanded={context.host.openExpanded} />
