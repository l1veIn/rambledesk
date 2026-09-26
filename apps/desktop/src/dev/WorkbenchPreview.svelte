<script lang="ts">
  import { canSubmitWorkbench, readWorkbenchState } from '../lib/workbenchState'
  import SessionWorkbench from '$lib/workbench/SessionWorkbench.svelte'
  import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import type { FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import { previewHostProfile } from './agentPreviewFixtures'
  import { workbenchExamples, workbenchPreviewLabels, workbenchPreviewWorkspace } from './workbenchPreviewFixtures'

  let index = typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('type') === 'document_review' ? 3 : 0
  let workspace = workbenchPreviewWorkspace(index)
  let view: SessionWorkbench
  let activeActionId: string | null = null
  let snapshot: FeedbackDraftSnapshot = { documentJson: '', bodyMarkdown: '' }
  let submitted = false
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
  const capabilities = createUnavailableWorkbenchCapabilities()
  const empty = { type: 'doc', content: [{ type: 'paragraph' }] }
  function choose(next: number) {
    index = next
    workspace = workbenchPreviewWorkspace(index)
    activeActionId = null
    snapshot = { documentJson: '', bodyMarkdown: '' }
    submitted = false
  }
  function change(next: FeedbackDraftSnapshot) { snapshot = next }
  function selectAction(id: string, actionIndex: number, title: string) {
    activeActionId = id
    view.applyDraftOperation({ kind: 'startActionGroup', action: { actionId: id, actionIndex, title } })
  }
</script>

<main class="flex h-screen flex-col bg-background text-foreground">
  <nav class="flex shrink-0 flex-wrap items-center gap-3 border-b px-5 py-3" aria-label="工作台实验">
    {#each workbenchExamples as example, i}
      <button type="button" aria-pressed={index === i} onclick={() => choose(i)} class="rounded-md border px-4 py-2 text-sm aria-pressed:bg-primary aria-pressed:text-primary-foreground">{workbenchPreviewLabels[i]}</button>
    {/each}
    <span class="ml-auto text-xs text-muted-foreground">交互预览 · 内容只保存在本页内存中</span>
  </nav>
  {#key index}
    <SessionWorkbench bind:this={view} {workspace} {transport} {capabilities} resolveHostProfile={previewHostProfile} formatTime={() => ''}
      draftDocumentJson={snapshot.documentJson || undefined} editorDocument={empty} draftBody={snapshot.bodyMarkdown} {activeActionId} onSelectAction={selectAction}
      onDraftChange={change} canSubmit={canSubmitWorkbench(workspace.workbench, readWorkbenchState(snapshot.documentJson), snapshot.bodyMarkdown) && !submitted} readOnly={submitted}
      onSubmit={() => submitted = true} />
  {/key}
  {#if submitted}
    <aside role="status" class="max-h-48 overflow-auto border-t bg-muted p-4 text-sm">
      <p>预览提交完成。正式请求会把结构化结果写入反馈包；下面是独立的工作台答案与补充正文。</p>
      <pre class="whitespace-pre-wrap text-xs">{JSON.stringify({ answers: readWorkbenchState(snapshot.documentJson), notes: snapshot.bodyMarkdown }, null, 2)}</pre>
    </aside>
  {/if}
</main>
