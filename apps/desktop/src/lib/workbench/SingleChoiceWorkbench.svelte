<script lang="ts">
  import type { QuestionAnswer, QuestionsData, SingleChoiceData } from '../generated/feedback'
  import QuestionnaireWorkbench from './QuestionnaireWorkbench.svelte'

  // Historical single_choice requests keep their wire state; only their view is adapted.
  export let data: SingleChoiceData
  export let selectedOptionId: string | null = null
  export let disabled = false
  export let onChange: (selectedOptionId: string | null) => void
  const questionId = 'selection'
  $: questions = { questions: [{ id: questionId, prompt: data.prompt, allowOther: false,
    options: data.options.map((option) => ({ value: option.id, label: option.label })),
  }] } satisfies QuestionsData
  $: selected = data.options.find((option) => option.id === selectedOptionId)
  $: answers = selected ? [{ id: questionId, value: selected.id, label: selected.label, wasCustom: false }] : []
  function change(next: QuestionAnswer[]) {
    if (disabled) return
    const answer = next.find((item) => item.id === questionId && !item.wasCustom && data.options.some((option) => option.id === item.value))
    onChange(answer?.value ?? null)
  }
</script>

<QuestionnaireWorkbench data={questions} {answers} {disabled} onChange={change} />
