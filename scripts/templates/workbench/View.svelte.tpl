<script lang="ts">
  import { onMount } from 'svelte'
  import type { __Pascal__Data } from '../../../generated/feedback'
  import type { WorkbenchViewContext } from '../contracts'
  import WorkbenchTextField from '../../../input/WorkbenchTextField.svelte'
  import { unavailableVoiceInputState, useVoiceInput, workbenchFieldVoiceTarget } from '../../../speech/voiceInputContext'
  import { __camel__MaterialVersion } from './definition'
  export let context: WorkbenchViewContext
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  $: data = context.workspace.workbench!.data as __Pascal__Data
  $: state = context.state?.type === '__snake__' ? context.state : { type: '__snake__' as const, score: null, note: '' }
  $: target = workbenchFieldVoiceTarget($voiceState, { workbenchType: '__snake__', version: 1,
    field: 'opinion', entityId: 'review', sourceVersion: __camel__MaterialVersion(data), label: '意见' })
  onMount(() => { if (!context.state && !context.disabled && !context.readOnly) context.host.updateState({ type: '__snake__', score: null, note: '' }) })
</script>

<section class="mx-auto grid w-full max-w-3xl gap-5" data-rating-review>
  <div class="flex items-center gap-3"><h2 class="m-0 flex-1 text-base font-semibold">{data.title}</h2>
    {#if context.host.openExpanded}<button type="button" class="rounded-md border px-3 py-1.5 text-xs" onclick={context.host.openExpanded}>全屏评审</button>{/if}
  </div>
  <p class="m-0 whitespace-pre-wrap text-sm leading-7">{data.material}</p>
  <fieldset class="m-0 border-0 p-0" disabled={context.disabled || context.readOnly}>
    <legend class="mb-2 text-xs">评分</legend>
    <div class="flex gap-2">{#each [1, 2, 3, 4, 5] as score}
      <button type="button" aria-label={`${score} 分`} aria-pressed={state.score === score}
        class="size-10 rounded-md border text-sm aria-pressed:border-primary aria-pressed:bg-primary/10"
        onclick={() => context.host.updateState({ ...state, score })}>{score}</button>
    {/each}</div>
  </fieldset>
  <WorkbenchTextField value={state.note} label="意见（可选）" voiceLabel="为意见录音" {target} maxLength={4000}
    disabled={context.disabled || context.readOnly} onChange={(note) => context.host.updateState({ ...state, note })} />
</section>
