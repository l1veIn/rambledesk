<script lang="ts">
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import type { HostProfile } from '$lib/domain/hostProfile'
  import QuestionnaireWorkbench from './QuestionnaireWorkbench.svelte'
  import type { DocumentReviewData, SingleChoiceData, QuestionsData, WorkbenchState } from '$lib/generated/feedback'
  import { resolveWorkbenchPolicy } from '../workbenchPolicy'
  import DocumentReviewWorkbench from './document-review/DocumentReviewWorkbench.svelte'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import RambleWorkbench from './RambleWorkbench.svelte'
  import TaskBriefPanel from './TaskBriefPanel.svelte'
  import WorkspaceHeader from './WorkspaceHeader.svelte'

  export let workspace: FeedbackWorkspaceView
  export let transport: ApplicationTransport
  export let capabilities: Pick<WorkbenchCapabilities, 'serverPaths'>
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let readOnly = false
  export let cooking = false
  export let activeActionId: string | null = null
  export let state: WorkbenchState | null = null
  export let onSelectAction: (id: string, index: number, title: string) => void = () => {}
  export let onStateChange: (state: WorkbenchState) => void = () => {}

  $: spec = workspace.workbench
  $: kind = resolveWorkbenchPolicy(spec)?.type ?? 'unsupported'
  $: choice = kind === 'single_choice' && spec ? spec.data as SingleChoiceData : null
  $: questions = kind === 'questions' && spec ? spec.data as QuestionsData : null
  $: review = kind === 'document_review' && spec ? spec.data as DocumentReviewData : null
  $: selectedOptionId = state?.type === 'single_choice' ? state.selected_option_id : null
  $: closed = readOnly || kind === 'unsupported' || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  const tr = (source: string) => t($locale, source)
  function select(id: string | null) {
    if (closed) return
    onStateChange({ type: 'single_choice', selected_option_id: id })
  }
</script>

{#if kind === 'ramble'}
  <RambleWorkbench {workspace} {transport} {capabilities} {resolveHostProfile} readOnly={closed} {cooking} {activeActionId} {onSelectAction} />
{:else}
  <div class="flex h-full min-h-0 min-w-0 flex-col" data-workbench={kind}>
    <WorkspaceHeader {workspace} {resolveHostProfile} {cooking} />
    <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain p-5" data-workbench-content>
      {#if questions}
        {@render questionnaire()}
      {:else if choice}
        {@render choices()}
      {:else if review}
        {#key workspace.request.request_id}
          <DocumentReviewWorkbench data={review} state={state?.type === 'document_review' ? state : { type: 'document_review', verdict: null, annotations: [], paragraph_marks: [] }} disabled={closed} onChange={onStateChange} />
        {/key}
      {:else}
        <p class="m-0 text-sm leading-6 text-muted-foreground" role="status">{tr('This workbench is unavailable. Your draft and materials are preserved in read-only mode. Open this request in a compatible client to continue.')}</p>
      {/if}
    </div>
    <TaskBriefPanel {workspace} {transport} {capabilities} />
  </div>
{/if}

{#snippet choices()}
  {#if choice}
    <fieldset disabled={closed} class="m-0 grid gap-3 border-0 p-0">
      <legend class="mb-3 text-sm font-medium">{choice.prompt}</legend>
      {#each choice.options as option (option.id)}
        <label class={`flex cursor-pointer items-start gap-3 rounded-lg border p-3 text-sm ${selectedOptionId === option.id ? 'border-primary bg-primary/5' : 'bg-background'}`}>
          <input type="radio" name={`workbench-${workspace.request.request_id}`} value={option.id} checked={selectedOptionId === option.id} onchange={() => select(option.id)} class="mt-0.5" />
          <span>{option.label}</span>
        </label>
      {/each}
      <button type="button" disabled={!selectedOptionId || closed} onclick={() => select(null)} class="justify-self-start text-xs underline disabled:opacity-40">{tr('Clear selection')}</button>
      <p class="m-0 text-xs text-muted-foreground">{tr('Your selection is saved automatically. Additional notes are optional.')}</p>
    </fieldset>
  {/if}
{/snippet}

{#snippet questionnaire()}
  {#if questions}
    {#key workspace.request.request_id}
      <QuestionnaireWorkbench data={questions} answers={state?.type === 'questions' ? state.answers : []} disabled={closed}
        onChange={(answers) => onStateChange({ type: 'questions', answers })} />
    {/key}
  {/if}
{/snippet}
