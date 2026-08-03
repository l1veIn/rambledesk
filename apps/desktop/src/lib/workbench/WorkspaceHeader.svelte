<script lang="ts">
  import { RefreshCw } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import { requestStatusLabel } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { HostProfile } from './types'

  export let workspace: FeedbackWorkspaceView
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let cooking = false
  export let disabled = false
  export let onReload: () => void = () => {}

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

<header class="flex min-h-16 shrink-0 items-center gap-4 border-b px-5 py-3">
  <div class="min-w-0 flex-1">
    <div class="flex min-w-0 flex-wrap items-center gap-2 text-[10px] text-muted-foreground">
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
      <Badge variant="outline" class={['h-5 px-1.5 text-[9px]', statusClass()]}>
        {cooking ? tr('Cooking 中') : requestStatusLabel(workspace.request.status, $locale)}
      </Badge>
    </div>
    <h1 class="m-0 mt-1 truncate text-sm font-semibold">
      {workspace.request.title || tr('未命名请求')}
    </h1>
  </div>

  <Button
    variant="ghost"
    size="icon-sm"
    aria-label={tr('重新载入')}
    title={tr('重新载入')}
    {disabled}
    onclick={onReload}
  >
    <RefreshCw />
  </Button>
</header>
