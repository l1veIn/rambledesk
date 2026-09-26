<!-- Ramble owns its entire workbench column; shared materials remain secondary. -->
<script lang="ts">
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import type { HostProfile } from '../domain/hostProfile'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import MarkdownPreview from '../editor/MarkdownPreview.svelte'
  import LinkifiedText from '../editor/LinkifiedText.svelte'
  import TaskBriefPanel from './TaskBriefPanel.svelte'
  import WorkspaceHeader from './WorkspaceHeader.svelte'

  export let workspace: FeedbackWorkspaceView
  export let transport: ApplicationTransport
  export let capabilities: Pick<WorkbenchCapabilities, 'serverPaths'>
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let readOnly = false
  export let cooking = false
  export let activeActionId: string | null = null
  export let onSelectAction: (actionId: string, actionIndex: number, title: string) => void = () => {}
  $: closed = readOnly || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  const tr = (source: string) => t($locale, source)
</script>

<div class="flex h-full min-h-0 min-w-0 flex-col" data-workbench="ramble">
  <WorkspaceHeader {workspace} {resolveHostProfile} {cooking} />
  <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain p-5" data-workbench-content>
    <h2 class="m-0 text-xs font-semibold text-muted-foreground">{tr('What happened')}</h2>
    <div class="mb-6 mt-2 text-sm leading-6"><MarkdownPreview markdown={workspace.request.what_happened} bare /></div>
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
  <TaskBriefPanel {transport} {capabilities} {workspace} showContext={false} />
</div>
