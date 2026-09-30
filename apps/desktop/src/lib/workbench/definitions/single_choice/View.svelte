<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { SingleChoiceData } from '../../../generated/feedback'
  import SingleChoiceWorkbench from '../../SingleChoiceWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<SingleChoiceWorkbench data={context.workspace.workbench!.data as SingleChoiceData} disabled={context.disabled}
  selectedOptionId={context.state?.type === 'single_choice' ? context.state.selected_option_id : null}
  onChange={(selected_option_id) => context.host.updateState({ type: 'single_choice', selected_option_id })} />
