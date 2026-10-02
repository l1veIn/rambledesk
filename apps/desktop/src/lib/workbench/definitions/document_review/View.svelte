<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { DocumentReviewData } from '../../../generated/feedback'
  import DocumentReviewWorkbench from '../../document-review/DocumentReviewWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<DocumentReviewWorkbench data={context.workspace.workbench!.data as DocumentReviewData}
  state={context.state?.type === 'document_review' ? context.state : null} disabled={context.disabled} onChange={context.host.updateState} />
