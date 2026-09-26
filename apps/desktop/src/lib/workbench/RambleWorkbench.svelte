<!-- Ramble owns the action list; RegisteredWorkbench supplies the shared context and shell. -->
<script lang="ts">
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import LinkifiedText from '../editor/LinkifiedText.svelte'

  export let workspace: FeedbackWorkspaceView
  export let readOnly = false
  export let activeActionId: string | null = null
  export let onSelectAction: (actionId: string, actionIndex: number, title: string) => void = () => {}
  $: closed = readOnly || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  const tr = (source: string) => t($locale, source)
</script>

<div data-tour="ramble-actions">
  <h2 class="m-0 text-xs font-semibold text-muted-foreground">{tr('Actions to experience')}</h2>
  <ol class="mt-3 grid list-none gap-3 p-0">
    {#each workspace.actions as action, index (action.id)}
      <li><button type="button" disabled={closed} aria-pressed={activeActionId === action.id}
        onclick={() => onSelectAction(action.id, index, action.instruction)}
        class={`grid w-full grid-cols-[24px_minmax(0,1fr)] gap-3 rounded-lg border p-3 text-left text-sm leading-6 hover:bg-accent/60 disabled:cursor-not-allowed disabled:opacity-60 ${activeActionId === action.id ? 'border-primary bg-primary/5' : 'bg-background'}`}>
        <span class="grid size-6 place-items-center rounded-md bg-muted text-xs">{index + 1}</span><span><LinkifiedText text={action.instruction} /></span>
      </button></li>
    {/each}
  </ol>
</div>
