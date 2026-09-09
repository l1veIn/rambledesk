<script lang="ts">
  import HostSessionRail from '$lib/components/navigation/HostSessionRail.svelte'
  import RequestListPane from '$lib/components/navigation/RequestListPane.svelte'
  import AppTitlebar from '$lib/shell/AppTitlebar.svelte'
  import type { HostSessionSummary } from '$lib/feedback'
  import WorkbenchShell from '$lib/workbench/WorkbenchShell.svelte'

  let hostCollapsed = true
  let requestCollapsed = true
  let refreshing = false
  const sessions: HostSessionSummary[] = [{
    session_id: 'existing', host_id: 'codex', host_session_id: 'existing', title: 'Existing session',
    management: { kind: 'external' }, source_hint: null, request_count: 1, pending_count: 1,
    updated_at: '2026-09-09T00:00:00Z', pinned_at: null, archived_at: null, host_pinned_at: null,
  }]

  function setHostCollapsed(value: boolean) {
    hostCollapsed = value
    if (!value) requestCollapsed = true
  }

  function setRequestCollapsed(value: boolean) {
    requestCollapsed = value
    if (!value) hostCollapsed = true
  }

  const resolveHostProfile = (id: string) => ({
    id, label: id, icon_svg: '', default_adapter: '', continuation_mode: 'none',
  })
</script>

<AppTitlebar sidebarCollapsed={hostCollapsed} onToggleSidebar={() => setHostCollapsed(!hostCollapsed)}>
  {#snippet workspaceTabs()}
    <button onclick={() => { setHostCollapsed(true); setRequestCollapsed(true) }}>Other titlebar action</button>
    <button onclick={() => refreshing = !refreshing}>Toggle session refresh</button>
  {/snippet}
</AppTitlebar>
<WorkbenchShell
  mode="phone"
  {hostCollapsed}
  {requestCollapsed}
  onHostCollapsedChange={setHostCollapsed}
  onRequestCollapsedChange={setRequestCollapsed}
>
  {#snippet hostRail()}
    <HostSessionRail collapsed={false} {sessions} {refreshing} {resolveHostProfile} onCollapsedChange={setHostCollapsed} />
  {/snippet}
  {#snippet requestPane()}
    <RequestListPane collapsed={false} {resolveHostProfile} formatTime={() => ''} onCollapsedChange={setRequestCollapsed} />
  {/snippet}
  {#snippet workspacePane()}
    <button>Workspace action</button>
  {/snippet}
</WorkbenchShell>
