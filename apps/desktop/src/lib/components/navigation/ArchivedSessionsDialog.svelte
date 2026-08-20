<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import {
    ArchiveRestore,
    Inbox,
    LoaderCircle,
    RefreshCw,
    Search,
    Trash2,
  } from '@lucide/svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import type { FeedbackRequestSummary, HostSessionSummary } from '$lib/feedback'
  import { requestStatusLabel } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { previewFixtures } from '$lib/previewFixtures'
  import type { HostProfile } from '$lib/workbench/types'

  const ALL_REQUEST_STATUSES = ['waiting', 'in_progress', 'completed', 'cancelled'] as const

  export let open = false
  export let isTauri = false
  export let previewMode = false
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let formatTime: (value: string | null | undefined) => string
  export let messageFrom: (cause: unknown) => string
  export let onError: (message: string) => void = () => {}
  export let onChanged: () => Promise<void> | void = () => {}
  export let onOpenRequest: (requestId: string) => Promise<void> | void = () => {}

  let search = ''
  let loadingSessions = false
  let loadingRequests = false
  let busyKey: string | null = null
  let sessions: HostSessionSummary[] = []
  let requests: FeedbackRequestSummary[] = []
  let selectedSession: HostSessionSummary | null = null
  let openedOnce = false
  let searchTimer: ReturnType<typeof setTimeout> | null = null

  $: if (open && !openedOnce) {
    openedOnce = true
    void loadSessions()
  }
  $: if (!open && openedOnce) {
    openedOnce = false
    selectedSession = null
    requests = []
  }

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function statusClass(status: FeedbackRequestSummary['status']) {
    switch (status) {
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

  function matchesSearch(value: string | null | undefined, query: string) {
    return (value ?? '').toLowerCase().includes(query)
  }

  function previewArchivedSessions(query: string) {
    const normalized = query.trim().toLowerCase()
    return previewFixtures.hostSessions.filter((session) => {
      if (!session.archived_at) return false
      if (!normalized) return true
      return (
        matchesSearch(session.title, normalized) ||
        matchesSearch(session.source_hint, normalized) ||
        matchesSearch(session.host_id, normalized) ||
        matchesSearch(session.host_session_id, normalized) ||
        previewFixtures.requests.some(
          (request) =>
            request.host_id === session.host_id &&
            request.host_session_id === session.host_session_id &&
            (matchesSearch(request.title, normalized) ||
              matchesSearch(request.what_happened, normalized) ||
              matchesSearch(request.source_hint, normalized)),
        )
      )
    })
  }

  function previewArchivedRequests(session: HostSessionSummary, query: string) {
    const normalized = query.trim().toLowerCase()
    return previewFixtures.requests.filter(
      (request) =>
        request.host_id === session.host_id &&
        request.host_session_id === session.host_session_id &&
        (!normalized ||
          matchesSearch(request.title, normalized) ||
          matchesSearch(request.what_happened, normalized) ||
          matchesSearch(request.source_hint, normalized) ||
          matchesSearch(request.request_id, normalized)),
    )
  }

  function scheduleSearch() {
    if (searchTimer) clearTimeout(searchTimer)
    searchTimer = setTimeout(() => void loadSessions(), 220)
  }

  async function loadSessions() {
    loadingSessions = true
    try {
      const nextSessions =
        previewMode || !isTauri
          ? previewArchivedSessions(search)
          : await invoke<HostSessionSummary[]>('list_archived_host_sessions', {
              input: { search: search.trim() || null },
            })
      sessions = nextSessions
      selectedSession =
        nextSessions.find(
          (session) =>
            selectedSession &&
            session.host_id === selectedSession.host_id &&
            session.host_session_id === selectedSession.host_session_id,
        ) ??
        nextSessions[0] ??
        null
      if (selectedSession) await loadRequests(selectedSession)
      else requests = []
    } catch (cause) {
      onError(messageFrom(cause))
    } finally {
      loadingSessions = false
    }
  }

  async function loadRequests(session: HostSessionSummary) {
    loadingRequests = true
    try {
      selectedSession = session
      requests =
        previewMode || !isTauri
          ? previewArchivedRequests(session, search)
          : (
              await invoke<{ requests: FeedbackRequestSummary[]; next_cursor: string | null }>(
                'list_feedback_requests',
                {
                  input: {
                    host_id: session.host_id,
                    host_session_id: session.host_session_id,
                    status: [...ALL_REQUEST_STATUSES],
                    archived: true,
                    search: search.trim() || null,
                    limit: 100,
                    cursor: null,
                  },
                },
              )
            ).requests
    } catch (cause) {
      onError(messageFrom(cause))
    } finally {
      loadingRequests = false
    }
  }

  async function runAction(key: string, action: () => Promise<void> | void) {
    if (busyKey) return
    busyKey = key
    try {
      await action()
      await onChanged()
      await loadSessions()
    } catch (cause) {
      onError(messageFrom(cause))
    } finally {
      busyKey = null
    }
  }

  async function unarchiveSession(session: HostSessionSummary) {
    await runAction(`unarchive:${session.host_id}:${session.host_session_id}`, async () => {
      if (!(previewMode || !isTauri)) {
        await invoke('unarchive_host_session', {
          input: {
            host_id: session.host_id,
            host_session_id: session.host_session_id,
          },
        })
      }
    })
  }

  async function deleteSession(session: HostSessionSummary) {
    if (!confirm(tr('Delete this archived session permanently?'))) return
    await runAction(`delete-session:${session.host_id}:${session.host_session_id}`, async () => {
      if (!(previewMode || !isTauri)) {
        await invoke('delete_host_session', {
          input: {
            host_id: session.host_id,
            host_session_id: session.host_session_id,
          },
        })
      }
    })
  }

  async function deleteRequest(request: FeedbackRequestSummary) {
    if (!confirm(tr('Delete this archived request permanently?'))) return
    await runAction(`delete-request:${request.request_id}`, async () => {
      if (!(previewMode || !isTauri)) {
        await invoke('delete_feedback_request', {
          input: { request_id: request.request_id },
        })
      }
    })
  }

  async function openRequest(request: FeedbackRequestSummary) {
    await onOpenRequest(request.request_id)
    open = false
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="h-[min(760px,calc(100vh-3rem))] max-w-[min(980px,calc(100vw-2rem))] gap-0 overflow-hidden p-0 sm:max-w-[980px]">
    <div class="flex h-14 items-center gap-3 border-b px-4">
      <Dialog.Title class="min-w-0 flex-1 text-sm font-semibold">
        {tr('Archived sessions')}
      </Dialog.Title>
      <Button
        variant="ghost"
        size="icon-sm"
        disabled={loadingSessions}
        aria-label={tr('Refresh archived sessions')}
        title={tr('Refresh archived sessions')}
        onclick={() => void loadSessions()}
      >
        <RefreshCw class={loadingSessions ? 'animate-spin' : ''} />
      </Button>
    </div>

    <div class="border-b p-3">
      <label class="flex h-9 items-center gap-2 rounded-md border border-input bg-background px-2.5 text-sm focus-within:ring-2 focus-within:ring-ring/40">
        <Search class="size-4 shrink-0 text-muted-foreground" />
        <input
          bind:value={search}
          class="min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
          placeholder={tr('Search archived sessions and requests…')}
          oninput={scheduleSearch}
        />
      </label>
    </div>

    <div class="grid min-h-0 flex-1 grid-cols-[280px_minmax(0,1fr)]">
      <section class="min-h-0 border-r">
        <ScrollArea class="h-full">
          {#if loadingSessions && sessions.length === 0}
            <div class="grid h-40 place-items-center text-muted-foreground">
              <LoaderCircle class="size-5 animate-spin" />
            </div>
          {:else if sessions.length === 0}
            <div class="grid place-items-center gap-2 px-6 py-16 text-center">
              <div class="grid size-9 place-items-center rounded-md bg-muted text-muted-foreground">
                <Inbox class="size-4" />
              </div>
              <strong class="text-xs">{tr('No archived sessions')}</strong>
            </div>
          {:else}
            <nav class="p-2" aria-label={tr('Archived sessions')}>
              {#each sessions as session (session.host_id + ':' + session.host_session_id)}
                {@const profile = resolveHostProfile(session.host_id)}
                <button
                  type="button"
                  class={[
                    'flex w-full flex-col gap-1.5 rounded-md px-2.5 py-2.5 text-left text-xs transition-colors',
                    selectedSession?.host_id === session.host_id &&
                    selectedSession?.host_session_id === session.host_session_id
                      ? 'bg-accent text-accent-foreground'
                      : 'hover:bg-muted/75',
                  ]}
                  onclick={() => void loadRequests(session)}
                >
                  <span class="flex w-full items-center gap-2">
                    <span class="grid size-5 shrink-0 place-items-center [&_svg]:size-4">
                      {@html profile.icon_svg}
                    </span>
                    <strong class="min-w-0 flex-1 truncate font-medium">{session.title}</strong>
                    <span class="shrink-0 text-[10px] tabular-nums text-muted-foreground">
                      {session.request_count}
                    </span>
                  </span>
                  <span class="truncate text-[10px] text-muted-foreground">
                    {session.source_hint ?? session.host_session_id}
                  </span>
                  <span class="text-[10px] text-muted-foreground">
                    {formatTime(session.archived_at)}
                  </span>
                </button>
              {/each}
            </nav>
          {/if}
        </ScrollArea>
      </section>

      <section class="flex min-h-0 min-w-0 flex-col">
        {#if selectedSession}
          {@const activeArchivedSession = selectedSession}
          <div class="flex h-14 shrink-0 items-center gap-2 border-b px-3">
            <div class="min-w-0 flex-1">
              <strong class="block truncate text-xs font-semibold">{activeArchivedSession.title}</strong>
              <span class="block truncate text-[10px] text-muted-foreground">
                {activeArchivedSession.host_id} / {activeArchivedSession.host_session_id}
              </span>
            </div>
            <Button
              variant="outline"
              size="sm"
              disabled={busyKey !== null}
              onclick={() => void unarchiveSession(activeArchivedSession)}
            >
              <ArchiveRestore data-icon="inline-start" />
              {tr('Unarchive')}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              disabled={busyKey !== null}
              onclick={() => void deleteSession(activeArchivedSession)}
            >
              <Trash2 data-icon="inline-start" />
              {tr('Delete')}
            </Button>
          </div>

          <ScrollArea class="min-h-0 flex-1">
            {#if loadingRequests}
              <div class="grid h-40 place-items-center text-muted-foreground">
                <LoaderCircle class="size-5 animate-spin" />
              </div>
            {:else if requests.length === 0}
              <div class="grid place-items-center gap-2 px-6 py-16 text-center">
                <div class="grid size-9 place-items-center rounded-md bg-muted text-muted-foreground">
                  <Inbox class="size-4" />
                </div>
                <strong class="text-xs">{tr('No archived requests')}</strong>
              </div>
            {:else}
              <div class="divide-y">
                {#each requests as request (request.request_id)}
                  <article class="grid grid-cols-[minmax(0,1fr)_auto] gap-3 px-4 py-3">
                    <button
                      type="button"
                      class="min-w-0 text-left"
                      onclick={() => void openRequest(request)}
                    >
                      <span class="flex items-center gap-2">
                        <strong class="min-w-0 flex-1 truncate text-xs font-medium">
                          {request.title}
                        </strong>
                        <Badge
                          variant="secondary"
                          class={['h-5 shrink-0 border-0 px-1.5 text-[9px]', statusClass(request.status)]}
                        >
                          {requestStatusLabel(request.status, $locale)}
                        </Badge>
                      </span>
                      <span class="mt-1 line-clamp-2 block text-[11px] leading-4 text-muted-foreground">
                        {request.what_happened}
                      </span>
                      <span class="mt-1 block truncate text-[10px] text-muted-foreground">
                        {request.source_hint ?? request.request_id} · {formatTime(request.updated_at)}
                      </span>
                    </button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      disabled={busyKey !== null}
                      aria-label={tr('Delete request')}
                      title={tr('Delete request')}
                      onclick={() => void deleteRequest(request)}
                    >
                      <Trash2 />
                    </Button>
                  </article>
                {/each}
              </div>
            {/if}
          </ScrollArea>
        {:else}
          <div class="grid flex-1 place-items-center gap-2 px-6 py-16 text-center">
            <div class="grid size-9 place-items-center rounded-md bg-muted text-muted-foreground">
              <Inbox class="size-4" />
            </div>
            <strong class="text-xs">{tr('No archived sessions')}</strong>
          </div>
        {/if}
      </section>
    </div>
  </Dialog.Content>
</Dialog.Root>
