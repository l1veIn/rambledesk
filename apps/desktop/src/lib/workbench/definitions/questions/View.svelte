<script lang="ts">
  import type { WorkbenchViewContext } from '../contracts'
  import type { QuestionsData } from '../../../generated/feedback'
  import QuestionnaireWorkbench from '../../QuestionnaireWorkbench.svelte'
  export let context: WorkbenchViewContext
</script>
<QuestionnaireWorkbench data={context.workspace.workbench!.data as QuestionsData}
  answers={context.state?.type === 'questions' ? context.state.answers : []} disabled={context.disabled}
  onChange={(answers) => context.host.updateState({ type: 'questions', answers })} />
