<script lang="ts">
  import { tick } from 'svelte'
  import type { QuestionsData, QuestionAnswer } from '../generated/feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import WorkbenchTextField from '../input/WorkbenchTextField.svelte'
  import { questionAnswerVoiceTarget, unavailableVoiceInputState, useVoiceInput } from '../speech/voiceInputContext'
  import type { SpeechTarget } from '../speech/speechDraftQueue'
  import FieldAttachments from '../input/FieldAttachments.svelte'
  import { fieldAttachmentText } from '../input/fieldAttachmentText'
  import { unavailableInputToolsState, useInputTools } from '../input/inputToolsContext'

  export let data: QuestionsData
  export let answers: QuestionAnswer[] = []
  export let disabled = false
  export let onChange: (answers: QuestionAnswer[]) => void
  let step = 0
  let root: HTMLElement
  let revealedSequence: number | undefined
  const voice = useVoiceInput()
  const voiceState = voice?.state ?? unavailableVoiceInputState
  const tools = useInputTools()
  const toolsState = tools?.state ?? unavailableInputToolsState
  async function navigate(next: number) {
    step = next
    await tick()
    const target = root?.querySelector<HTMLElement>('[data-answer-option]') ?? root?.querySelector<HTMLElement>('h3')
    target?.focus()
  }
  $: question = data.questions[step]
  $: current = question ? answers.find((answer) => answer.id === question.id) : undefined
  $: answeredCount = data.questions.filter((question) => answers.some((answer) => answer.id === question.id && answer.value.trim())).length
  $: voiceTarget = question?.allowOther && current?.wasCustom ? questionAnswerVoiceTarget($voiceState, question.id, question.label || question.prompt) : null
  $: void revealVoiceTarget($voiceState.revealSequence, $voiceState.revealTarget, answers)
  function tr(source: string, values: Record<string, string | number> = {}) { return t($locale, source, values) }
  function save(answer: QuestionAnswer, advance = false) {
    if (disabled) return
    answers = [...answers.filter((existing) => existing.id !== answer.id), answer]
    onChange(answers)
    if (advance) void navigate(Math.min(step + 1, data.questions.length))
  }
  function clear(id: string) { if (!disabled) { answers = answers.filter((answer) => answer.id !== id); onChange(answers) } }
  function saveCustom(value: string) {
    if (!question) return
    save({ id: question.id, value, label: value, wasCustom: true })
  }
  async function revealVoiceTarget(sequence: number | undefined, target: SpeechTarget | null | undefined, currentAnswers: QuestionAnswer[]) {
    if (sequence === undefined || sequence === revealedSequence || target?.requestId !== $voiceState.requestId || target?.destination.kind !== 'question_answer') return
    const destination = target.destination
    const index = data.questions.findIndex((item) => item.id === destination.questionId)
    if (index < 0) return
    revealedSequence = sequence
    await navigate(index)
    if (data.questions[index].allowOther && currentAnswers.some((answer) => answer.id === destination.questionId && answer.wasCustom)) {
      root?.querySelector<HTMLElement>('[data-question-answer]')?.focus({ preventScroll: true })
    }
  }
</script>

<section bind:this={root} aria-label={tr('Questionnaire')} class="grid gap-4">
  <div class="flex flex-wrap items-center gap-2" aria-label={tr('Question navigation')}>
    {#each data.questions as item, index (item.id)}
      <button type="button" aria-current={step === index ? 'step' : undefined}
        onclick={() => void navigate(index)}
        class={`rounded-md border px-3 py-1.5 text-xs ${step === index ? 'border-primary bg-primary/10 text-primary' : 'bg-background'}`}>
        {answers.some((answer) => answer.id === item.id && answer.value.trim()) ? '✓' : index + 1} {item.label ?? `Q${index + 1}`}
      </button>
    {/each}
    <button type="button" aria-current={step === data.questions.length ? 'step' : undefined}
      onclick={() => void navigate(data.questions.length)} class="rounded-md border px-3 py-1.5 text-xs">{tr('Review answers')}</button>
  </div>
  <p class="m-0 text-xs text-muted-foreground" aria-live="polite">{tr('{answered} of {total} answered', { answered: answeredCount, total: data.questions.length })}</p>
  {#if question}
    <h3 tabindex="-1" class="m-0 text-base font-medium leading-6">{question.prompt}</h3>
    <div class="grid gap-2" aria-label={question.prompt}>
      {#each question.options as option, index (option.value)}
        <button type="button" data-answer-option disabled={disabled} aria-pressed={current?.wasCustom === false && current.value === option.value}
          onclick={() => save({ id: question.id, value: option.value, label: option.label, wasCustom: false, index: index + 1 }, true)}
          class="flex items-start gap-3 rounded-lg border bg-background p-3 text-left transition-colors hover:border-primary/60 focus-visible:ring-2 focus-visible:ring-ring aria-pressed:border-primary aria-pressed:bg-primary/5 disabled:opacity-60">
          <span class="grid size-5 shrink-0 place-items-center rounded border text-xs text-muted-foreground">{index + 1}</span>
          <span class="grid gap-1"><strong class="text-sm font-medium">{option.label}</strong>{#if option.description}<span class="text-xs leading-5 text-muted-foreground">{option.description}</span>{/if}</span>
        </button>
      {/each}
      {#if question.allowOther}
        <button type="button" disabled={disabled} aria-pressed={current?.wasCustom === true}
          onclick={() => { if (!current?.wasCustom) save({ id: question.id, value: '', label: '', wasCustom: true }) }}
          class="rounded-lg border border-dashed p-3 text-left text-sm aria-pressed:border-primary aria-pressed:bg-primary/5">{tr('Other — write your answer')}</button>
      {/if}
    </div>
    {#if current?.wasCustom}
      {#key question.id}
      <WorkbenchTextField value={current.value} target={voiceTarget} {disabled} maxLength={4000}
        label={tr('Your answer')} voiceLabel="Speak answer" editorClass="p-3"
        data-question-answer={question.id} onChange={saveCustom} />
      {/key}
    {/if}
    <div class="flex items-center gap-3">
      <button type="button" onclick={() => void navigate(step - 1)} disabled={step === 0} class="rounded-md border px-3 py-2 text-xs disabled:opacity-40">{tr('Previous question')}</button>
      <button type="button" onclick={() => void navigate(step + 1)} disabled={!current?.value.trim()} class="rounded-md bg-primary px-3 py-2 text-xs text-primary-foreground disabled:opacity-40">{step === data.questions.length - 1 ? tr('Review answers') : tr('Next question')}</button>
      <button type="button" onclick={() => clear(question.id)} disabled={disabled || !current} class="ml-auto text-xs underline disabled:opacity-40">{tr('Clear answer')}</button>
    </div>
    <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Choose an option to continue. You can return to any question before submitting.')}</p>
  {:else}
    <h3 tabindex="-1" class="m-0 text-base font-medium">{tr('Review answers')}</h3>
    <div class="grid gap-2">
      {#each data.questions as item, index (item.id)}
        {@const answer = answers.find((answer) => answer.id === item.id)}
        <div class="overflow-hidden rounded-lg border bg-background">
          <button type="button" onclick={() => void navigate(index)} class="grid w-full gap-1 p-3 text-left">
            <span class="text-xs text-muted-foreground">{item.prompt}</span>
            <span class={`text-sm ${answer?.value.trim() ? '' : 'text-amber-600'}`}>{answer?.value.trim() ? fieldAttachmentText(answer.label, $toolsState.attachments).text || tr('Attachments') : tr('Unanswered')}</span>
          </button>
          {#if answer?.wasCustom}<FieldAttachments value={answer.value} {disabled} onChange={(value) => save({ ...answer, value, label: value })} />{/if}
        </div>
      {/each}
    </div>
    <p class="m-0 text-xs leading-5 text-muted-foreground">{answeredCount === data.questions.length ? tr('All answers are ready. Submit when you have finished reviewing.') : tr('Answer every question before submitting. Notes are optional.')}</p>
  {/if}
</section>
