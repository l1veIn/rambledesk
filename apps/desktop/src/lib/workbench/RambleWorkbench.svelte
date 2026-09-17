<!--
  The Ramble workbench: the task a person is being asked to review.

  It renders only the request identity and its brief (what happened, the actions
  to experience, and the materials the Agent attached). The document, submission
  and host protocol belong to the workbench container around it; this component
  reports the action a person picked and nothing else.
-->
<script lang="ts">
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import type { HostProfile } from '../domain/hostProfile'
  import TaskBriefPanel from './TaskBriefPanel.svelte'
  import WorkspaceHeader from './WorkspaceHeader.svelte'

  export let workspace: FeedbackWorkspaceView
  export let transport: ApplicationTransport
  export let capabilities: Pick<WorkbenchCapabilities, 'serverPaths'>
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let readOnly = false
  export let cooking = false
  export let activeActionId: string | null = null
  export let open = true
  /** Opens the same brief as a full workspace view. */
  export let onOpenFullView: () => void = () => {}
  export let onSelectAction: (actionId: string, actionIndex: number, title: string) => void = () => {}
</script>

<div class="flex h-full min-h-0 min-w-0 flex-col" data-workbench="ramble">
  <WorkspaceHeader {workspace} {resolveHostProfile} {cooking} />
  <TaskBriefPanel
    {transport}
    {capabilities}
    bind:open
    {workspace}
    {activeActionId}
    onSelectAction={(id, index, title) => { if (!readOnly) onSelectAction(id, index, title) }}
    onOpenPreview={onOpenFullView}
  />
</div>
