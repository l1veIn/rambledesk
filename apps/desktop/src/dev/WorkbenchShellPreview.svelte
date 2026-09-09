<script lang="ts">
  import AppTitlebar from '$lib/shell/AppTitlebar.svelte'
  import HostSessionRail from '$lib/components/navigation/HostSessionRail.svelte'
  import RequestListPane from '$lib/components/navigation/RequestListPane.svelte'
  import { COLLAPSED_RAIL_WIDTH } from '$lib/components/navigation/railResize'
  import { previewFixtures } from '$lib/previewFixtures'
  import WorkbenchShell from '$lib/workbench/WorkbenchShell.svelte'
  import type { ShellMode } from '$lib/workbench/shellMode'
  import type { HostProfile } from '$lib/workbench/types'
  import { initialHostRailCollapsed, initialRequestRailCollapsed, saveHostRailCollapsed, saveRequestRailCollapsed } from '$lib/uiPreferences'

  // A long list catches height/scroll regressions that a few fixture rows hide.
  const sessions = [
    ...previewFixtures.hostSessions,
    ...Array.from({ length: 40 }, (_, index) => ({
      ...previewFixtures.hostSessions[0],
      host_session_id: `shell-preview-${index}`,
      session_id: `shell-preview-${index}`,
      title: `Drawer scroll check ${index + 1}`,
      pending_count: 0,
      pinned_at: null,
    })),
  ]

  // Mirrors App.svelte: the persisted preference is desktop-only; phones drive drawers.
  let mode: ShellMode = 'desktop'
  let hostPreference = initialHostRailCollapsed()
  let requestPreference = initialRequestRailCollapsed()
  let phoneHostRailOpen = false
  let phoneRequestRailOpen = false
  let hostDisplayWidth = 0
  let resizing = false
  let activeHostId: string | null = previewFixtures.hostSessions[0].host_id
  let activeHostSessionId: string | null = previewFixtures.hostSessions[0].host_session_id
  let activeRequestId = previewFixtures.requests[0].request_id

  $: hostCollapsed = mode === 'phone' ? !phoneHostRailOpen : hostPreference
  $: requestCollapsed = mode === 'phone' ? !phoneRequestRailOpen : requestPreference
  $: saveHostRailCollapsed(hostPreference)
  $: saveRequestRailCollapsed(requestPreference)
  $: activeRequest = previewFixtures.requests.find((request) => request.request_id === activeRequestId) ?? previewFixtures.requests[0]

  function setHostRailCollapsed(collapsed: boolean) {
    if (mode === 'phone') {
      phoneHostRailOpen = !collapsed
      if (!collapsed) phoneRequestRailOpen = false
    } else {
      hostPreference = collapsed
    }
  }

  function setRequestRailCollapsed(collapsed: boolean) {
    if (mode === 'phone') {
      phoneRequestRailOpen = !collapsed
      if (!collapsed) phoneHostRailOpen = false
    } else {
      requestPreference = collapsed
    }
  }

  function closeDrawers() {
    phoneHostRailOpen = false
    phoneRequestRailOpen = false
  }

  function profile(hostId: string): HostProfile {
    return previewFixtures.hostProfiles.find((candidate) => candidate.id === hostId) ?? previewFixtures.hostProfiles[0]
  }
</script>

<main
  class="preview-root flex h-full w-full flex-col overflow-hidden rounded-[16px] border bg-background text-foreground"
  class:navigation-resizing={resizing}
  style:--workbench-sidebar-width={`${hostDisplayWidth}px`}
  style:--workbench-sidebar-collapsed-width={`${COLLAPSED_RAIL_WIDTH}px`}
  data-preview-state
  data-shell-mode={mode}
  data-host-collapsed={hostCollapsed}
  data-request-collapsed={requestCollapsed}
  data-host-display-width={hostDisplayWidth}
>
  <AppTitlebar
    sidebarCollapsed={hostCollapsed}
    onToggleSidebar={mode === 'phone' ? () => setHostRailCollapsed(!hostCollapsed) : undefined}
    pendingCount={2}
  >
    {#snippet workspaceTabs()}
      <div class="flex h-full min-w-0 items-stretch" role="tablist" aria-label="Preview workspace tabs">
        <button class="min-w-0 max-w-72 truncate rounded-t-lg bg-background px-4 text-xs" role="tab" aria-selected="true" data-workspace-tab-item>Review shell breakpoints</button>
      </div>
    {/snippet}
  </AppTitlebar>

  <WorkbenchShell
    {hostCollapsed}
    {requestCollapsed}
    onHostCollapsedChange={setHostRailCollapsed}
    onRequestCollapsedChange={setRequestRailCollapsed}
    {mode}
    onModeChange={(next) => (mode = next)}
    bind:hostDisplayWidth
    bind:resizing
  >
    {#snippet hostRail()}
      <HostSessionRail
        collapsed={hostCollapsed}
        onCollapsedChange={setHostRailCollapsed}
        {sessions}
        {activeHostId}
        {activeHostSessionId}
        inboxActive={activeHostId === null}
        resolveHostProfile={profile}
        onSelect={(hostId, sessionId) => {
          closeDrawers()
          activeHostId = hostId
          activeHostSessionId = sessionId
        }}
        onNewSession={() => {}}
      />
    {/snippet}

    {#snippet requestPane()}
      <RequestListPane
        collapsed={requestCollapsed}
        onCollapsedChange={setRequestRailCollapsed}
        requests={previewFixtures.requests}
        {activeRequestId}
        scopeLabel="All preview requests"
        resolveHostProfile={profile}
        formatTime={(value) => value?.slice(11, 16) ?? ''}
        onOpenRequest={(requestId) => {
          closeDrawers()
          activeRequestId = requestId
        }}
      />
    {/snippet}

    {#snippet workspacePane()}
      <section class="min-h-0 min-w-0 flex-1 overflow-auto p-5" aria-label="Preview workspace">
        <p class="m-0 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">RambleDesk development preview</p>
        <h1 class="mt-3 text-lg font-semibold">{activeRequest.title}</h1>
        <p class="mt-3 max-w-xl text-sm leading-6 text-muted-foreground">
          Resize the window or use device emulation. Below 768px both rails become drawers with a backdrop; Escape or the backdrop closes them, and the floating buttons reopen them.
        </p>
        <dl class="mt-5 grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-2 rounded-lg border bg-card p-4 text-xs tabular-nums" aria-label="Shell measurements">
          <dt>Mode</dt><dd data-shell-mode-readout>{mode}</dd>
          <dt>Host collapsed / display width</dt><dd data-host-readout>{String(hostCollapsed)} / {hostDisplayWidth}</dd>
          <dt>Request collapsed</dt><dd data-request-readout>{String(requestCollapsed)}</dd>
          <dt>Rail dragging</dt><dd data-resizing-readout>{String(resizing)}</dd>
        </dl>
        <div class="mt-5 min-h-40 rounded-xl border bg-muted/20 p-5 text-sm leading-7 text-muted-foreground">
          Workspace content stays usable while the navigation panes change presentation. 窄屏时侧栏与请求列变成抽屉，正文占满整屏。
        </div>
      </section>
    {/snippet}
  </WorkbenchShell>
</main>
