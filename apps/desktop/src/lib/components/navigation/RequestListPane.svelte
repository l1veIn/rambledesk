<script lang="ts">
  import { ChevronDown, FileText, Inbox, LoaderCircle, PanelLeftClose, PanelLeftOpen } from '@lucide/svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import type { FeedbackRequestSummary } from '$lib/feedback'
  import { requestStatusLabel } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { HostProfile } from '$lib/workbench/types'
  import { DEFAULT_REQUEST_FILTERS, requestFilterCount, type RequestFilters } from '$lib/workbench/requestFilters'
  import RequestFilterPopover from './RequestFilterPopover.svelte'

  export let requests: FeedbackRequestSummary[] = []
  export let activeRequestId: string | null = null
  export let cookingRequestIds: ReadonlySet<string> = new Set()
  export let scopeLabel = ''
  export let searchQuery = ''
  export let loading = false
  export let refreshing = false
  export let loadingMore = false
  export let hasMore = false
  export let collapsed = false
  export let filters: RequestFilters = DEFAULT_REQUEST_FILTERS
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let formatTime: (value: string | null | undefined) => string
  export let onLoadMore: () => void = () => {}
  export let onOpenRequest: (requestId: string) => void = () => {}
  export let onFiltersChange: (filters: RequestFilters) => void = () => {}

  $: filtered = requestFilterCount(filters) > 0
  $: busy = loading || refreshing

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function statusClass(status: FeedbackRequestSummary['status'] | 'cooking') {
    switch (status) {
      case 'cooking':
        return 'bg-primary/15 text-primary'
      case 'waiting':
        return 'bg-warning/15 text-warning-foreground dark:text-warning'
      case 'in_progress':
        return 'bg-info/15 text-info'
      case 'completed':
        return 'bg-success/15 text-success'
      case 'cancelled':
        return 'bg-destructive/12 text-destructive'
    }
  }
</script>

<aside
  class="flex h-full min-h-0 flex-col bg-background"
  aria-label={tr('Request list')}
>
  <div class={['flex h-12 shrink-0 items-center gap-1.5 border-b', collapsed ? 'justify-center px-2' : 'px-3']}>
    {#if !collapsed}
      <div class="min-w-0 flex-1">
        <strong class="flex items-center gap-1.5 text-xs font-semibold">
          {tr('Requests')}
          {#if requests.length > 0}
            <Badge variant="secondary" class="h-4 rounded-full px-1.5 text-[9px] font-medium tabular-nums">
              {requests.length}{hasMore ? '+' : ''}
            </Badge>
          {/if}
        </strong>
        <span class="block truncate text-[10px] text-muted-foreground">{scopeLabel}</span>
      </div>
      <RequestFilterPopover {filters} onChange={onFiltersChange} />
    {/if}
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={collapsed ? tr('Expand request list') : tr('Collapse request list')}
      title={collapsed ? tr('Expand request list') : tr('Collapse request list')}
      aria-expanded={!collapsed}
      onclick={() => (collapsed = !collapsed)}
    >
      {#if collapsed}<PanelLeftOpen />{:else}<PanelLeftClose />{/if}
    </Button>
  </div>

  {#if collapsed}
    <div class="flex flex-col items-center gap-2 border-b py-2">
      <RequestFilterPopover {filters} {collapsed} onChange={onFiltersChange} />
      <span class="text-[10px] tabular-nums text-muted-foreground" title={tr('Requests')}>
        {requests.length}{hasMore ? '+' : ''}
      </span>
    </div>
  {/if}
  <ScrollArea class="min-h-0 flex-1" aria-busy={busy}>
    <div class="relative min-h-full">
      <div class={busy ? 'pointer-events-none select-none opacity-40' : undefined} inert={busy}>
        {#if collapsed}
          <nav class="flex flex-col items-center gap-1 py-2" aria-label={tr('Requests')}>
            {#each requests as request (request.request_id)}
              {@const displayStatus = cookingRequestIds.has(request.request_id) ? 'cooking' : request.status}
              <button
                type="button"
                class={['relative grid size-9 shrink-0 place-items-center rounded-md transition-colors hover:bg-muted', activeRequestId === request.request_id && 'bg-accent text-accent-foreground']}
                aria-label={request.title}
                aria-current={activeRequestId === request.request_id ? 'true' : undefined}
                title={`${request.title} · ${displayStatus === 'cooking' ? tr('Cooking') : requestStatusLabel(displayStatus, $locale)}`}
                onclick={() => onOpenRequest(request.request_id)}
              >
                <span class={['grid size-6 place-items-center rounded', statusClass(displayStatus)]}>
                  <FileText class="size-3.5" />
                </span>
                {#if activeRequestId === request.request_id}
                  <span class="absolute inset-y-2 left-0 w-0.5 rounded-full bg-primary"></span>
                {/if}
              </button>
            {/each}
          </nav>
        {:else if loading && requests.length === 0}
          <div class="space-y-2 p-2">
            {#each Array(6) as _}
              <div class="space-y-2 border-b px-2 py-3">
                <Skeleton class="h-3 w-3/4" />
                <Skeleton class="h-2.5 w-full" />
                <Skeleton class="h-2.5 w-1/2" />
              </div>
            {/each}
          </div>
        {:else if requests.length === 0}
          <div class="grid place-items-center gap-2 px-6 py-16 text-center">
            <div class="grid size-9 place-items-center rounded-md bg-muted text-muted-foreground">
              <Inbox class="size-4" />
            </div>
            <strong class="text-xs">
              {searchQuery.trim() || filtered
                ? tr('No matching requests')
                : tr('No requests in this scope')}
            </strong>
            <span class="text-[11px] leading-5 text-muted-foreground">
              {filtered ? tr('Try changing or resetting your filters.') : tr('New requests appear here by most recent update.')}
            </span>
            {#if filtered}
              <Button variant="ghost" size="sm" onclick={() => onFiltersChange(DEFAULT_REQUEST_FILTERS)}>{tr('Reset filters')}</Button>
            {/if}
          </div>
        {:else}
          <nav class="p-2" aria-label={tr('Requests')}>
            {#each requests as request (request.request_id)}
              {@const profile = resolveHostProfile(request.host_id)}
              {@const displayStatus = cookingRequestIds.has(request.request_id) ? 'cooking' : request.status}
              <button
                type="button"
                aria-current={activeRequestId === request.request_id ? 'true' : undefined}
                class={[
                  'group relative flex w-full flex-col gap-1.5 border-b px-2.5 py-3 text-left transition-colors last:border-b-0',
                  activeRequestId === request.request_id
                    ? 'bg-accent text-accent-foreground'
                    : 'hover:bg-muted/75',
                ]}
                onclick={() => onOpenRequest(request.request_id)}
              >
                {#if activeRequestId === request.request_id}
                  <span class="absolute inset-y-2 left-0 w-0.5 rounded-full bg-primary"></span>
                {/if}
                <div class="flex w-full items-center gap-2">
                  <strong class="min-w-0 flex-1 truncate text-xs font-medium">{request.title}</strong>
                  <Badge
                    variant="secondary"
                    class={['h-5 shrink-0 border-0 px-1.5 text-[9px]', statusClass(displayStatus)]}
                  >
                    {displayStatus === 'cooking'
                      ? tr('Cooking')
                      : requestStatusLabel(displayStatus, $locale)}
                  </Badge>
                </div>
                <p class="m-0 line-clamp-2 text-[11px] leading-4 text-muted-foreground">
                  {request.what_happened}
                </p>
                <div class="flex w-full items-center gap-1.5 text-[9px] text-muted-foreground">
                  <span class="grid size-5 shrink-0 place-items-center [&_svg]:size-4">
                    {@html profile.icon_svg}
                  </span>
                  <span class="min-w-0 flex-1 truncate">
                    {request.source_hint ?? request.host_session_id}
                  </span>
                  <span class="shrink-0 tabular-nums">{formatTime(request.updated_at)}</span>
                </div>
              </button>
            {/each}
          </nav>
        {/if}
        {#if hasMore}
          <div class="border-t p-2">
            <Button
              variant="ghost"
              size={collapsed ? 'icon-sm' : 'sm'}
              class="w-full"
              disabled={loadingMore}
              onclick={onLoadMore}
              aria-label={loadingMore ? tr('Loading…') : tr('Load more')}
              title={tr('Load more')}
            >
              <ChevronDown data-icon="inline-start" />
              {#if !collapsed}{loadingMore ? tr('Loading…') : tr('Load more')}{/if}
            </Button>
          </div>
        {/if}
      </div>
      {#if busy && (requests.length > 0 || collapsed)}
        <div class="absolute inset-0 z-20 grid place-items-center bg-background/80 backdrop-blur-[1px]">
          <LoaderCircle class="size-5 animate-spin text-primary" aria-hidden="true" />
        </div>
      {/if}
    </div>
  </ScrollArea>
</aside>
