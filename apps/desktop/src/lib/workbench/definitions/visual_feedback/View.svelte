<script lang="ts">
  import type { VisualFeedbackData } from '../../../generated/feedback'
  import type { WorkbenchViewContext } from '../contracts'
  import type { VisualWorkbenchController } from '../../visual-feedback/visualController'
  import VisualFeedbackWorkbench from '../../visual-feedback/VisualFeedbackWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<VisualFeedbackWorkbench data={context.workspace.workbench!.data as VisualFeedbackData}
  state={context.state?.type === 'visual_feedback' ? context.state : null}
  attachments={context.workspace.attachments}
  runtime={context.host.controller as VisualWorkbenchController | undefined}
  disabled={context.disabled} readOnly={context.readOnly} onChange={context.host.updateState} onOpenReview={context.host.openExpanded} />
