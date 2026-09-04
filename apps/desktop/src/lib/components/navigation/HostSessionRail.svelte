<script lang="ts">
  import { tick } from 'svelte'
  import {
    Archive,
    Inbox,
    LoaderCircle,
    MoreHorizontal,
    PanelLeftClose,
    PanelLeftOpen,
    Pencil,
    Pin,
    PinOff,
    Plus,
    Search,
    Settings,
  } from '@lucide/svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import type { HostSessionSummary } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { agentText } from '$lib/agents/agentI18n'
  import { locale } from '$lib/preferences'
  import type { HostProfile } from '$lib/workbench/types'
  import { hostSessionKey, orderSessionRailSessions } from './sessionRail'

  export let sessions: HostSessionSummary[] = []
  export let activeHostId: string | null = null
  export let activeHostSessionId: string | null = null
  export let requestSearch = ''
  export let loading = false
  export let refreshing = false
  export let collapsed = false
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let onSelect: (hostId: string | null, hostSessionId: string | null) => void = () => {}
  export let onRequestSearch: (search: string) => void = () => {}
  export let onSettings: () => void = () => {}
  export let onNewSession: (() => void) | undefined = undefined
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

  let orderedSessions: HostSessionSummary[] = []
  let editingSessionKey: string | null = null
  let editingTitle = ''
  let actionKey: string | null = null
  let titleInput: HTMLInputElement | null = null
  let requestSearchTimer: ReturnType<typeof setTimeout> | null = null

  $: orderedSessions = orderSessionRailSessions(sessions)

  $: totalRequests = sessions.reduce((total, session) => total + session.request_count, 0)
  $: totalPending = sessions.reduce((total, session) => total + session.pending_count, 0)

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function toggleSidebar() {
    collapsed = !collapsed
  }

  function scheduleRequestSearch(value: string) {
    if (requestSearchTimer) clearTimeout(requestSearchTimer)
    requestSearchTimer = setTimeout(() => onRequestSearch(value), 180)
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
    editingSessionKey = hostSessionKey(session)
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
    const key = hostSessionKey(session)
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
    aria-label={tr('Sessions')}
  >
    <div
      class={[
        'flex h-12 shrink-0 items-center border-b border-sidebar-border',
        collapsed ? 'justify-center px-2' : 'justify-between px-3',
      ]}
    >
      {#if !collapsed}
        <div class="min-w-0">
          <strong class="block truncate text-xs font-semibold">{tr('Sessions')}</strong>
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
      {#if onNewSession}
        <Button variant="outline" size="sm" class={collapsed ? 'mb-2 w-full justify-center px-0' : 'mb-2 w-full justify-start'} aria-label={agentText($locale, 'New agent session')} title={collapsed ? agentText($locale, 'New agent session') : undefined} onclick={onNewSession}>
          <Plus class="size-4" />{#if !collapsed}{agentText($locale, 'New agent session')}{/if}
        </Button>
      {/if}
      {#if !collapsed}
        <label
          class="mb-2 flex h-8 items-center gap-2 rounded-md border border-sidebar-border bg-background/80 px-2 text-[11px] text-muted-foreground focus-within:ring-2 focus-within:ring-ring/40"
        >
          <Search class="size-3.5 shrink-0" aria-hidden="true" />
          <input
            value={requestSearch}
            class="min-w-0 flex-1 bg-transparent text-sidebar-foreground outline-none placeholder:text-muted-foreground"
            aria-label={tr('Search active requests…')}
            placeholder={tr('Search active requests…')}
            oninput={(event) => scheduleRequestSearch(event.currentTarget.value)}
          />
        </label>
      {/if}
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
        <Inbox class="size-5 shrink-0" aria-hidden="true" />
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

    <ScrollArea class="min-h-0 flex-1" aria-busy={refreshing}>
      <div class="relative min-h-full">
        <div class={refreshing ? 'pointer-events-none select-none opacity-40' : undefined}>
          {#if collapsed}
            <div class="space-y-1 p-2">
              {#each orderedSessions as session (hostSessionKey(session))}
                {@const profile = resolveHostProfile(session.host_id)}
                <button
                  type="button"
                  class={[
                    'relative grid h-9 w-full place-items-center rounded-md text-muted-foreground transition-colors [&_svg]:size-5',
                    activeHostId === session.host_id &&
                    activeHostSessionId === session.host_session_id
                      ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                      : 'hover:bg-sidebar-accent/65 hover:text-sidebar-foreground',
                  ]}
                  aria-label={`${session.title} · ${profile.label}`}
                  title={`${session.title} · ${profile.label}`}
                  onclick={() => onSelect(session.host_id, session.host_session_id)}
                >
                  <span aria-hidden="true">{@html profile.icon_svg}</span>
                  {#if session.management.kind === 'managed'}<span class="absolute bottom-0.5 right-0.5 rounded bg-sidebar px-0.5 text-[7px] font-medium text-primary" title={agentText($locale, 'Managed session')}>ACP</span>{/if}
                </button>
              {:else}
                <div
                  class="grid h-16 place-items-center text-muted-foreground"
                  aria-label={loading ? tr('Loading host sessions…') : tr('No host sessions yet')}
                  title={loading ? tr('Loading host sessions…') : tr('No host sessions yet')}
                >
                  <Inbox class="size-4" aria-hidden="true" />
                </div>
              {/each}
            </div>
          {:else}
            <div class="space-y-1 p-2">
              {#each orderedSessions as session (hostSessionKey(session))}
                {@const profile = resolveHostProfile(session.host_id)}
                {@const key = hostSessionKey(session)}
                <div class="group/session flex min-h-9 items-center gap-1">
                  {#if editingSessionKey === key}
                    <form
                      class="flex h-9 min-w-0 flex-1 items-center"
                      onsubmit={(event) => {
                        event.preventDefault()
                        void commitRename(session)
                      }}
                    >
                      <input
                        bind:this={titleInput}
                        bind:value={editingTitle}
                        class="h-7 min-w-0 flex-1 rounded-md border border-input bg-background px-2 text-[11px] outline-none ring-ring/40 focus:ring-2"
                        aria-label={`${tr('Rename session')}: ${session.title}`}
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
                    <div
                      class={[
                        'flex min-h-9 min-w-0 flex-1 items-center rounded-md text-[11px] transition-colors',
                        activeHostId === session.host_id && activeHostSessionId === session.host_session_id
                          ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
                          : 'text-muted-foreground hover:bg-sidebar-accent/55 hover:text-sidebar-foreground',
                      ]}
                    >
                      <button
                        type="button"
                        class="flex min-h-9 min-w-0 flex-1 items-center gap-2 rounded-md bg-transparent px-2 py-1.5 text-left text-[11px] outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
                        aria-label={`${session.title} · ${profile.label}`}
                        title={`${session.title} · ${profile.label}`}
                        onclick={() => onSelect(session.host_id, session.host_session_id)}
                      >
                        <span class="grid size-5 shrink-0 place-items-center text-muted-foreground [&_svg]:size-4" aria-hidden="true">
                          {@html profile.icon_svg}
                        </span>
                        <span class="min-w-0 flex-1 truncate">{session.title}</span>
                        {#if session.management.kind === 'managed'}<span class="shrink-0 text-[8px] font-medium text-muted-foreground" title={agentText($locale, 'Managed session')}>ACP</span>{/if}
                        {#if session.pinned_at}
                          <Pin class="size-3 shrink-0 text-primary" aria-hidden="true" />
                        {/if}
                        {#if session.pending_count > 0}
                          <span class="size-1.5 shrink-0 rounded-full bg-primary" aria-hidden="true"></span>
                        {/if}
                      </button>

                      <DropdownMenu.Root>
                        <DropdownMenu.Trigger>
                          {#snippet child({ props })}
                            <Button
                              {...props}
                              variant="ghost"
                              size="icon-xs"
                              aria-label={`${tr('Session actions')}: ${session.title}`}
                              title={tr('Session actions')}
                              class="mr-1 hidden hover:bg-transparent aria-expanded:bg-transparent group-hover/session:inline-flex group-focus-within/session:inline-flex aria-expanded:inline-flex dark:hover:bg-transparent"
                              disabled={actionKey !== null}
                            >
                              <MoreHorizontal aria-hidden="true" />
                            </Button>
                          {/snippet}
                        </DropdownMenu.Trigger>
                        <DropdownMenu.Content align="end" class="w-44">
                          <DropdownMenu.Item onclick={() => void startRename(session)}>
                            <Pencil class="size-4" aria-hidden="true" />
                            {tr('Rename session')}
                          </DropdownMenu.Item>
                          <DropdownMenu.Item
                            onclick={() =>
                              void runAction(`session-pin:${key}`, () =>
                                onSetSessionPinned(session, !session.pinned_at),
                              )}
                          >
                            {#if session.pinned_at}
                              <PinOff class="size-4" aria-hidden="true" />
                              {tr('Unpin session')}
                            {:else}
                              <Pin class="size-4" aria-hidden="true" />
                              {tr('Pin session')}
                            {/if}
                          </DropdownMenu.Item>
                          <DropdownMenu.Item
                            onclick={() =>
                              void runAction(`host-pin:${session.host_id}`, () =>
                                onSetHostPinned(session.host_id, !session.host_pinned_at),
                              )}
                          >
                            {#if session.host_pinned_at}
                              <PinOff class="size-4" aria-hidden="true" />
                              {tr('Unpin host')}
                            {:else}
                              <Pin class="size-4" aria-hidden="true" />
                              {tr('Pin host')}
                            {/if}
                          </DropdownMenu.Item>
                          <DropdownMenu.Item
                            disabled={session.pending_count > 0}
                            onclick={() =>
                              session.pending_count === 0
                                ? void runAction(`session-archive:${key}`, () => onArchiveSession(session))
                                : undefined}
                          >
                            <Archive class="size-4" aria-hidden="true" />
                            {tr('Archive session')}
                          </DropdownMenu.Item>
                        </DropdownMenu.Content>
                      </DropdownMenu.Root>
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
        </div>
        {#if refreshing && sessions.length > 0}
          <div class="absolute inset-0 z-20 grid place-items-center bg-sidebar/80 backdrop-blur-[1px]">
            <LoaderCircle class="size-5 animate-spin text-primary" aria-hidden="true" />
          </div>
        {/if}
      </div>
    </ScrollArea>

    <div class="space-y-1 border-t border-sidebar-border p-2">
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
