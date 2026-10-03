<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { MediaReviewData } from '../../../generated/feedback'
  import type { MediaReviewController } from '../../media-review/mediaController'
  import MediaReviewWorkbench from '../../media-review/MediaReviewWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<MediaReviewWorkbench data={context.workspace.workbench!.data as MediaReviewData}
  state={context.state?.type === 'media_review' ? context.state : null} requestId={context.host.requestId}
  runtime={context.host.controller as MediaReviewController | undefined}
  disabled={context.disabled} readOnly={context.readOnly} onChange={context.host.updateState} onOpenExpanded={context.host.openExpanded} />
