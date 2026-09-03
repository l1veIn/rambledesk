<script lang="ts">
  import { Funnel, X } from '@lucide/svelte'
  import { Popover } from 'bits-ui'
  import { Button } from '$lib/components/ui/button'
  import { requestStatusLabel } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    DEFAULT_REQUEST_FILTERS,
    REQUEST_STATUS_FILTERS,
    REQUEST_TIME_RANGES,
    requestFilterCount,
    type RequestFilters,
    type RequestStatusFilter,
  } from '$lib/workbench/requestFilters'

  export let filters: RequestFilters = DEFAULT_REQUEST_FILTERS
  export let collapsed = false
  export let onChange: (filters: RequestFilters) => void = () => {}

  let open = false
  $: filterCount = requestFilterCount(filters)

  function tr(source: string) {
    return t($locale, source)
  }

  function statusLabel(status: RequestStatusFilter) {
    if (status === 'all') return tr('All statuses')
    if (status === 'pending') return tr('Pending requests')
    return requestStatusLabel(status, $locale)
  }

  const timeLabels = {
    all: 'All time',
    '24h': 'Last 24 hours',
    '7d': 'Last 7 days',
    '30d': 'Last 30 days',
  }
</script>

<Popover.Root bind:open>
  <Popover.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        variant={filterCount ? 'secondary' : 'ghost'}
        size="icon-sm"
        class="relative shrink-0"
        aria-label={tr('Filter requests')}
        title={tr('Filter requests')}
      >
        <Funnel />
        {#if filterCount > 0}
          <span class="absolute -right-0.5 -top-0.5 grid size-3.5 place-items-center rounded-full bg-primary text-[9px] font-medium text-primary-foreground">
            {filterCount}
          </span>
        {/if}
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content
      side={collapsed ? 'right' : 'bottom'}
      align={collapsed ? 'start' : 'end'}
      sideOffset={8}
      class="z-[130] w-72 max-w-[calc(100vw-1rem)] space-y-4 overflow-y-auto rounded-xl border bg-popover p-4 text-popover-foreground shadow-lg outline-none max-h-[var(--bits-popover-content-available-height)]"
      aria-label={tr('Request filters')}
    >
      <div class="flex items-center justify-between gap-2">
        <strong class="text-sm font-semibold">{tr('Request filters')}</strong>
        <Popover.Close>
          {#snippet child({ props })}
            <Button {...props} variant="ghost" size="icon-xs" aria-label={tr('Close')}>
              <X />
            </Button>
          {/snippet}
        </Popover.Close>
      </div>
      <fieldset class="space-y-2">
        <legend class="text-xs font-medium text-muted-foreground">{tr('Request status')}</legend>
        <div class="grid grid-cols-2 gap-1.5">
          {#each REQUEST_STATUS_FILTERS as status}
            <Button
              variant={filters.status === status ? 'secondary' : 'outline'}
              size="sm"
              class={['justify-start text-xs', filters.status === status && 'border-primary/30 bg-primary/10 text-primary']}
              aria-pressed={filters.status === status}
              onclick={() => onChange({ ...filters, status })}
            >
              {statusLabel(status)}
            </Button>
          {/each}
        </div>
      </fieldset>
      <fieldset class="space-y-2">
        <legend class="text-xs font-medium text-muted-foreground">{tr('Updated time')}</legend>
        <div class="grid grid-cols-2 gap-1.5">
          {#each REQUEST_TIME_RANGES as timeRange}
            <Button
              variant={filters.timeRange === timeRange ? 'secondary' : 'outline'}
              size="sm"
              class={['justify-start text-xs', filters.timeRange === timeRange && 'border-primary/30 bg-primary/10 text-primary']}
              aria-pressed={filters.timeRange === timeRange}
              onclick={() => onChange({ ...filters, timeRange })}
            >
              {tr(timeLabels[timeRange])}
            </Button>
          {/each}
        </div>
      </fieldset>
      <div class="flex items-center justify-between border-t pt-3">
        <span class="text-[11px] text-muted-foreground">{tr('Filters apply immediately')}</span>
        <Button
          variant="ghost"
          size="sm"
          class="text-xs"
          disabled={filterCount === 0}
          onclick={() => onChange(DEFAULT_REQUEST_FILTERS)}
        >
          {tr('Reset filters')}
        </Button>
      </div>
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
