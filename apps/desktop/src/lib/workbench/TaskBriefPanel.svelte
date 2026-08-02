<script lang="ts">
  import { ChevronDown, ListChecks } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let workspace: FeedbackWorkspaceView
  export let open = true

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<Collapsible.Root bind:open class="flex max-h-[50%] shrink-0 flex-col overflow-hidden border-b">
  <div class="flex min-h-12 shrink-0 items-center gap-3 px-5 py-2">
    <ListChecks class="size-4 shrink-0 text-muted-foreground" />
    <div class="min-w-0 flex-1">
      {#if open}
        <strong class="block text-xs font-medium">
          {tr('发生了什么')} · {tr('需要体验')}
        </strong>
      {:else}
        <strong class="block text-xs font-medium">{tr('任务简报')}</strong>
        <span class="block truncate text-[10px] text-muted-foreground">
          {workspace.request.what_happened}
        </span>
      {/if}
    </div>
    <Badge variant="secondary" class="h-5 px-1.5 text-[9px]">
      {tr('{count} 个步骤', { count: workspace.actions.length })}
    </Badge>
    <Collapsible.Trigger>
      {#snippet child({ props })}
        <Button
          {...props}
          variant="ghost"
          size="icon-sm"
          aria-label={open ? tr('收起') : tr('展开')}
          title={open ? tr('收起') : tr('展开')}
        >
          <ChevronDown class={['transition-transform', open ? 'rotate-180' : '']} />
        </Button>
      {/snippet}
    </Collapsible.Trigger>
  </div>

  <Collapsible.Content class="min-h-0 overflow-y-auto overscroll-contain">
    <div class="grid gap-5 bg-muted/25 px-5 py-4 text-xs @min-[700px]:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
      <section>
        <h2 class="m-0 text-[10px] font-semibold uppercase text-muted-foreground">
          {tr('发生了什么')}
        </h2>
        <p class="m-0 mt-2 leading-5">{workspace.request.what_happened}</p>
      </section>

      <section>
        <h2 class="m-0 text-[10px] font-semibold uppercase text-muted-foreground">
          {tr('需要体验')}
        </h2>
        <ol class="m-0 mt-2 grid list-none gap-2 p-0">
          {#each workspace.actions as action, index (action.id)}
            <li class="grid grid-cols-[22px_minmax(0,1fr)] gap-2 leading-5">
              <span class="grid size-5 place-items-center rounded-md bg-background text-[9px] font-medium ring-1 ring-border">
                {index + 1}
              </span>
              <span>{action.instruction}</span>
            </li>
          {/each}
        </ol>
      </section>
    </div>
  </Collapsible.Content>
</Collapsible.Root>
