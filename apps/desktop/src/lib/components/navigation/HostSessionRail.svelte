<script lang="ts">
  import {
    ChevronDown,
    ChevronRight,
    Inbox,
    MessageSquareText,
    PanelLeftClose,
    PanelLeftOpen,
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
  export let collapsed = false
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let onSelect: (hostId: string | null, hostSessionId: string | null) => void = () => {}
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

  function toggleSidebar() {
    collapsed = !collapsed
  }
</script>

<Tooltip.Provider delayDuration={350}>
  <aside
    class={[
      'flex min-h-0 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground transition-[width] duration-200',
      collapsed ? 'w-14' : 'w-[224px]',
    ]}
    aria-label={tr('宿主与会话')}
  >
    <div
      class={[
        'flex h-12 shrink-0 items-center border-b border-sidebar-border',
        collapsed ? 'justify-center px-2' : 'justify-between px-3',
      ]}
    >
      {#if !collapsed}
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
      {/if}

      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="ghost"
              size="icon-sm"
              aria-label={collapsed ? tr('展开侧栏') : tr('收起侧栏')}
              onclick={toggleSidebar}
            >
              {#if collapsed}
                <PanelLeftOpen />
              {:else}
                <PanelLeftClose />
              {/if}
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content side="right">
          {collapsed ? tr('展开侧栏') : tr('收起侧栏')}
        </Tooltip.Content>
      </Tooltip.Root>
    </div>

    <div class="border-b border-sidebar-border p-2">
      <button
        type="button"
        class={[
          'flex h-9 w-full items-center rounded-md text-left text-xs transition-colors',
          collapsed ? 'justify-center px-2' : 'gap-2 px-2',
          activeHostId === null
            ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
            : 'hover:bg-sidebar-accent/65',
        ]}
        aria-label={tr('全部请求')}
        title={collapsed ? tr('全部请求') : undefined}
        onclick={() => onSelect(null, null)}
      >
        <Inbox class="size-4 shrink-0" />
        {#if !collapsed}
          <span class="min-w-0 flex-1 truncate">{tr('全部请求')}</span>
          {#if totalPending > 0}
            <Badge variant="default" class="h-5 min-w-5 px-1.5 text-[10px]">{totalPending}</Badge>
          {:else}
            <span class="text-[10px] tabular-nums text-muted-foreground">{totalRequests}</span>
          {/if}
        {/if}
      </button>
    </div>

    <ScrollArea class="min-h-0 flex-1">
      {#if collapsed}
        <div class="space-y-1 p-2">
          {#each groups as group (group.hostId)}
            {@const profile = resolveHostProfile(group.hostId)}
            <button
              type="button"
              class={[
                'grid h-9 w-full place-items-center rounded-md text-muted-foreground transition-colors [&_svg]:size-4',
                activeHostId === group.hostId && activeHostSessionId === null
                  ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                  : 'hover:bg-sidebar-accent/65 hover:text-sidebar-foreground',
              ]}
              aria-label={profile.label}
              title={profile.label}
              onclick={() => onSelect(group.hostId, null)}
            >
              {@html profile.icon_svg}
            </button>

            {#each group.sessions as session (session.host_session_id)}
              <button
                type="button"
                class={[
                  'grid h-8 w-full place-items-center rounded-md text-muted-foreground transition-colors',
                  activeHostId === group.hostId && activeHostSessionId === session.host_session_id
                    ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                    : 'hover:bg-sidebar-accent/55 hover:text-sidebar-foreground',
                ]}
                aria-label={session.title}
                title={session.title}
                onclick={() => onSelect(group.hostId, session.host_session_id)}
              >
                <MessageSquareText class="size-3.5" />
              </button>
            {/each}
          {:else}
            <div
              class="grid h-16 place-items-center text-muted-foreground"
              aria-label={loading ? tr('正在读取宿主会话…') : tr('还没有宿主会话')}
              title={loading ? tr('正在读取宿主会话…') : tr('还没有宿主会话')}
            >
              <Inbox class="size-4" />
            </div>
          {/each}
        </div>
      {:else}
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
                  title={collapsedHosts.has(group.hostId) ? tr('展开会话') : tr('收起会话')}
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
                        activeHostId === group.hostId && activeHostSessionId === session.host_session_id
                          ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
                          : 'text-muted-foreground hover:bg-sidebar-accent/55 hover:text-sidebar-foreground',
                      ]}
                      title={session.title}
                      onclick={() => onSelect(group.hostId, session.host_session_id)}
                    >
                      <span class="min-w-0 flex-1 truncate">{session.title}</span>
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
      {/if}
    </ScrollArea>

    <div class="border-t border-sidebar-border p-2">
      <Button
        variant="ghost"
        class={collapsed ? 'w-full justify-center px-0' : 'w-full justify-start'}
        aria-label={tr('设置与适配器')}
        title={collapsed ? tr('设置与适配器') : undefined}
        onclick={onSettings}
      >
        <Settings data-icon="inline-start" />
        {#if !collapsed}{tr('设置与适配器')}{/if}
      </Button>
    </div>
  </aside>
</Tooltip.Provider>
