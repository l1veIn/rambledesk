<script lang="ts">
  import type { Snippet } from 'svelte'
  import { Badge } from '$lib/components/ui/badge'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import { requestStatusLabel } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { HostProfile } from './types'

  export let workspace: FeedbackWorkspaceView
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let cooking = false
  export let agentStatus: Snippet | undefined = undefined

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function statusClass() {
    if (cooking) return 'border-primary/25 bg-primary/10 text-primary'
    switch (workspace.request.status) {
      case 'waiting':
        return 'border-warning/25 bg-warning/10 text-warning-foreground dark:text-warning'
      case 'in_progress':
        return 'border-info/25 bg-info/10 text-info'
      case 'completed':
        return 'border-success/25 bg-success/10 text-success'
      case 'cancelled':
        return 'border-destructive/25 bg-destructive/10 text-destructive'
    }
  }
</script>

<header class="workspace-header shrink-0 border-b" class:has-agent-status={!!agentStatus}>
  <div class="flex min-h-14 min-w-0 flex-col justify-center px-4 py-2">
    <div class="flex min-w-0 items-center gap-2 overflow-hidden whitespace-nowrap text-[10px] text-muted-foreground">
      <span class="flex min-w-0 items-center gap-1.5">
        <span class="grid size-4 shrink-0 place-items-center [&_svg]:size-3.5">
          {@html resolveHostProfile(workspace.request.host_id).icon_svg}
        </span>
        <span class="truncate font-medium text-foreground">
          {resolveHostProfile(workspace.request.host_id).label}
        </span>
      </span>
      <span aria-hidden="true">/</span>
      <span class="max-w-48 truncate" title={workspace.request.host_session_id}>
        {workspace.request.host_session_id}
      </span>
    </div>
    <div class="mt-1 flex min-w-0 items-center gap-2">
      <Badge variant="outline" class={['h-5 shrink-0 px-1.5 text-[9px]', statusClass()]}>
        {cooking ? tr('Cooking') : requestStatusLabel(workspace.request.status, $locale)}
      </Badge>
      <h1 class="m-0 truncate text-sm font-semibold">
        {workspace.request.title || tr('Untitled request')}
      </h1>
    </div>
  </div>
  {#if agentStatus}
    <div class="agent-status-column min-w-0 border-l bg-muted/15 px-4 py-2">
      {@render agentStatus()}
    </div>
  {/if}
</header>

<style>
  .workspace-header.has-agent-status {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--workspace-rail-width, 288px);
  }
  @media (max-width: 1180px) {
    .workspace-header.has-agent-status { grid-template-columns: minmax(0, 1fr); }
    .agent-status-column { border-left: 0; border-top: 1px solid var(--border); }
  }
</style>
