<script lang="ts">
  import {
    ChevronDown,
    ChevronRight,
    Inbox,
    RefreshCw,
    Settings,
  } from '@lucide/svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import type { HostSessionSummary } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { HostProfile } from '$lib/workbench/types'

  type HostGroup = {
    hostId: string
    sessions: HostSessionSummary[]
    requestCount: number
    pendingCount: number
  }

  export let sessions: HostSessionSummary[] = []
  export let activeHostId: string | null = null
  export let activeHostSessionId: string | null = null
  export let loading = false
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let onSelect: (hostId: string | null, hostSessionId: string | null) => void = () => {}
  export let onRefresh: () => void = () => {}
  export let onSettings: () => void = () => {}

  let collapsedHosts = new Set<string>()
  let groups: HostGroup[] = []

  $: groups = Array.from(
    sessions.reduce((byHost, session) => {
      const group = byHost.get(session.host_id) ?? {
        hostId: session.host_id,
        sessions: [],
        requestCount: 0,
        pendingCount: 0,
      }
      group.sessions.push(session)
      group.requestCount += session.request_count
      group.pendingCount += session.pending_count
      byHost.set(session.host_id, group)
      return byHost
    }, new Map<string, HostGroup>()),
  ).map(([, group]) => group)

  $: totalRequests = sessions.reduce((total, session) => total + session.request_count, 0)
  $: totalPending = sessions.reduce((total, session) => total + session.pending_count, 0)

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function toggleHost(hostId: string) {
    const next = new Set(collapsedHosts)
    if (next.has(hostId)) next.delete(hostId)
    else next.add(hostId)
    collapsedHosts = next
  }
</script>

<Tooltip.Provider delayDuration={350}>
  <aside
    class="flex min-h-0 w-[224px] shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground"
    aria-label={tr('宿主与会话')}
  >
    <div class="flex h-12 items-center justify-between border-b border-sidebar-border px-3">
      <div class="flex min-w-0 items-center gap-2">
        <div class="grid size-6 place-items-center rounded-md bg-primary text-[11px] font-bold text-primary-foreground">
          R
        </div>
        <div class="min-w-0">
          <strong class="block truncate text-xs font-semibold">{tr('宿主')}</strong>
          <span class="block text-[10px] text-muted-foreground">
            {sessions.length} {tr('个会话')}
          </span>
        </div>
      </div>

      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="ghost"
              size="icon-sm"
              disabled={loading}
              aria-label={tr('刷新宿主与会话')}
              onclick={onRefresh}
            >
              <RefreshCw class={loading ? 'animate-spin' : ''} />
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content side="right">{tr('刷新宿主与会话')}</Tooltip.Content>
      </Tooltip.Root>
    </div>

    <div class="border-b border-sidebar-border p-2">
      <button
        type="button"
        class={[
          'flex h-9 w-full items-center gap-2 rounded-md px-2 text-left text-xs transition-colors',
          activeHostId === null
            ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
            : 'hover:bg-sidebar-accent/65',
        ]}
        onclick={() => onSelect(null, null)}
      >
        <Inbox class="size-4 shrink-0" />
        <span class="min-w-0 flex-1 truncate">{tr('全部请求')}</span>
        {#if totalPending > 0}
          <Badge variant="default" class="h-5 min-w-5 px-1.5 text-[10px]">{totalPending}</Badge>
        {:else}
          <span class="text-[10px] tabular-nums text-muted-foreground">{totalRequests}</span>
        {/if}
      </button>
    </div>

    <ScrollArea class="min-h-0 flex-1">
      <div class="space-y-1 p-2">
        {#each groups as group (group.hostId)}
          {@const profile = resolveHostProfile(group.hostId)}
          <div>
            <div class="group flex items-center gap-1">
              <button
                type="button"
                class={[
                  'flex h-9 min-w-0 flex-1 items-center gap-2 rounded-md px-2 text-left text-xs transition-colors',
                  activeHostId === group.hostId && activeHostSessionId === null
                    ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
                    : 'hover:bg-sidebar-accent/65',
                ]}
                onclick={() => onSelect(group.hostId, null)}
              >
                <span class="grid size-5 shrink-0 place-items-center text-muted-foreground [&_svg]:size-4">
                  {@html profile.icon_svg}
                </span>
                <span class="min-w-0 flex-1 truncate">{profile.label}</span>
                {#if group.pendingCount > 0}
                  <Badge variant="secondary" class="h-5 min-w-5 px-1.5 text-[10px]">
                    {group.pendingCount}
                  </Badge>
                {:else}
                  <span class="text-[10px] tabular-nums text-muted-foreground">
                    {group.requestCount}
                  </span>
                {/if}
              </button>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={collapsedHosts.has(group.hostId) ? tr('展开会话') : tr('收起会话')}
                onclick={() => toggleHost(group.hostId)}
              >
                {#if collapsedHosts.has(group.hostId)}
                  <ChevronRight />
                {:else}
                  <ChevronDown />
                {/if}
              </Button>
            </div>

            {#if !collapsedHosts.has(group.hostId)}
              <div class="ml-3 border-l border-sidebar-border pl-2">
                {#each group.sessions as session (session.host_session_id)}
                  <button
                    type="button"
                    class={[
                      'flex min-h-8 w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[11px] transition-colors',
                      activeHostId === group.hostId &&
                      activeHostSessionId === session.host_session_id
                        ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
                        : 'text-muted-foreground hover:bg-sidebar-accent/55 hover:text-sidebar-foreground',
                    ]}
                    title={session.host_session_id}
                    onclick={() => onSelect(group.hostId, session.host_session_id)}
                  >
                    <span class="min-w-0 flex-1 truncate">{session.host_session_id}</span>
                    {#if session.pending_count > 0}
                      <span class="size-1.5 shrink-0 rounded-full bg-primary"></span>
                    {/if}
                    <span class="shrink-0 text-[9px] tabular-nums">{session.request_count}</span>
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        {:else}
          <div class="px-2 py-8 text-center text-[11px] leading-5 text-muted-foreground">
            {loading ? tr('正在读取宿主会话…') : tr('还没有宿主会话')}
          </div>
        {/each}
      </div>
    </ScrollArea>

    <div class="border-t border-sidebar-border p-2">
      <Button variant="ghost" class="w-full justify-start" onclick={onSettings}>
        <Settings data-icon="inline-start" />
        {tr('设置与适配器')}
      </Button>
    </div>
  </aside>
</Tooltip.Provider>
