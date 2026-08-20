<script lang="ts">
  import { tick } from 'svelte'
  import {
    Archive,
    ArchiveRestore,
    ChevronDown,
    ChevronRight,
    Inbox,
    MessageSquareText,
    MoreHorizontal,
    PanelLeftClose,
    PanelLeftOpen,
    Pencil,
    Pin,
    PinOff,
    Settings,
  } from '@lucide/svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
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
    updatedAt: string
    hostPinnedAt: string | null
  }

  export let sessions: HostSessionSummary[] = []
  export let activeHostId: string | null = null
  export let activeHostSessionId: string | null = null
  export let loading = false
  export let collapsed = false
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let onSelect: (hostId: string | null, hostSessionId: string | null) => void = () => {}
  export let onArchived: () => void = () => {}
  export let onSettings: () => void = () => {}
  export let onRenameSession: (
    session: HostSessionSummary,
    title: string,
  ) => Promise<void> | void = () => {}
  export let onSetSessionPinned: (
    session: HostSessionSummary,
    pinned: boolean,
  ) => Promise<void> | void = () => {}
  export let onArchiveSession: (session: HostSessionSummary) => Promise<void> | void = () => {}
  export let onSetHostPinned: (hostId: string, pinned: boolean) => Promise<void> | void = () => {}

  let collapsedHosts = new Set<string>()
  let groups: HostGroup[] = []
  let editingSessionKey: string | null = null
  let editingTitle = ''
  let actionKey: string | null = null
  let titleInput: HTMLInputElement | null = null

  $: groups = Array.from(
    sessions.reduce((byHost, session) => {
      const group = byHost.get(session.host_id) ?? {
        hostId: session.host_id,
        sessions: [],
        requestCount: 0,
        pendingCount: 0,
        updatedAt: session.updated_at,
        hostPinnedAt: session.host_pinned_at,
      }
      group.sessions.push(session)
      group.requestCount += session.request_count
      group.pendingCount += session.pending_count
      if (session.updated_at > group.updatedAt) group.updatedAt = session.updated_at
      group.hostPinnedAt = group.hostPinnedAt ?? session.host_pinned_at
      byHost.set(session.host_id, group)
      return byHost
    }, new Map<string, HostGroup>()),
  )
    .map(([, group]) => {
      group.sessions = group.sessions.sort(compareSessionOrder)
      return group
    })
    .sort(compareHostGroupOrder)

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

  function sessionKey(session: HostSessionSummary) {
    return `${session.host_id}\u0000${session.host_session_id}`
  }

  function compareNullableIsoDesc(left: string | null | undefined, right: string | null | undefined) {
    if (left === right) return 0
    if (!left) return 1
    if (!right) return -1
    return right.localeCompare(left)
  }

  function compareHostGroupOrder(left: HostGroup, right: HostGroup) {
    return (
      compareNullableIsoDesc(left.hostPinnedAt, right.hostPinnedAt) ||
      compareNullableIsoDesc(left.updatedAt, right.updatedAt) ||
      left.hostId.localeCompare(right.hostId)
    )
  }

  function compareSessionOrder(left: HostSessionSummary, right: HostSessionSummary) {
    return (
      compareNullableIsoDesc(left.pinned_at, right.pinned_at) ||
      compareNullableIsoDesc(left.updated_at, right.updated_at) ||
      left.host_session_id.localeCompare(right.host_session_id)
    )
  }

  async function runAction(key: string, action: () => Promise<void> | void) {
    if (actionKey) return
    actionKey = key
    try {
      await action()
    } finally {
      actionKey = null
    }
  }

  async function startRename(session: HostSessionSummary) {
    editingSessionKey = sessionKey(session)
    editingTitle = session.title
    await tick()
    titleInput?.focus()
    titleInput?.select()
  }

  function cancelRename() {
    editingSessionKey = null
    editingTitle = ''
  }

  async function commitRename(session: HostSessionSummary) {
    const key = sessionKey(session)
    if (editingSessionKey !== key) return
    const nextTitle = editingTitle.trim()
    if (!nextTitle || nextTitle === session.title) {
      cancelRename()
      return
    }
    await runAction(`rename:${key}`, async () => {
      await onRenameSession(session, nextTitle)
      cancelRename()
    })
  }
</script>

<Tooltip.Provider delayDuration={350}>
  <aside
    class={[
      'flex min-h-0 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground transition-[width] duration-200',
      collapsed ? 'w-14' : 'w-[224px]',
    ]}
    aria-label={tr('Hosts and sessions')}
  >
    <div
      class={[
        'flex h-12 shrink-0 items-center border-b border-sidebar-border',
        collapsed ? 'justify-center px-2' : 'justify-between px-3',
      ]}
    >
      {#if !collapsed}
        <div class="min-w-0">
          <strong class="block truncate text-xs font-semibold">{tr('Hosts')}</strong>
          <span class="block text-[10px] text-muted-foreground">
            {sessions.length} {tr('sessions')}
          </span>
        </div>
      {/if}

      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="ghost"
              size="icon-sm"
              aria-label={collapsed ? tr('Expand sidebar') : tr('Collapse sidebar')}
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
          {collapsed ? tr('Expand sidebar') : tr('Collapse sidebar')}
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
        aria-label={tr('All requests')}
        title={collapsed ? tr('All requests') : undefined}
        onclick={() => onSelect(null, null)}
      >
        <Inbox class="size-5 shrink-0" />
        {#if !collapsed}
          <span class="min-w-0 flex-1 truncate">{tr('All requests')}</span>
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
                <MessageSquareText class="size-4" />
              </button>
            {/each}
          {:else}
            <div
              class="grid h-16 place-items-center text-muted-foreground"
              aria-label={loading ? tr('Loading host sessions…') : tr('No host sessions yet')}
              title={loading ? tr('Loading host sessions…') : tr('No host sessions yet')}
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
                  <span class="grid size-6 shrink-0 place-items-center text-muted-foreground [&_svg]:size-5">
                    {@html profile.icon_svg}
                  </span>
                  <span class="min-w-0 flex-1 truncate">{profile.label}</span>
                  {#if group.hostPinnedAt}
                    <Pin class="size-3 shrink-0 text-primary" />
                  {/if}
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
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    {#snippet child({ props })}
                      <Button
                        {...props}
                        variant="ghost"
                        size="icon-xs"
                        aria-label={tr('Host actions')}
                        title={tr('Host actions')}
                        class="opacity-70 group-hover:opacity-100 aria-expanded:opacity-100"
                        disabled={actionKey !== null}
                      >
                        <MoreHorizontal />
                      </Button>
                    {/snippet}
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end" class="w-40">
                    <DropdownMenu.Item
                      onclick={() =>
                        void runAction(`host-pin:${group.hostId}`, () =>
                          onSetHostPinned(group.hostId, !group.hostPinnedAt),
                        )}
                    >
                      {#if group.hostPinnedAt}
                        <PinOff class="size-4" />
                        {tr('Unpin host')}
                      {:else}
                        <Pin class="size-4" />
                        {tr('Pin host')}
                      {/if}
                    </DropdownMenu.Item>
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={collapsedHosts.has(group.hostId) ? tr('Expand sessions') : tr('Collapse sessions')}
                  title={collapsedHosts.has(group.hostId) ? tr('Expand sessions') : tr('Collapse sessions')}
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
                    {@const key = sessionKey(session)}
                    <div class="group/session flex min-h-8 items-center gap-1">
                      {#if editingSessionKey === key}
                        <form
                          class="flex h-8 min-w-0 flex-1 items-center"
                          onsubmit={(event) => {
                            event.preventDefault()
                            void commitRename(session)
                          }}
                        >
                          <input
                            bind:this={titleInput}
                            bind:value={editingTitle}
                            class="h-7 min-w-0 flex-1 rounded-md border border-input bg-background px-2 text-[11px] outline-none ring-ring/40 focus:ring-2"
                            maxlength="160"
                            disabled={actionKey === `rename:${key}`}
                            onblur={() => void commitRename(session)}
                            onkeydown={(event) => {
                              if (event.key === 'Escape') {
                                event.preventDefault()
                                cancelRename()
                              }
                            }}
                          />
                        </form>
                      {:else}
                        <button
                          type="button"
                          class={[
                            'flex min-h-8 min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left text-[11px] transition-colors',
                            activeHostId === group.hostId && activeHostSessionId === session.host_session_id
                              ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
                              : 'text-muted-foreground hover:bg-sidebar-accent/55 hover:text-sidebar-foreground',
                          ]}
                          title={session.title}
                          onclick={() => onSelect(group.hostId, session.host_session_id)}
                        >
                          <span class="min-w-0 flex-1 truncate">{session.title}</span>
                          {#if session.pinned_at}
                            <Pin class="size-3 shrink-0 text-primary" />
                          {/if}
                          {#if session.pending_count > 0}
                            <span class="size-1.5 shrink-0 rounded-full bg-primary"></span>
                          {/if}
                          <span class="shrink-0 text-[9px] tabular-nums">{session.request_count}</span>
                        </button>

                        <DropdownMenu.Root>
                          <DropdownMenu.Trigger>
                            {#snippet child({ props })}
                              <Button
                                {...props}
                                variant="ghost"
                                size="icon-xs"
                                aria-label={tr('Session actions')}
                                title={tr('Session actions')}
                                class="opacity-60 group-hover/session:opacity-100 aria-expanded:opacity-100"
                                disabled={actionKey !== null}
                              >
                                <MoreHorizontal />
                              </Button>
                            {/snippet}
                          </DropdownMenu.Trigger>
                          <DropdownMenu.Content align="end" class="w-44">
                            <DropdownMenu.Item onclick={() => void startRename(session)}>
                              <Pencil class="size-4" />
                              {tr('Rename session')}
                            </DropdownMenu.Item>
                            <DropdownMenu.Item
                              onclick={() =>
                                void runAction(`session-pin:${key}`, () =>
                                  onSetSessionPinned(session, !session.pinned_at),
                                )}
                            >
                              {#if session.pinned_at}
                                <PinOff class="size-4" />
                                {tr('Unpin session')}
                              {:else}
                                <Pin class="size-4" />
                                {tr('Pin session')}
                              {/if}
                            </DropdownMenu.Item>
                            <DropdownMenu.Item
                              disabled={session.pending_count > 0}
                              onclick={() =>
                                session.pending_count === 0
                                  ? void runAction(`session-archive:${key}`, () => onArchiveSession(session))
                                  : undefined}
                            >
                              <Archive class="size-4" />
                              {tr('Archive session')}
                            </DropdownMenu.Item>
                          </DropdownMenu.Content>
                        </DropdownMenu.Root>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {:else}
            <div class="px-2 py-8 text-center text-[11px] leading-5 text-muted-foreground">
              {loading ? tr('Loading host sessions…') : tr('No host sessions yet')}
            </div>
          {/each}
        </div>
      {/if}
    </ScrollArea>

    <div class="space-y-1 border-t border-sidebar-border p-2">
      <Button
        variant="ghost"
        class={collapsed ? 'w-full justify-center px-0' : 'w-full justify-start'}
        aria-label={tr('Archived')}
        title={collapsed ? tr('Archived') : undefined}
        onclick={onArchived}
      >
        <ArchiveRestore data-icon="inline-start" />
        {#if !collapsed}{tr('Archived')}{/if}
      </Button>
      <Button
        variant="ghost"
        class={collapsed ? 'w-full justify-center px-0' : 'w-full justify-start'}
        aria-label={tr('Settings and adapters')}
        title={collapsed ? tr('Settings and adapters') : undefined}
        onclick={onSettings}
      >
        <Settings data-icon="inline-start" />
        {#if !collapsed}{tr('Settings and adapters')}{/if}
      </Button>
    </div>
  </aside>
</Tooltip.Provider>
