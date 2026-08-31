<script lang="ts">
  import {
    ArchiveRestore,
    ChevronDown,
    ChevronRight,
    Inbox,
    LoaderCircle,
    MessageSquareText,
    RefreshCw,
    Search,
    Trash2,
    X,
  } from '@lucide/svelte'
  import { Badge } from '$lib/components/ui/badge'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import type { FeedbackRequestSummary, FeedbackWorkspaceView, HostSessionSummary } from '$lib/feedback'
  import { requestStatusLabel } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    normalizePublishedFeedback,
    type PublishedFeedbackView,
  } from '$lib/publishedFeedback'
  import { previewFixtures, previewWorkspaceFor } from '$lib/previewFixtures'
  import type { HostProfile } from '$lib/workbench/types'
  import type { SessionViewDescriptor } from '$lib/workspace/viewDescriptors'

  const ALL_REQUEST_STATUSES = ['waiting', 'in_progress', 'completed', 'cancelled'] as const

  type SelectedArchivedItem =
    | { kind: 'session'; sessionKey: string }
    | { kind: 'request'; sessionKey: string; requestId: string }

  type ArchivedRequestDetails = {
    workspace: FeedbackWorkspaceView
    publishedFeedback: PublishedFeedbackView | null
  }

  export let open = false
  export let isTauri = false
  export let transport: ApplicationTransport
  export let previewMode = false
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let formatTime: (value: string | null | undefined) => string
  export let messageFrom: (cause: unknown) => string
  export let initialSession: SessionViewDescriptor | null = null
  export let onError: (message: string) => void = () => {}
  export let onChanged: () => Promise<void> | void = () => {}

  let search = ''
  let loading = false
  let busyKey: string | null = null
  let sessions: HostSessionSummary[] = []
  let requestsBySession: Record<string, FeedbackRequestSummary[]> = {}
  let requestDetailsById: Record<string, ArchivedRequestDetails> = {}
  let expandedSessions = new Set<string>()
  let selected: SelectedArchivedItem | null = null
  let detailLoadingRequestId: string | null = null
  let openedOnce = false
  let applyInitialSession = false
  let searchTimer: ReturnType<typeof setTimeout> | null = null

  $: activeSession = selected ? sessionForKey(selected.sessionKey) : null
  $: activeRequests = activeSession ? requestsFor(activeSession) : []
  $: selectedRequestId = selected?.kind === 'request' ? selected.requestId : null
  $: activeRequest = selectedRequestId
    ? activeRequests.find((request) => request.request_id === selectedRequestId) ?? null
    : null
  $: activeRequestDetails = activeRequest
    ? requestDetailsById[activeRequest.request_id] ?? null
    : null
  $: activePublishedFeedback = activeRequestDetails?.publishedFeedback ?? null
  $: activeUncookedMarkdown =
    activePublishedFeedback?.uncooked_markdown ?? activeRequestDetails?.workspace.draft.body_markdown ?? ''

  $: if (open && !openedOnce) {
    openedOnce = true
    applyInitialSession = true
    void loadArchive()
  }
  $: if (!open && openedOnce) {
    openedOnce = false
    sessions = []
    requestsBySession = {}
    requestDetailsById = {}
    selected = null
    detailLoadingRequestId = null
  }

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function sessionKey(session: HostSessionSummary) {
    return `${session.host_id}\u0000${session.host_session_id}`
  }

  function sessionForKey(key: string) {
    return sessions.find((session) => sessionKey(session) === key) ?? null
  }

  function requestsFor(session: HostSessionSummary) {
    return requestsBySession[sessionKey(session)] ?? []
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

  function escapeHtml(value: string) {
    return value
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;')
  }

  function highlighted(value: string | null | undefined) {
    const text = value ?? ''
    const query = search.trim()
    if (!query) return escapeHtml(text)
    const lowerText = text.toLowerCase()
    const lowerQuery = query.toLowerCase()
    let cursor = 0
    let output = ''
    while (cursor < text.length) {
      const index = lowerText.indexOf(lowerQuery, cursor)
      if (index === -1) {
        output += escapeHtml(text.slice(cursor))
        break
      }
      output += escapeHtml(text.slice(cursor, index))
      output += `<mark class="rounded-sm bg-primary/25 px-0.5 text-inherit">${escapeHtml(
        text.slice(index, index + query.length),
      )}</mark>`
      cursor = index + query.length
    }
    return output
  }

  function requestMatchesSession(request: FeedbackRequestSummary, session: HostSessionSummary) {
    return request.host_id === session.host_id && request.host_session_id === session.host_session_id
  }

  function previewArchivedSessions(query: string) {
    const normalized = query.trim().toLowerCase()
    return previewFixtures.archivedHostSessions.filter((session) => {
      if (!normalized) return true
      return (
        matchesSearch(session.title, normalized) ||
        matchesSearch(session.source_hint, normalized) ||
        matchesSearch(session.host_id, normalized) ||
        matchesSearch(session.host_session_id, normalized) ||
        previewFixtures.requests.some(
          (request) =>
            requestMatchesSession(request, session) &&
            (matchesSearch(request.title, normalized) ||
              matchesSearch(request.what_happened, normalized) ||
              matchesSearch(request.source_hint, normalized) ||
              matchesSearch(request.request_id, normalized)),
        )
      )
    })
  }

  function previewArchivedRequests(session: HostSessionSummary, query: string) {
    const normalized = query.trim().toLowerCase()
    return previewFixtures.requests.filter(
      (request) =>
        requestMatchesSession(request, session) &&
        (!normalized ||
          matchesSearch(request.title, normalized) ||
          matchesSearch(request.what_happened, normalized) ||
          matchesSearch(request.source_hint, normalized) ||
          matchesSearch(request.request_id, normalized) ||
          matchesSearch(request.host_id, normalized) ||
          matchesSearch(request.host_session_id, normalized)),
    )
  }

  function selectionExists(
    item: SelectedArchivedItem | null,
    nextSessions: HostSessionSummary[],
    nextRequests: Record<string, FeedbackRequestSummary[]>,
  ) {
    if (!item) return false
    if (!nextSessions.some((session) => sessionKey(session) === item.sessionKey)) return false
    if (item.kind === 'session') return true
    return (nextRequests[item.sessionKey] ?? []).some(
      (request) => request.request_id === item.requestId,
    )
  }

  function scheduleSearch() {
    if (searchTimer) clearTimeout(searchTimer)
    searchTimer = setTimeout(() => void loadArchive(), 220)
  }

  async function fetchSessionRequests(session: HostSessionSummary) {
    if (previewMode || !isTauri) return previewArchivedRequests(session, search)
    return (
      await transport.call('listFeedbackRequests', {
        host_id: session.host_id,
        host_session_id: session.host_session_id,
        status: [...ALL_REQUEST_STATUSES],
        archived: true,
        search: search.trim() || null,
        limit: 100,
        cursor: null,
      })
    ).requests
  }

  async function loadArchive() {
    loading = true
    try {
      const nextSessions =
        previewMode || !isTauri
          ? previewArchivedSessions(search)
          : await transport.call('listArchivedHostSessions', {
              search: search.trim() || null,
            })
      const entries = await Promise.all(
        nextSessions.map(
          async (session) => [sessionKey(session), await fetchSessionRequests(session)] as const,
        ),
      )
      const nextRequests = Object.fromEntries(entries)
      sessions = nextSessions
      requestsBySession = nextRequests

      const nextExpanded = search.trim() ? new Set(nextSessions.map(sessionKey)) : new Set(expandedSessions)
      const requestedSession = applyInitialSession && initialSession
        ? nextSessions.find(
            (session) =>
              session.host_id === initialSession?.hostId &&
              session.host_session_id === initialSession?.hostSessionId,
          )
        : null
      applyInitialSession = false
      if (requestedSession) {
        const requestedKey = sessionKey(requestedSession)
        selected = { kind: 'session', sessionKey: requestedKey }
        expandedSessions = nextExpanded.add(requestedKey)
      } else if (selectionExists(selected, nextSessions, nextRequests)) {
        expandedSessions = nextExpanded.add(selected!.sessionKey)
      } else if (nextSessions[0]) {
        const firstKey = sessionKey(nextSessions[0])
        selected = { kind: 'session', sessionKey: firstKey }
        expandedSessions = nextExpanded.add(firstKey)
      } else {
        selected = null
        expandedSessions = nextExpanded
      }
    } catch (cause) {
      onError(messageFrom(cause))
    } finally {
      loading = false
    }
  }

  function toggleSession(session: HostSessionSummary) {
    const key = sessionKey(session)
    const next = new Set(expandedSessions)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    expandedSessions = next
  }

  function selectSession(session: HostSessionSummary) {
    const key = sessionKey(session)
    selected = { kind: 'session', sessionKey: key }
    expandedSessions = new Set(expandedSessions).add(key)
  }

  function selectRequest(session: HostSessionSummary, request: FeedbackRequestSummary) {
    const key = sessionKey(session)
    selected = { kind: 'request', sessionKey: key, requestId: request.request_id }
    expandedSessions = new Set(expandedSessions).add(key)
    void loadRequestDetails(request)
  }

  async function loadRequestDetails(request: FeedbackRequestSummary) {
    if (requestDetailsById[request.request_id] || detailLoadingRequestId === request.request_id) return
    detailLoadingRequestId = request.request_id
    try {
      const workspace =
        previewMode || !isTauri
          ? previewWorkspaceFor(request.request_id)
          : await transport.call('getFeedbackWorkspace', {
              request_id: request.request_id,
            })
      if (!workspace) throw new Error(tr('This feedback request could not be found.'))
      const publishedFeedback =
        workspace.request.status === 'completed' && workspace.feedback
          ? previewMode || !isTauri
            ? {
                markdown: workspace.draft.body_markdown,
                uncooked_markdown: workspace.draft.body_markdown,
              }
            : normalizePublishedFeedback(
                await transport.call('readPublishedFeedback', {
                  request_id: request.request_id,
                }),
              )
          : null
      requestDetailsById = {
        ...requestDetailsById,
        [request.request_id]: { workspace, publishedFeedback },
      }
    } catch (cause) {
      onError(messageFrom(cause))
    } finally {
      if (detailLoadingRequestId === request.request_id) detailLoadingRequestId = null
    }
  }

  async function runAction(key: string, action: () => Promise<void> | void) {
    if (busyKey) return
    busyKey = key
    try {
      await action()
      await onChanged()
      await loadArchive()
    } catch (cause) {
      onError(messageFrom(cause))
    } finally {
      busyKey = null
    }
  }

  async function unarchiveSession(session: HostSessionSummary) {
    await runAction(`unarchive:${session.host_id}:${session.host_session_id}`, async () => {
      if (!(previewMode || !isTauri)) {
        await transport.call('unarchiveHostSession', {
          host_id: session.host_id,
          host_session_id: session.host_session_id,
        })
      }
    })
  }

  async function deleteSession(session: HostSessionSummary) {
    if (!confirm(tr('Delete this archived session permanently?'))) return
    await runAction(`delete-session:${session.host_id}:${session.host_session_id}`, async () => {
      if (!(previewMode || !isTauri)) {
        await transport.call('deleteHostSession', {
          host_id: session.host_id,
          host_session_id: session.host_session_id,
        })
      }
    })
  }

  async function deleteRequest(request: FeedbackRequestSummary) {
    if (!confirm(tr('Delete this archived request permanently?'))) return
    await runAction(`delete-request:${request.request_id}`, async () => {
      if (!(previewMode || !isTauri)) {
        await transport.call('deleteFeedbackRequest', { request_id: request.request_id })
      }
    })
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    showCloseButton={false}
    class="grid h-[min(760px,calc(100vh-3rem))] w-[min(1060px,calc(100vw-2rem))] max-w-none grid-rows-[56px_minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-none"
  >
    <div class="flex min-h-0 items-center gap-2 border-b px-4">
      <Dialog.Title class="min-w-0 flex-1 text-sm font-semibold">
        {tr('Archived sessions')}
      </Dialog.Title>
      <Button
        variant="ghost"
        size="icon-sm"
        disabled={loading}
        aria-label={tr('Refresh archived sessions')}
        title={tr('Refresh archived sessions')}
        onclick={() => void loadArchive()}
      >
        <RefreshCw class={loading ? 'animate-spin' : ''} />
      </Button>
      <Dialog.Close
        class="inline-flex size-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        aria-label={tr('Close')}
      >
        <X class="size-4" />
      </Dialog.Close>
    </div>

    <div class="grid min-h-0 overflow-hidden grid-cols-[minmax(0,320px)_minmax(0,1fr)]">
      <aside class="grid min-h-0 min-w-0 overflow-hidden grid-rows-[56px_minmax(0,1fr)] border-r bg-muted/20">
        <div class="min-w-0 border-b p-3">
          <label class="flex h-9 min-w-0 max-w-full items-center gap-2 rounded-md border border-input bg-background px-2.5 text-sm focus-within:ring-2 focus-within:ring-ring/40">
            <Search class="size-4 shrink-0 text-muted-foreground" />
            <input
              bind:value={search}
              class="min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
              placeholder={tr('Search archived sessions and requests…')}
              oninput={scheduleSearch}
            />
          </label>
        </div>

        <ScrollArea class="min-h-0 min-w-0 overflow-hidden">
          {#if loading && sessions.length === 0}
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
            <nav class="min-w-0 space-y-1 overflow-hidden p-2" aria-label={tr('Archived sessions')}>
              {#each sessions as session (sessionKey(session))}
                {@const key = sessionKey(session)}
                {@const profile = resolveHostProfile(session.host_id)}
                {@const sessionRequests = requestsFor(session)}
                <div class="min-w-0 overflow-hidden">
                  <div class="flex min-w-0 items-start gap-1">
                    <button
                      type="button"
                      class="mt-1 grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-muted"
                      aria-label={expandedSessions.has(key) ? tr('Collapse sessions') : tr('Expand sessions')}
                      title={expandedSessions.has(key) ? tr('Collapse sessions') : tr('Expand sessions')}
                      onclick={() => toggleSession(session)}
                    >
                      {#if expandedSessions.has(key)}
                        <ChevronDown class="size-4" />
                      {:else}
                        <ChevronRight class="size-4" />
                      {/if}
                    </button>
                    <button
                      type="button"
                      class={[
                        'flex min-w-0 flex-1 flex-col gap-1 overflow-hidden rounded-md px-2.5 py-2 text-left text-xs transition-colors',
                        selected?.kind === 'session' && selected.sessionKey === key
                          ? 'bg-accent text-accent-foreground'
                          : 'hover:bg-muted/75',
                      ]}
                      onclick={() => selectSession(session)}
                    >
                      <span class="flex min-w-0 max-w-full items-center gap-2">
                        <span class="grid size-5 shrink-0 place-items-center [&_svg]:size-4">
                          {@html profile.icon_svg}
                        </span>
                        <strong class="min-w-0 flex-1 truncate font-medium">
                          {@html highlighted(session.title)}
                        </strong>
                        <span class="shrink-0 text-[10px] tabular-nums text-muted-foreground">
                          {session.request_count}
                        </span>
                      </span>
                      <span class="max-w-full truncate text-[10px] text-muted-foreground">
                        {@html highlighted(session.source_hint ?? session.host_session_id)}
                      </span>
                    </button>
                  </div>

                  {#if expandedSessions.has(key)}
                    <div class="ml-9 min-w-0 overflow-hidden border-l pl-2">
                      {#each sessionRequests as request (request.request_id)}
                        <button
                          type="button"
                          class={[
                            'flex w-full min-w-0 flex-col gap-1 overflow-hidden rounded-md px-2 py-2 text-left text-[11px] transition-colors',
                            selected?.kind === 'request' && selected.requestId === request.request_id
                              ? 'bg-accent text-accent-foreground'
                              : 'text-muted-foreground hover:bg-muted/70 hover:text-foreground',
                          ]}
                          onclick={() => selectRequest(session, request)}
                        >
                          <span class="flex min-w-0 items-center gap-2">
                            <MessageSquareText class="size-3.5 shrink-0" />
                            <span class="min-w-0 flex-1 truncate font-medium">
                              {@html highlighted(request.title)}
                            </span>
                            <Badge
                              variant="secondary"
                              class={['h-4 shrink-0 border-0 px-1 text-[8px]', statusClass(request.status)]}
                            >
                              {requestStatusLabel(request.status, $locale)}
                            </Badge>
                          </span>
                          <span class="line-clamp-2 max-w-full leading-4">
                            {@html highlighted(request.what_happened)}
                          </span>
                        </button>
                      {:else}
                        <div class="px-2 py-2 text-[11px] text-muted-foreground">
                          {tr('No archived requests')}
                        </div>
                      {/each}
                    </div>
                  {/if}
                </div>
              {/each}
            </nav>
          {/if}
        </ScrollArea>
      </aside>

      <section class="grid min-h-0 min-w-0 overflow-hidden grid-rows-[72px_minmax(0,1fr)]">
        {#if activeSession && activeRequest}
          <div class="flex min-h-0 items-center gap-2 border-b px-5">
            <div class="min-w-0 flex-1">
              <p class="m-0 text-[10px] font-medium uppercase text-muted-foreground">
                {tr('Request details')}
              </p>
              <h2 class="m-0 mt-1 truncate text-base font-semibold">{activeRequest.title}</h2>
            </div>
            <Badge
              variant="secondary"
              class={['h-6 shrink-0 border-0 px-2 text-[10px]', statusClass(activeRequest.status)]}
            >
              {requestStatusLabel(activeRequest.status, $locale)}
            </Badge>
            <Button
              variant="destructive"
              size="sm"
              disabled={busyKey !== null}
              onclick={() => void deleteRequest(activeRequest)}
            >
              <Trash2 data-icon="inline-start" />
              {tr('Delete request')}
            </Button>
          </div>

          <ScrollArea class="min-h-0 min-w-0 overflow-hidden">
            <div class="min-w-0 space-y-6 p-5">
              <div class="grid grid-cols-2 gap-x-6 gap-y-3 text-xs">
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Host')}</span>
                  <span class="mt-1 block truncate">{activeRequest.host_id}</span>
                </div>
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Session')}</span>
                  <span class="mt-1 block truncate">{activeRequest.host_session_id}</span>
                </div>
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Request ID')}</span>
                  <span class="mt-1 block truncate font-mono text-[11px]">{activeRequest.request_id}</span>
                </div>
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Updated')}</span>
                  <span class="mt-1 block">{formatTime(activeRequest.updated_at)}</span>
                </div>
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Created')}</span>
                  <span class="mt-1 block">{formatTime(activeRequest.created_at)}</span>
                </div>
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Source')}</span>
                  <span class="mt-1 block truncate">{activeRequest.source_hint ?? '—'}</span>
                </div>
              </div>

              <section>
                <h3 class="m-0 text-xs font-semibold">{tr('User feedback')}</h3>
                {#if activePublishedFeedback?.markdown}
                  <p class="m-0 mt-2 whitespace-pre-wrap break-words rounded-md border bg-muted/20 p-3 text-sm leading-6 text-muted-foreground">
                    {activePublishedFeedback.markdown}
                  </p>
                {:else if activeRequest.final_summary}
                  <p class="m-0 mt-2 whitespace-pre-wrap break-words rounded-md border bg-muted/20 p-3 text-sm leading-6 text-muted-foreground">
                    {activeRequest.final_summary}
                  </p>
                {:else if detailLoadingRequestId === activeRequest.request_id && !activeRequestDetails}
                  <div class="mt-2 flex items-center gap-2 rounded-md border bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
                    <LoaderCircle class="size-3.5 animate-spin" />
                    {tr('Loading request details…')}
                  </div>
                {:else}
                  <p class="m-0 mt-2 text-xs text-muted-foreground">
                    {tr('No published feedback yet.')}
                  </p>
                {/if}
              </section>

              {#if activeUncookedMarkdown}
                <section>
                  <h3 class="m-0 text-xs font-semibold">{tr('Uncooked feedback')}</h3>
                  <p class="m-0 mt-2 whitespace-pre-wrap break-words rounded-md border bg-muted/20 p-3 text-sm leading-6 text-muted-foreground">
                    {activeUncookedMarkdown}
                  </p>
                </section>
              {/if}

              <section>
                <h3 class="m-0 text-xs font-semibold">{tr('What happened')}</h3>
                <p class="m-0 mt-2 whitespace-pre-wrap break-words text-sm leading-6 text-muted-foreground">
                  {activeRequest.what_happened}
                </p>
              </section>

              {#if activeRequest.final_summary}
                <section>
                  <h3 class="m-0 text-xs font-semibold">{tr('Final summary')}</h3>
                  <p class="m-0 mt-2 whitespace-pre-wrap break-words text-sm leading-6 text-muted-foreground">
                    {activeRequest.final_summary}
                  </p>
                </section>
              {/if}
            </div>
          </ScrollArea>
        {:else if activeSession}
          <div class="flex min-h-0 items-center gap-2 border-b px-5">
            <div class="min-w-0 flex-1">
              <p class="m-0 text-[10px] font-medium uppercase text-muted-foreground">
                {tr('Session details')}
              </p>
              <h2 class="m-0 mt-1 truncate text-base font-semibold">{activeSession.title}</h2>
              <p class="m-0 mt-1 truncate text-[11px] text-muted-foreground">
                {activeSession.host_id} / {activeSession.host_session_id}
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              disabled={busyKey !== null}
              onclick={() => void unarchiveSession(activeSession)}
            >
              <ArchiveRestore data-icon="inline-start" />
              {tr('Unarchive')}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              disabled={busyKey !== null}
              onclick={() => void deleteSession(activeSession)}
            >
              <Trash2 data-icon="inline-start" />
              {tr('Delete')}
            </Button>
          </div>

          <ScrollArea class="min-h-0 min-w-0 overflow-hidden">
            <div class="min-w-0 space-y-6 p-5">
              <div class="grid grid-cols-3 gap-x-6 gap-y-3 text-xs">
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Host')}</span>
                  <span class="mt-1 block truncate">{activeSession.host_id}</span>
                </div>
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Requests')}</span>
                  <span class="mt-1 block">{activeSession.request_count}</span>
                </div>
                <div>
                  <span class="block text-[10px] font-medium uppercase text-muted-foreground">{tr('Archived at')}</span>
                  <span class="mt-1 block">{formatTime(activeSession.archived_at)}</span>
                </div>
              </div>

              <section>
                <h3 class="m-0 text-xs font-semibold">{tr('Ramble requests')}</h3>
                <div class="mt-3 min-w-0 overflow-hidden rounded-md border">
                  {#each activeRequests as request (request.request_id)}
                    <button
                      type="button"
                      class="grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-3 px-3 py-3 text-left transition-colors hover:bg-muted/65"
                      onclick={() => selectRequest(activeSession, request)}
                    >
                      <span class="min-w-0">
                        <strong class="block truncate text-xs font-medium">{request.title}</strong>
                        <span class="mt-1 line-clamp-2 block break-words text-[11px] leading-4 text-muted-foreground">
                          {request.what_happened}
                        </span>
                      </span>
                      <span class="text-right">
                        <Badge
                          variant="secondary"
                          class={['h-5 border-0 px-1.5 text-[9px]', statusClass(request.status)]}
                        >
                          {requestStatusLabel(request.status, $locale)}
                        </Badge>
                        <span class="mt-1 block text-[10px] text-muted-foreground">
                          {formatTime(request.updated_at)}
                        </span>
                      </span>
                    </button>
                  {:else}
                    <div class="px-3 py-8 text-center text-xs text-muted-foreground">
                      {tr('No archived requests')}
                    </div>
                  {/each}
                </div>
              </section>
            </div>
          </ScrollArea>
        {:else}
          <div class="row-span-2 grid place-items-center gap-2 px-6 py-16 text-center">
            <div class="grid size-9 place-items-center rounded-md bg-muted text-muted-foreground">
              <Inbox class="size-4" />
            </div>
            <strong class="text-xs">{tr('Select a session or request')}</strong>
          </div>
        {/if}
      </section>
    </div>
  </Dialog.Content>
</Dialog.Root>
