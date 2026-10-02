<script lang="ts">
  import type { Writable } from 'svelte/store'
  import type { VisualFeedbackData } from '../../generated/feedback'
  import type { VisualState } from './visualModel'
  import VisualFeedbackWorkbench from './VisualFeedbackWorkbench.svelte'
  export let state: Writable<VisualState | null>
  export let data: VisualFeedbackData
  export let readOnly = false
  export let disabled = false
  export let onChange: (state: VisualState) => void
</script>
<VisualFeedbackWorkbench {data} state={$state} {readOnly} {disabled} {onChange} />
