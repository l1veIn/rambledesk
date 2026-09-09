<script lang="ts">
  import { onMount } from 'svelte'
  import AppTitlebar from '$lib/shell/AppTitlebar.svelte'
  import HostSessionRail from '$lib/components/navigation/HostSessionRail.svelte'
  import RequestListPane from '$lib/components/navigation/RequestListPane.svelte'
  import NavigationResizeHandle from '$lib/components/navigation/NavigationResizeHandle.svelte'
  import { COLLAPSED_RAIL_WIDTH, RAIL_LIMITS, fitNavigationWidths } from '$lib/components/navigation/railResize'
  import { previewFixtures } from '$lib/previewFixtures'
  import type { HostProfile } from '$lib/workbench/types'
  import {
    initialHostRailCollapsed, initialRequestRailCollapsed,
    initialHostRailWidth, initialRequestRailWidth,
    saveHostRailCollapsed, saveRequestRailCollapsed,
    saveHostRailWidth, saveRequestRailWidth,
  } from '$lib/uiPreferences'
  import { appearancePreferences, updateAppearance } from '$lib/appearance/appearancePreferences'

  // A long list catches height/scroll regressions that a few fixture rows hide.
  const sessions = [
    ...previewFixtures.hostSessions,
    ...Array.from({ length: 60 }, (_, index) => ({
      ...previewFixtures.hostSessions[0],
      host_session_id: `resize-preview-${index}`,
      session_id: `resize-preview-${index}`,
      title: `Navigation scroll check ${index + 1}`,
      pending_count: 0,
      pinned_at: null,
    })),
  ]

  let hostWidth = initialHostRailWidth()
  let requestWidth = initialRequestRailWidth()
  let hostCollapsed = initialHostRailCollapsed()
  let requestCollapsed = initialRequestRailCollapsed()
  let containerWidth = 0
  let hostMeasuredWidth = 0
  let requestMeasuredWidth = 0
  let workspaceMeasuredWidth = 0
  let hostDragging = false
  let requestDragging = false
  let hostCommits = 0
  let requestCommits = 0
  let mounted = false
  let lastHost = `${hostWidth}/${hostCollapsed}`
  let lastRequest = `${requestWidth}/${requestCollapsed}`
  let activeHostId: string | null = previewFixtures.hostSessions[0].host_id
  let activeHostSessionId: string | null = previewFixtures.hostSessions[0].host_session_id
  let activeRequestId = previewFixtures.requests[0].request_id
  let requestSearch = ''

  $: widths = fitNavigationWidths({ hostWidth, requestWidth, hostCollapsed, requestCollapsed, containerWidth })
  $: hostMaxWidth = Math.max(RAIL_LIMITS.host.minWidth, Math.min(RAIL_LIMITS.host.maxWidth, containerWidth - widths.request - 640))
  $: requestMaxWidth = Math.max(RAIL_LIMITS.request.minWidth, Math.min(RAIL_LIMITS.request.maxWidth, containerWidth - widths.host - 640))
  $: activeRequest = previewFixtures.requests.find(request => request.request_id === activeRequestId) ?? previewFixtures.requests[0]
  $: if (mounted && !hostDragging) persistHost(hostWidth, hostCollapsed)
  $: if (mounted && !requestDragging) persistRequest(requestWidth, requestCollapsed)

  onMount(() => { mounted = true })

  function profile(hostId: string): HostProfile {
    return previewFixtures.hostProfiles.find(profile => profile.id === hostId) ?? previewFixtures.hostProfiles[0]
  }
  function persistHost(width = hostWidth, collapsed = hostCollapsed) {
    const key = `${width}/${collapsed}`
    if (key === lastHost) return
    saveHostRailWidth(width)
    saveHostRailCollapsed(collapsed)
    lastHost = key
    hostCommits += 1
  }
  function persistRequest(width = requestWidth, collapsed = requestCollapsed) {
    const key = `${width}/${collapsed}`
    if (key === lastRequest) return
    saveRequestRailWidth(width)
    saveRequestRailCollapsed(collapsed)
    lastRequest = key
    requestCommits += 1
  }
  function resetWidths() {
    hostWidth = RAIL_LIMITS.host.defaultWidth
    requestWidth = RAIL_LIMITS.request.defaultWidth
    hostCollapsed = false
    requestCollapsed = false
    persistHost()
    persistRequest()
    updateAppearance({ zoom: 100 })
  }
</script>

<main
  class="preview-root flex h-full w-full flex-col overflow-hidden rounded-[16px] border bg-background text-foreground"
  class:navigation-resizing={hostDragging || requestDragging}
  style:--workbench-sidebar-width={`${widths.host}px`}
  style:--workbench-sidebar-collapsed-width={`${COLLAPSED_RAIL_WIDTH}px`}
  data-preview-state
  data-host-width={hostWidth}
  data-request-width={requestWidth}
  data-host-display-width={widths.host}
  data-request-display-width={widths.request}
  data-host-collapsed={hostCollapsed}
  data-request-collapsed={requestCollapsed}
  data-host-dragging={hostDragging}
  data-request-dragging={requestDragging}
  data-zoom={$appearancePreferences.zoom}
>
  <AppTitlebar sidebarCollapsed={hostCollapsed} pendingCount={2}>
    {#snippet workspaceTabs()}
      <div class="flex h-full min-w-0 items-stretch" role="tablist" aria-label="Preview workspace tabs">
        <button class="min-w-0 max-w-72 truncate rounded-t-lg bg-background px-4 text-xs" role="tab" aria-selected="true" data-workspace-tab-item>Review navigation sizing</button>
      </div>
    {/snippet}
  </AppTitlebar>
  <div class="flex min-h-0 min-w-0 flex-1" bind:clientWidth={containerWidth}>
    <div id="preview-host-rail" data-navigation-pane class="preview-rail relative flex min-h-0 shrink-0" style:width={`${widths.host}px`} bind:clientWidth={hostMeasuredWidth}>
      <HostSessionRail
        bind:collapsed={hostCollapsed}
        {sessions}
        {activeHostId}
        {activeHostSessionId}
        inboxActive={activeHostId === null}
        {requestSearch}
        resolveHostProfile={profile}
        onSelect={(hostId, sessionId) => { activeHostId = hostId; activeHostSessionId = sessionId }}
        onRequestSearch={(search) => requestSearch = search}
        onNewSession={() => {}}
      />
      <NavigationResizeHandle
        label="Resize project sidebar" controls="preview-host-rail"
        expandedWidth={hostWidth} displayWidth={widths.host} collapsed={hostCollapsed}
        minWidth={RAIL_LIMITS.host.minWidth} maxWidth={hostMaxWidth}
        onResize={(next) => { hostWidth = next.width; hostCollapsed = next.collapsed }}
        onCommit={() => persistHost()}
        onDraggingChange={(active) => hostDragging = active}
      />
    </div>
    <div class="appearance-workspace flex min-h-0 min-w-0 flex-1">
      <div id="preview-request-rail" data-navigation-pane class="preview-rail relative min-h-0 shrink-0 border-r" style:width={`${widths.request}px`} bind:clientWidth={requestMeasuredWidth}>
        <RequestListPane bind:collapsed={requestCollapsed}
          requests={previewFixtures.requests} {activeRequestId}
          scopeLabel="All preview requests" resolveHostProfile={profile}
          formatTime={(value) => value?.slice(11, 16) ?? ''}
          onOpenRequest={(requestId) => activeRequestId = requestId}
        />
        <NavigationResizeHandle
          label="Resize request list" controls="preview-request-rail"
          expandedWidth={requestWidth} displayWidth={widths.request} collapsed={requestCollapsed}
          minWidth={RAIL_LIMITS.request.minWidth} maxWidth={requestMaxWidth}
          onResize={(next) => { requestWidth = next.width; requestCollapsed = next.collapsed }}
          onCommit={() => persistRequest()}
          onDraggingChange={(active) => requestDragging = active}
        />
      </div>
      <section class="workspace-panel min-h-0 min-w-0 flex-1 overflow-auto p-5" bind:clientWidth={workspaceMeasuredWidth} aria-label="Preview workspace">
        <p class="m-0 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">RambleDesk development preview</p>
        <h1 class="mt-3 text-lg font-semibold">{activeRequest.title}</h1>
        <p class="mt-3 max-w-xl text-sm leading-6 text-muted-foreground">Drag either navigation divider, use the existing collapse buttons, and reload to check the saved widths. This fixture stores preferences under its own preview prefix.</p>
        <div class="my-5 flex flex-wrap gap-2" aria-label="Preview controls">
          <button class="rounded-md border px-3 py-1.5 text-xs" aria-pressed={$appearancePreferences.zoom === 100} onclick={() => updateAppearance({ zoom: 100 })}>100% zoom</button>
          <button class="rounded-md border px-3 py-1.5 text-xs" aria-pressed={$appearancePreferences.zoom === 150} onclick={() => updateAppearance({ zoom: 150 })}>150% zoom</button>
          <button class="rounded-md border px-3 py-1.5 text-xs" onclick={resetWidths}>Reset preview widths</button>
        </div>
        <dl class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-2 rounded-lg border bg-card p-4 text-xs tabular-nums" aria-label="Navigation sizing measurements">
          <dt>Host saved / displayed / measured</dt><dd data-host-measurement>{hostWidth} / {widths.host} / {hostMeasuredWidth}</dd>
          <dt>Request saved / displayed / measured</dt><dd data-request-measurement>{requestWidth} / {widths.request} / {requestMeasuredWidth}</dd>
          <dt>Container / workspace</dt><dd data-container-measurement>{containerWidth} / {workspaceMeasuredWidth}</dd>
          <dt>Host / request collapsed</dt><dd>{String(hostCollapsed)} / {String(requestCollapsed)}</dd>
          <dt>Host / request dragging</dt><dd data-dragging-measurement>{String(hostDragging)} / {String(requestDragging)}</dd>
          <dt>Host / request preference commits</dt><dd data-commit-measurement>{hostCommits} / {requestCommits}</dd>
        </dl>
        <p class="mt-5 text-sm leading-7">{activeRequest.what_happened}</p>
        <div class="mt-5 min-h-40 rounded-xl border bg-muted/20 p-5 text-sm leading-7 text-muted-foreground">Workspace content remains usable while the navigation panes change size. 项目和请求列表调整宽度时，正文仍保留可用空间。</div>
      </section>
    </div>
  </div>
</main>
