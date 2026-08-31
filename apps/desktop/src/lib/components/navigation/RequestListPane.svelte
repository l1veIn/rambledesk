<script lang="ts">
  import {
    ChevronDown,
    CircleHelp,
    Inbox,
    LoaderCircle,
    RefreshCw,
    ShieldCheck,
  } from '@lucide/svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { requestStatusLabel } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    requestListItemKindClass,
    requestListItemKindLabel,
    requestListItemStatusClass,
    type RequestListAgentProfile,
    type WorkbenchRequestListItem,
  } from './requestListItem'

  export let requests: WorkbenchRequestListItem[] = []
  export let activeRequestKey: string | null = null
  export let cookingRequestKeys: ReadonlySet<string> = new Set()
  export let scopeLabel = ''
  export let searchQuery = ''
  export let loading = false
  export let refreshing = false
  export let loadingMore = false
  export let hasMore = false
  export let todayOnly = false
  export let resolveAgentProfile: (agentId: string) => RequestListAgentProfile
  export let formatTime: (value: string | null | undefined) => string
  export let onRefresh: () => void = () => {}
  export let onLoadMore: () => void = () => {}
  export let onOpenRequest: (request: WorkbenchRequestListItem) => void = () => {}
  export let onToggleToday: () => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

</script>

<aside
  class="flex h-full min-h-0 flex-col bg-background"
  aria-label={tr('Request list')}
>
  <div class="flex h-12 items-center gap-1.5 border-b px-3">
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
    <Button
      variant={todayOnly ? 'secondary' : 'ghost'}
      size="sm"
      class="h-7 shrink-0 px-2 text-[11px]"
      aria-pressed={todayOnly}
      title={tr('Last 24h')}
      onclick={onToggleToday}
    >
      {tr('Last 24h')}
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      disabled={loading || refreshing}
      aria-label={tr('Refresh sessions and requests')}
      title={tr('Refresh sessions and requests')}
      onclick={onRefresh}
    >
      <RefreshCw class={loading || refreshing ? 'animate-spin' : ''} />
    </Button>
  </div>

  <ScrollArea class="min-h-0 flex-1" aria-busy={refreshing}>
    <div class="relative min-h-full">
      <div class={refreshing ? 'pointer-events-none select-none opacity-40' : undefined}>
        {#if loading && requests.length === 0}
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
              {searchQuery.trim()
                ? tr('No matching requests')
                : todayOnly
                  ? tr('No requests in the last 24 hours')
                  : tr('No requests in this scope')}
            </strong>
            <span class="text-[11px] leading-5 text-muted-foreground">
              {tr('New requests appear here by most recent update.')}
            </span>
          </div>
        {:else}
          <nav class="p-2" aria-label={tr('Requests')}>
            {#each requests as request (request.key)}
              {@const profile = resolveAgentProfile(request.agentId)}
              {@const displayStatus = cookingRequestKeys.has(request.key) ? 'cooking' : request.status}
              <button
                type="button"
                class={[
                  'group relative flex w-full flex-col gap-1.5 border-b px-2.5 py-3 text-left transition-colors last:border-b-0',
                  activeRequestKey === request.key
                    ? 'bg-accent text-accent-foreground'
                    : 'hover:bg-muted/75',
                ]}
                onclick={() => onOpenRequest(request)}
                aria-current={activeRequestKey === request.key ? 'page' : undefined}
              >
                {#if activeRequestKey === request.key}
                  <span class="absolute inset-y-2 left-0 w-0.5 rounded-full bg-primary"></span>
                {/if}
                <div class="flex w-full items-center gap-2">
                  {#if request.kind !== 'feedback'}
                    <span
                      class={[
                        'grid size-5 shrink-0 place-items-center rounded-md',
                        requestListItemKindClass(request.kind),
                      ]}
                      title={tr(requestListItemKindLabel(request.kind))}
                    >
                      {#if request.kind === 'permission'}
                      <ShieldCheck class="size-3" />
                      {:else}
                        <CircleHelp class="size-3" />
                      {/if}
                      <span class="sr-only">{tr(requestListItemKindLabel(request.kind))}</span>
                    </span>
                  {/if}
                  <strong class="min-w-0 flex-1 truncate text-xs font-medium">{request.title}</strong>
                  <Badge
                    variant="secondary"
                    class={['h-5 shrink-0 border-0 px-1.5 text-[9px]', requestListItemStatusClass(displayStatus)]}
                  >
                    {displayStatus === 'cooking'
                      ? tr('Cooking')
                      : requestStatusLabel(displayStatus, $locale)}
                  </Badge>
                </div>
                <p class="m-0 line-clamp-2 text-[11px] leading-4 text-muted-foreground">
                  {request.summary}
                </p>
                <div class="flex w-full items-center gap-1.5 text-[9px] text-muted-foreground">
                  <span class="grid size-5 shrink-0 place-items-center [&_svg]:size-4">
                    {@html profile.iconSvg}
                  </span>
                  <span class="min-w-0 flex-1 truncate">
                    {request.sourceHint ?? request.sessionId}
                  </span>
                  <span class="shrink-0 tabular-nums">{formatTime(request.updatedAt)}</span>
                </div>
              </button>
            {/each}
          </nav>

          {#if hasMore}
            <div class="border-t p-2">
              <Button
                variant="ghost"
                size="sm"
                class="w-full"
                disabled={loadingMore}
                onclick={onLoadMore}
              >
                <ChevronDown data-icon="inline-start" />
                {loadingMore ? tr('Loading…') : tr('Load more')}
              </Button>
            </div>
          {/if}
        {/if}
      </div>
      {#if refreshing && requests.length > 0}
        <div class="absolute inset-0 z-20 grid place-items-center bg-background/80 backdrop-blur-[1px]">
          <LoaderCircle class="size-5 animate-spin text-primary" aria-hidden="true" />
        </div>
      {/if}
    </div>
  </ScrollArea>
</aside>
