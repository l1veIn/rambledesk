<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { WebReviewData } from '../../../generated/feedback'
  import WebReviewWorkbench from '../../web-review/WebReviewWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<WebReviewWorkbench data={context.workspace.workbench!.data as WebReviewData}
  state={context.state?.type === 'web_review' ? context.state : null} disabled={context.disabled} readOnly={context.readOnly}
  onChange={context.host.updateState} onOpenReview={context.host.openExpanded} />
