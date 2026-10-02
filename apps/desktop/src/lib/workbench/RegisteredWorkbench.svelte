<script lang="ts">
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import type { HostProfile } from '$lib/domain/hostProfile'
  import type { WorkbenchState } from '$lib/generated/feedback'
  import { resolveWorkbenchDefinition } from './definitions/registry'
  import type { WorkbenchController, WorkbenchViewContext, WorkbenchDefinition } from './definitions/contracts'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import RequestContextPanel from './RequestContextPanel.svelte'
  import WorkspaceHeader from './WorkspaceHeader.svelte'
  import WorkbenchGuide from './onboarding/WorkbenchGuide.svelte'

  export let workspace: FeedbackWorkspaceView
  export let transport: ApplicationTransport
  export let capabilities: Pick<WorkbenchCapabilities, 'serverPaths'>
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let readOnly = false
  export let locked = false
  export let cooking = false
  export let activeActionId: string | null = null
  export let state: WorkbenchState | null = null
  export let controller: WorkbenchController | undefined = undefined
  export let onSelectAction: (id: string, index: number, title: string) => void = () => {}
  export let onStateChange: (state: WorkbenchState) => void = () => {}
  export let onOpenReview: (() => void) | undefined = undefined
  export let onQuote: ((text: string) => void) | undefined = undefined

  let root: HTMLDivElement
  let View: Awaited<ReturnType<WorkbenchDefinition['loadView']>>['default'] | undefined
  let viewError = ''
  let loadingDefinition: WorkbenchDefinition | null | undefined
  $: guideScope = root?.closest<HTMLElement>('[data-workbench-scope]') ?? root
  $: definition = resolveWorkbenchDefinition(workspace.workbench)
  $: kind = definition?.type ?? 'unsupported'
  $: immutable = readOnly || !definition || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  $: closed = immutable || locked
  $: if (definition !== loadingDefinition) loadView(definition)
  $: context = {
    workspace, state, disabled: closed, readOnly: immutable, activeActionId,
    host: { requestId: workspace.request.request_id, controller,
      updateState: (next) => { if (!closed) onStateChange(next) },
      quote: (text) => { if (!closed) onQuote?.(text) },
      openExpanded: definition?.layout.expanded && !immutable ? onOpenReview : undefined,
      selectAction: (id, index, title) => { if (!closed) onSelectAction(id, index, title) },
    },
  } satisfies WorkbenchViewContext
  function loadView(next: WorkbenchDefinition | null) {
    loadingDefinition = next; View = undefined; viewError = ''
    if (next) void next.loadView().then((module) => { if (loadingDefinition === next) View = module.default })
      .catch(() => { if (loadingDefinition === next) viewError = tr('This workbench is unavailable. Your draft and materials are preserved in read-only mode. Open this request in a compatible client to continue.') })
  }
  const tr = (source: string) => t($locale, source)
</script>

<div bind:this={root} class="flex h-full min-h-0 min-w-0 flex-col" data-workbench={kind}>
  <WorkspaceHeader {workspace} {resolveHostProfile} {cooking}>
    {#snippet actions()}<WorkbenchGuide {definition} requestId={workspace.request.request_id} disabled={closed || cooking} ready={!!View} scope={guideScope} />{/snippet}
  </WorkspaceHeader>
  <RequestContextPanel {workspace} {transport} {capabilities} />
  <div class="min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain" class:p-5={definition?.layout.padded ?? true} data-workbench-content>
    {#if View}
      {#key workspace.request.request_id}<svelte:component this={View} {context} />{/key}
    {:else if !definition || viewError}
      <p class="m-0 text-sm leading-6 text-muted-foreground" role="status">{viewError || tr('This workbench is unavailable. Your draft and materials are preserved in read-only mode. Open this request in a compatible client to continue.')}</p>
    {/if}
  </div>
</div>
