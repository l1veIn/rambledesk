<script lang="ts">
  import { onDestroy } from 'svelte'
  import {
    Archive, ChevronDown, ChevronRight, Folder, FolderOpen, Inbox, LoaderCircle,
    PanelLeftClose, PanelLeftOpen, Pin, PinOff, Plus, Search, Settings, X,
  } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import AgentIcon from '$lib/agents/AgentIcon.svelte'
  import type { HostSessionSummary } from '$lib/feedback'
  import { locale } from '$lib/preferences'
  import type { HostProfile } from '$lib/workbench/types'
  import { filterSessionRailProjects, groupSessionRailProjects, hostSessionKey } from './sessionRail'
  import { sessionRailText } from './sessionRailI18n'

  export let sessions: HostSessionSummary[] = []
  export let activeHostId: string | null = null
  export let activeHostSessionId: string | null = null
  export let inboxActive = false
  export let requestSearch = ''
  export let loading = false
  export let refreshing = false
  export let collapsed = false
  /** When set, the parent owns collapse state (used by the phone drawer); otherwise `bind:collapsed` applies. */
  export let onCollapsedChange: ((collapsed: boolean) => void) | undefined = undefined
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let onSelect: (hostId: string | null, hostSessionId: string | null) => void = () => {}
  export let onRequestSearch: (search: string) => void = () => {}
  export let onSearchRequests: ((search: string) => void) | undefined = undefined
  export let onSettings: () => void = () => {}
  export let onNewSession: ((cwd?: string) => void) | undefined = undefined
  export let onSetSessionPinned: (session: HostSessionSummary, pinned: boolean) => Promise<void> | void = () => {}
  export let onArchiveSession: (session: HostSessionSummary) => Promise<void> | void = () => {}

  let actionKey: string | null = null
  let searchOpen = false
  let searchDraft = requestSearch
  let collapsedProjects = new Set<string>()
  let requestSearchTimer: ReturnType<typeof setTimeout> | null = null
  let lastActiveSessionKey = ''

  $: projects = groupSessionRailProjects(sessions)
  $: visibleProjects = filterSessionRailProjects(projects, requestSearch)
  $: totalPending = sessions.reduce((total, session) => total + session.pending_count, 0)
  $: if (!searchOpen) searchDraft = requestSearch
  $: revealActiveSession(projects, activeHostId, activeHostSessionId)

  onDestroy(() => { if (requestSearchTimer) clearTimeout(requestSearchTimer) })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return sessionRailText($locale, source, values)
  }

  function revealActiveSession(nextProjects: typeof projects, hostId: string | null, sessionId: string | null) {
    const key = `${hostId ?? ''}\u0000${sessionId ?? ''}`
    if (key === lastActiveSessionKey) return
    lastActiveSessionKey = key
    const project = nextProjects.find((candidate) => candidate.sessions.some(
      (session) => session.host_id === hostId && session.host_session_id === sessionId,
    ))
    if (project && collapsedProjects.has(project.key)) {
      collapsedProjects = new Set([...collapsedProjects].filter((candidate) => candidate !== project.key))
    }
  }

  function toggleProject(key: string) {
    const next = new Set(collapsedProjects)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    collapsedProjects = next
  }

  function scheduleRequestSearch(value: string) {
    searchDraft = value
    if (requestSearchTimer) clearTimeout(requestSearchTimer)
    requestSearchTimer = setTimeout(() => {
      requestSearchTimer = null
      onRequestSearch(value)
    }, 180)
  }

  function setCollapsed(next: boolean) {
    if (onCollapsedChange) onCollapsedChange(next)
    else collapsed = next
  }

  function applySearch(requests = false) {
    if (requestSearchTimer) clearTimeout(requestSearchTimer)
    requestSearchTimer = null
    onRequestSearch(searchDraft)
    if (requests) onSearchRequests?.(searchDraft)
    else setCollapsed(false)
    searchOpen = false
  }

  function clearSearch() {
    if (requestSearchTimer) clearTimeout(requestSearchTimer)
    requestSearchTimer = null
    searchDraft = ''
    onRequestSearch('')
  }

  async function runAction(key: string, action: () => Promise<void> | void) {
    if (actionKey) return
    actionKey = key
    try { await action() } finally { actionKey = null }
  }
</script>

<aside
  data-navigation-pane
  class={[
    'flex min-h-0 shrink-0 flex-col overflow-hidden border-r border-sidebar-border bg-sidebar text-sidebar-foreground transition-[width] duration-200',
    collapsed ? 'w-[var(--workbench-sidebar-collapsed-width)]' : 'w-[var(--workbench-sidebar-width)]',
  ]}
  aria-label={tr('Projects')}
>
  <div class={['flex shrink-0 items-center gap-0.5', collapsed ? 'flex-col px-2 py-2' : 'h-12 px-3']}>
    {#if !collapsed}<strong class="min-w-0 flex-1 truncate text-xs font-semibold">RambleDesk</strong>{/if}
    {#if !collapsed}
    <Dialog.Root bind:open={searchOpen}>
      <Dialog.Trigger>
        {#snippet child({ props })}
          <Button {...props} variant="ghost" size="icon-sm" class={requestSearch ? 'text-primary' : ''} aria-label={tr('Search sessions and projects')} title={tr('Search sessions and projects')}>
            <Search aria-hidden="true" />
          </Button>
        {/snippet}
      </Dialog.Trigger>
      <Dialog.Content class="max-w-md">
        <Dialog.Header>
          <Dialog.Title>{tr('Search sessions and projects')}</Dialog.Title>
          <Dialog.Description>{tr('Search by session title, project name, or folder path.')}</Dialog.Description>
        </Dialog.Header>
        <form class="space-y-3" onsubmit={(event) => { event.preventDefault(); applySearch() }}>
          <label class="flex h-10 items-center gap-2 rounded-lg border bg-background px-3 focus-within:ring-2 focus-within:ring-ring/40">
            <Search class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            <input value={searchDraft} class="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground" aria-label={tr('Search sessions and projects')} placeholder={tr('Search sessions and projects')} oninput={(event) => scheduleRequestSearch(event.currentTarget.value)} />
            {#if searchDraft}<Button type="button" variant="ghost" size="icon-xs" aria-label={tr('Clear search')} title={tr('Clear search')} onclick={clearSearch}><X aria-hidden="true" /></Button>{/if}
          </label>
          <div class="flex justify-end gap-2">
            {#if onSearchRequests}<Button type="button" variant="outline" size="sm" onclick={() => applySearch(true)}>{tr('Search requests')}</Button>{/if}
            <Button type="submit" size="sm">{tr('Show results')}</Button>
          </div>
        </form>
      </Dialog.Content>
    </Dialog.Root>
    <Button variant="ghost" size="icon-sm" class={['relative', inboxActive ? 'bg-sidebar-accent text-sidebar-accent-foreground' : '']} aria-label={tr('All requests')} title={tr('All requests')} aria-current={inboxActive ? 'page' : undefined} onclick={() => onSelect(null, null)}>
      <Inbox aria-hidden="true" />
      {#if totalPending > 0}<span class="absolute right-1 top-1 size-1.5 rounded-full bg-primary" aria-label={`${totalPending}`}></span>{/if}
    </Button>
    {/if}
    <Button variant="ghost" size="icon-sm" aria-label={collapsed ? tr('Expand sidebar') : tr('Collapse sidebar')} title={collapsed ? tr('Expand sidebar') : tr('Collapse sidebar')} onclick={() => setCollapsed(!collapsed)}>
      {#if collapsed}<PanelLeftOpen aria-hidden="true" />{:else}<PanelLeftClose aria-hidden="true" />{/if}
    </Button>
  </div>

  {#if onNewSession}
    <div class="shrink-0 px-2 pb-3">
      <Button variant="ghost" size="sm" class={collapsed ? 'w-full justify-center px-0' : 'w-full justify-start'} aria-label={tr('New session')} title={collapsed ? tr('New session') : undefined} onclick={() => onNewSession?.()}>
        <Plus class="size-4" aria-hidden="true" />{#if !collapsed}{tr('New session')}{/if}
      </Button>
    </div>
  {/if}
  {#if !collapsed}<div class="shrink-0 px-4 pb-1 text-[10px] font-medium text-muted-foreground">{tr('Projects')}</div>{/if}

  <ScrollArea class="min-h-0 flex-1 overflow-hidden" aria-busy={refreshing}>
    <div class="relative min-h-full">
      <div class={refreshing ? 'pointer-events-none select-none opacity-40' : undefined}>
        {#if collapsed}
          <nav class="space-y-1 px-2 pb-3" aria-label={tr('Sessions')}>
            {#each visibleProjects as project (project.key)}
              {#each project.sessions as session (hostSessionKey(session))}
                {@const profile = resolveHostProfile(session.host_id)}
                {@const active = activeHostId === session.host_id && activeHostSessionId === session.host_session_id}
                <button type="button" class={[
                  'relative grid h-9 w-full place-items-center rounded-md text-muted-foreground outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/40',
                  active ? 'bg-sidebar-accent text-sidebar-accent-foreground' : 'hover:bg-sidebar-accent/65 hover:text-sidebar-foreground',
                ]} data-session-id={session.session_id} aria-label={`${session.title} · ${profile.label}`} title={`${session.title} · ${profile.label} · ${project.cwd ?? tr(project.kind === 'external' ? 'External sessions' : 'No project')}`} aria-current={active ? 'page' : undefined} onclick={() => onSelect(session.host_id, session.host_session_id)}>
                  <AgentIcon hostId={session.host_id} class="size-5" />
                  {#if session.pending_count > 0}<span class="absolute right-1 top-1 size-1.5 rounded-full bg-primary" aria-label={`${session.pending_count}`}></span>{/if}
                </button>
              {/each}
            {/each}
          </nav>
        {:else}
          <div class="space-y-3 px-2 pb-3">
            {#each visibleProjects as project (project.key)}
              {@const expanded = !!requestSearch.trim() || !collapsedProjects.has(project.key)}
              {@const name = project.name ?? tr(project.kind === 'external' ? 'External sessions' : 'No project')}
              <section aria-label={project.cwd ?? name} data-project-key={project.key}>
                <div class="group/project flex h-8 items-center rounded-md hover:bg-sidebar-accent/50">
                  <button type="button" class="flex h-full min-w-0 flex-1 items-center gap-2 rounded-md px-2 text-left text-[11px] text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/40" aria-expanded={expanded} aria-label={name} title={project.cwd ?? name} onclick={() => toggleProject(project.key)}>
                    {#if expanded}<FolderOpen class="size-4 shrink-0" aria-hidden="true" />{:else}<Folder class="size-4 shrink-0" aria-hidden="true" />{/if}
                    <span class="min-w-0 flex-1 truncate">{name}</span>
                    {#if !expanded && project.pendingCount > 0}<span class="size-1.5 shrink-0 rounded-full bg-primary" aria-label={`${project.pendingCount}`}></span>{/if}
                    {#if expanded}<ChevronDown class="size-3 shrink-0 opacity-0 group-hover/project:opacity-100 group-focus-within/project:opacity-100 pointer-coarse:opacity-100" aria-hidden="true" />{:else}<ChevronRight class="size-3 shrink-0" aria-hidden="true" />{/if}
                  </button>
                  {#if onNewSession && project.cwd}
                    <Button variant="ghost" size="icon-xs" class="mr-1 opacity-0 focus-visible:opacity-100 group-hover/project:opacity-100 group-focus-within/project:opacity-100 pointer-coarse:opacity-100" aria-label={tr('New session in {project}', { project: name })} title={tr('New session in {project}', { project: name })} onclick={() => onNewSession?.(project.cwd!)}><Plus aria-hidden="true" /></Button>
                  {/if}
                </div>
                {#if expanded}
                  <div class="mt-0.5 space-y-0.5">
                    {#each project.sessions as session (hostSessionKey(session))}
                      {@const key = hostSessionKey(session)}
                      {@const profile = resolveHostProfile(session.host_id)}
                      {@const active = activeHostId === session.host_id && activeHostSessionId === session.host_session_id}
                      <div class={[
                        'session-row relative flex min-h-8 min-w-0 items-center rounded-md text-[11px] transition-colors',
                        active ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground' : 'hover:bg-sidebar-accent/55',
                      ]}>
                        <button type="button" class="session-title flex min-h-8 min-w-0 flex-1 items-center gap-1.5 rounded-md py-1.5 pl-8 pr-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring/40" aria-label={`${session.title} · ${profile.label}`} title={`${session.title} · ${profile.label}`} aria-current={active ? 'page' : undefined} onclick={() => onSelect(session.host_id, session.host_session_id)}>
                          <span class="min-w-0 flex-1 truncate">{session.title}</span>
                          {#if session.pending_count > 0}<span class="size-1.5 shrink-0 rounded-full bg-primary" aria-label={`${session.pending_count}`}></span>{/if}
                        </button>
                        <div class="session-actions absolute right-1 top-1/2 flex -translate-y-1/2 items-center">
                          <Button variant="ghost" size="icon-xs" class={session.pinned_at ? 'text-primary' : 'text-muted-foreground'} aria-label={`${session.pinned_at ? tr('Unpin session') : tr('Pin session')}: ${session.title}`} title={session.pinned_at ? tr('Unpin session') : tr('Pin session')} disabled={actionKey !== null} onclick={() => void runAction(`session-pin:${key}`, () => onSetSessionPinned(session, !session.pinned_at))}>
                            {#if session.pinned_at}<PinOff aria-hidden="true" />{:else}<Pin aria-hidden="true" />{/if}
                          </Button>
                          <Button variant="ghost" size="icon-xs" class="text-muted-foreground" aria-label={`${tr('Archive session')}: ${session.title}`} title={tr('Archive session')} disabled={actionKey !== null || session.pending_count > 0} onclick={() => void runAction(`session-archive:${key}`, () => onArchiveSession(session))}><Archive aria-hidden="true" /></Button>
                        </div>
                      </div>
                    {/each}
                  </div>
                {/if}
              </section>
            {:else}
              <div class="px-2 py-8 text-center text-[11px] leading-5 text-muted-foreground">{loading ? tr('Loading host sessions…') : requestSearch ? tr('No matching sessions') : tr('No project sessions yet')}</div>
            {/each}
          </div>
        {/if}
      </div>
      {#if !collapsed && refreshing && sessions.length > 0}<div class="absolute inset-0 z-20 grid place-items-center bg-sidebar/80 backdrop-blur-[1px]"><LoaderCircle class="size-5 animate-spin text-primary" aria-hidden="true" /></div>{/if}
    </div>
  </ScrollArea>
  <div class="shrink-0 border-t border-sidebar-border p-2">
    <Button variant="ghost" class={collapsed ? 'w-full justify-center px-0' : 'w-full justify-start'} aria-label={tr('Settings')} title={collapsed ? tr('Settings') : undefined} onclick={onSettings}><Settings data-icon="inline-start" aria-hidden="true" />{#if !collapsed}{tr('Settings')}{/if}</Button>
  </div>
</aside>

<style>
  .session-actions { opacity: 0; pointer-events: none; }
  .session-row:is(:hover, :has(:focus-visible)) .session-actions { opacity: 1; pointer-events: auto; }
  .session-row:is(:hover, :has(:focus-visible)) .session-title { padding-right: 3.5rem; }

  /* Touch has no hover: session actions must stay reachable and keep clear of the title. */
  @media (pointer: coarse) {
    .session-actions { opacity: 1; pointer-events: auto; }
    .session-title { padding-right: 4.5rem; }
  }
</style>
