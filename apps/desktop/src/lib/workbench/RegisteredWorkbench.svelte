<script lang="ts">
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import type { HostProfile } from '$lib/domain/hostProfile'
  import QuestionnaireWorkbench from './QuestionnaireWorkbench.svelte'
  import SingleChoiceWorkbench from './SingleChoiceWorkbench.svelte'
  import type { DocumentReviewData, SingleChoiceData, QuestionsData, WebReviewData, TerminalData, WorkbenchState } from '$lib/generated/feedback'
  import { resolveWorkbenchPolicy } from '../workbenchPolicy'
  import DocumentReviewWorkbench from './document-review/DocumentReviewWorkbench.svelte'
  import WebReviewWorkbench from './web-review/WebReviewWorkbench.svelte'
  import TerminalWorkbench from './terminal/TerminalWorkbench.svelte'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import RambleWorkbench from './RambleWorkbench.svelte'
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
  export let onSelectAction: (id: string, index: number, title: string) => void = () => {}
  export let onStateChange: (state: WorkbenchState) => void = () => {}
  export let onOpenReview: (() => void) | undefined = undefined
  export let onQuote: ((text: string) => void) | undefined = undefined
  export let onTerminalBusyChange: (requestId: string, busy: boolean) => void = () => {}

  let terminalWorkbench: TerminalWorkbench | undefined
  export async function prepareSubmission(requestId: string): Promise<boolean> {
    if (workspace.request.request_id !== requestId || immutable || locked) return false
    await terminalWorkbench?.prepareSubmission()
    return workspace.request.request_id === requestId
  }

  let root: HTMLDivElement
  $: guideScope = root?.closest<HTMLElement>('[data-workbench-scope]') ?? root

  $: spec = workspace.workbench
  $: kind = resolveWorkbenchPolicy(spec)?.type ?? 'unsupported'
  $: choice = kind === 'single_choice' && spec ? spec.data as SingleChoiceData : null
  $: questions = kind === 'questions' && spec ? spec.data as QuestionsData : null
  $: review = kind === 'document_review' && spec ? spec.data as DocumentReviewData : null
  $: webReview = kind === 'web_review' && spec ? spec.data as WebReviewData : null
  $: terminal = kind === 'terminal' && spec ? spec.data as TerminalData : null
  $: selectedOptionId = state?.type === 'single_choice' ? state.selected_option_id : null
  $: immutable = readOnly || kind === 'unsupported' || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  $: closed = immutable || locked
  const tr = (source: string) => t($locale, source)
  function select(id: string | null) {
    if (closed) return
    onStateChange({ type: 'single_choice', selected_option_id: id })
  }
</script>

<div bind:this={root} class="flex h-full min-h-0 min-w-0 flex-col" data-workbench={kind}>
  <WorkspaceHeader {workspace} {resolveHostProfile} {cooking}>
    {#snippet actions()}
      <WorkbenchGuide {kind} requestId={workspace.request.request_id} disabled={closed || cooking} scope={guideScope} />
    {/snippet}
  </WorkspaceHeader>
  <RequestContextPanel {workspace} {transport} {capabilities} />
  <div class="min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain" class:p-5={!webReview && !terminal} data-workbench-content>
    {#if kind === 'ramble'}
      <RambleWorkbench {workspace} readOnly={closed} {activeActionId} {onSelectAction} />
    {:else if questions}
      {#key workspace.request.request_id}
        <QuestionnaireWorkbench data={questions} answers={state?.type === 'questions' ? state.answers : []} disabled={closed}
          onChange={(answers) => onStateChange({ type: 'questions', answers })} />
      {/key}
    {:else if choice}
      {#key workspace.request.request_id}<SingleChoiceWorkbench data={choice} {selectedOptionId} disabled={closed} onChange={select} />{/key}
    {:else if review}
      {#key workspace.request.request_id}
        <DocumentReviewWorkbench data={review} state={state?.type === 'document_review' ? state : null} disabled={closed} onChange={onStateChange} />
      {/key}
    {:else if webReview}
      {#key workspace.request.request_id}
        <WebReviewWorkbench data={webReview} state={state?.type === 'web_review' ? state : null}
          disabled={closed} readOnly={immutable} onChange={onStateChange} {onOpenReview} />
      {/key}
    {:else if terminal}
      {#key workspace.request.request_id}
        <TerminalWorkbench bind:this={terminalWorkbench} requestId={workspace.request.request_id}
          data={terminal} state={state?.type === 'terminal' ? state : null} {transport}
          disabled={closed} readOnly={immutable} onChange={onStateChange} {onOpenReview} {onQuote}
          onBusyChange={(busy) => onTerminalBusyChange(workspace.request.request_id, busy)} />
      {/key}
    {:else}
      <p class="m-0 text-sm leading-6 text-muted-foreground" role="status">{tr('This workbench is unavailable. Your draft and materials are preserved in read-only mode. Open this request in a compatible client to continue.')}</p>
    {/if}
  </div>
</div>
