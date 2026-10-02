<script lang="ts">
  import { canSubmitWorkbench, readWorkbenchState } from '../lib/workbenchState'
  import SessionWorkbench from '$lib/workbench/SessionWorkbench.svelte'
  import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import { decodeFeedbackDraftDocument, snapshotFeedbackDraftMarkdown, type FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import { writable } from 'svelte/store'
  import { tick } from 'svelte'
  import RambleSessionController from '$lib/workbench/RambleSessionController.svelte'
  import { createRambleSession } from '$lib/workbench/rambleSession'
  import { createRequestInputTargets, resolveRequestInputTarget } from '$lib/input/requestInputTargets'
  import { provideVoiceInput, type VoiceInputState } from '$lib/speech/voiceInputContext'
  import { applySpeechWriteback, type SpeechWriteInput } from '$lib/speech/speechWriteback'
  import { applyInputTextWriteback } from '$lib/inputTextWriteback'
  import { attachmentMarkdown } from '$lib/attachmentMarkdown'
  import rambelleIdle from '../assets/rambelle-states/idle.webp'
  import type { SpeechTarget } from '$lib/speech/speechTargets'
  import type { RambleSessionControllerHandle } from '$lib/speech/rambleSessionControllerHandle'
  import { speechTargetLabel } from '$lib/speech/speechTargetLabel'
  import RequestSpeechTools from '$lib/speech/RequestSpeechTools.svelte'
  import { provideRequestSpeechTools } from '$lib/speech/requestSpeechToolsContext'
  import { createRequestSpeechTidy } from '$lib/speech/requestSpeechTidy'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { createSimulatedSpeech } from './simulatedSpeech'
  import { previewHostProfile } from './agentPreviewFixtures'
  import { workbenchExamples, workbenchPreviewAttachments, workbenchPreviewLabels, workbenchPreviewWorkspace } from './workbenchPreviewFixtures'
  import { TerminalPreviewRuntime } from '$lib/preview/terminalPreviewFixture'
  import { findWorkbenchExample, registeredWorkbenchExamples } from '$lib/workbench/definitions/examples'

  const initialType = typeof window !== 'undefined' ? new URLSearchParams(window.location.search).get('type') : null
  let index = initialType === 'single_question' ? 2 : Math.max(0, registeredWorkbenchExamples.indexOf(findWorkbenchExample(initialType)!))
  let workspace = workbenchPreviewWorkspace(index)
  let view: SessionWorkbench
  let activeActionId: string | null = null
  let snapshot: FeedbackDraftSnapshot = { documentJson: '', bodyMarkdown: '' }
  let submitted = false
  let editorEpoch = 0
  let error = ''
  let transcript = '这是一段模拟转写，用于检查语音写入位置。'
  const voicePreview = typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('voice') === '1'
  const simulated = createSimulatedSpeech()
  const rambleSession = createRambleSession()
  let ramble: RambleSessionControllerHandle
  const targets = createRequestInputTargets()
  let terminal = new TerminalPreviewRuntime()
  let revealTarget: SpeechTarget | null = null
  let revealSequence = 0
  const voiceState = writable<VoiceInputState>({ requestId: '', documentTarget: null, nextTarget: null, recording: false, disabled: true })
  $: documentTarget = { requestId: workspace.request.request_id, requestTitle: workspace.request.title, destination: {
    kind: 'document' as const, action: activeActionId ? { actionId: activeActionId, actionIndex: workspace.actions.findIndex((item) => item.id === activeActionId), title: workspace.actions.find((item) => item.id === activeActionId)?.instruction ?? '' } : null,
  } }
  $: nextTarget = resolveRequestInputTarget(documentTarget, $targets.get(documentTarget.requestId))
  $: voiceState.set({ requestId: workspace.request.request_id, documentTarget, nextTarget, recording: $rambleSession.voiceActive, disabled: submitted || !voicePreview, revealTarget, revealSequence, draftSnapshot: snapshot })
  provideVoiceInput({ state: voiceState,
    selectTarget: (target) => targets.select(target),
    start: async (target) => { if (voicePreview) { targets.select(target); await tick(); await ramble.startInput() } },
    stop: () => { if (voicePreview) return ramble.pauseInput() },
  })
  const speechTidy = createRequestSpeechTidy({
    getWorkspace: () => workspace, isLocked: () => submitted,
    getConfig: () => ({ provider: 'openai', apiKey: 'preview-only', baseUrl: '', model: 'simulated-tidy', reasoningEffort: 'none', locale: $locale }),
    commit(requestId, edit) {
      if (workspace.request.request_id !== requestId || submitted) throw new Error('The request changed while tidying. No text was replaced.')
      change(edit(workspace)); editorEpoch += 1
    },
    tidy: async (segments) => segments.map((segment) => segment.text.replace(/^(?:嗯[，,\s]*|呃[，,\s]*|um[,\s]+)+/gi, '').replace(/就是说[，,\s]*/g, '').trim()),
  })
  provideRequestSpeechTools(speechTidy)
  $: { workspace; submitted; speechTidy.refresh(workspace) }
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .handle('openTerminalSession', (input) => terminal.call('openTerminalSession', input))
    .handle('readTerminalSession', (input) => terminal.call('readTerminalSession', input))
    .handle('writeTerminalSession', (input) => terminal.call('writeTerminalSession', input))
    .handle('resizeTerminalSession', (input) => terminal.call('resizeTerminalSession', input))
    .handle('stopTerminalSession', (input) => terminal.call('stopTerminalSession', input))
    .handle('readRequestAttachment', ({ request_id, attachment_id }) => {
      const attachment = request_id === workspace.request.request_id && workspace.request_attachments.some((item) => item.attachment_id === attachment_id)
        ? workbenchPreviewAttachments.find((item) => item.attachment_id === attachment_id) : undefined
      if (!attachment) throw new Error('预览附件不存在。')
      return attachment.contents.slice(0)
    })
    .handle('readFeedbackAttachment', ({ attachment_id }) => {
      const contents = attachmentContents.get(attachment_id)
      if (!contents) throw new Error('预览附件不存在。')
      return contents
    })
  const attachmentContents = new Map<string, ArrayBuffer>()
  const capabilities = voicePreview ? simulated.capabilities : createUnavailableWorkbenchCapabilities()
  const empty = { type: 'doc', content: [{ type: 'paragraph' }] }
  async function choose(next: number) {
    if (voicePreview) await ramble.exitRamble()
    index = next
    terminal = new TerminalPreviewRuntime()
    workspace = workbenchPreviewWorkspace(index)
    activeActionId = null
    snapshot = { documentJson: '', bodyMarkdown: '' }
    attachmentContents.clear()
    submitted = false
    error = ''; editorEpoch += 1
  }
  function change(next: FeedbackDraftSnapshot) {
    snapshot = next
    workspace = { ...workspace, draft: { ...workspace.draft, document_json: next.documentJson, body_markdown: next.bodyMarkdown, saved_revision: workspace.draft.saved_revision + 1 } }
  }
  async function writeSpeech(input: SpeechWriteInput) {
    if (!snapshot.documentJson) change(snapshotFeedbackDraftMarkdown(''))
    change(applySpeechWriteback(workspace, input))
    editorEpoch += 1
  }
  async function writeInputText(target: SpeechTarget, text: string, id: string = crypto.randomUUID()) {
    if (!snapshot.documentJson) change(snapshotFeedbackDraftMarkdown(''))
    if (target.destination.kind === 'document') {
      view.applyDraftOperation({ kind: 'appendClipboardText', text, label: 'Clipboard', action: target.destination.action })
    } else change(applyInputTextWriteback(workspace, { target, text, id }))
    editorEpoch += 1
  }
  async function addInputFiles(files: readonly File[], target?: SpeechTarget) {
    if (!target || target.requestId !== workspace.request.request_id || submitted) return
    for (const file of files) {
      const contents = await file.arrayBuffer()
      if (target.requestId !== workspace.request.request_id || submitted) return
      const attachment = { attachment_id: crypto.randomUUID(), file_name: file.name,
        media_type: file.type || (file.name.endsWith('.md') ? 'text/markdown' : 'application/octet-stream'),
        byte_size: file.size, sha256: 'preview-only', position: workspace.attachments.length }
      attachmentContents.set(attachment.attachment_id, contents)
      workspace = { ...workspace, attachments: [...workspace.attachments, attachment] }
      if (target.destination.kind === 'document') view.applyDraftOperation({ kind: 'appendAttachment', attachment, label: file.name, action: target.destination.action })
      else await writeInputText(target, attachmentMarkdown(attachment))
    }
  }
  async function submit() {
    if (voicePreview) {
      const ready = await ramble.prepareFeedback(workspace.request.request_id)
      if (ready.kind !== 'ready') { error = ready.kind === 'failed' ? ready.message : '请先确认或丢弃待处理语音。'; return }
    }
    submitted = true
  }
  function selectAction(id: string, actionIndex: number, title: string) {
    activeActionId = id
    targets.select({ ...documentTarget, destination: { kind: 'document', action: { actionId: id, actionIndex, title } } })
    view.applyDraftOperation({ kind: 'startActionGroup', action: { actionId: id, actionIndex, title } })
  }
</script>

<main class="flex h-screen flex-col bg-background text-foreground">
  <nav class="flex shrink-0 flex-wrap items-center gap-3 border-b px-5 py-3" aria-label="工作台实验">
    {#each workbenchExamples as example, i}
      <button type="button" aria-pressed={index === i} onclick={() => choose(i)} class="rounded-md border px-4 py-2 text-sm aria-pressed:bg-primary aria-pressed:text-primary-foreground">{workbenchPreviewLabels[i]}</button>
    {/each}
    {#if workspace.workbench?.type === 'web_review'}
      <a class="rounded-md border px-3 py-2 text-sm" href="/?preview=fixtures&workspace=web_review" target="_blank" rel="noopener noreferrer">在完整应用中体验网页评审</a>
    {/if}
    {#if workspace.workbench?.type === 'terminal'}
      <a class="rounded-md border px-3 py-2 text-sm" href="/?preview=fixtures&workspace=terminal" target="_blank" rel="noopener noreferrer">在完整应用中体验终端工作台</a>
    {/if}
    <span class="ml-auto text-xs text-muted-foreground">交互预览 · 内容只保存在本页内存中</span>
  </nav>
  {#if voicePreview}
    <section class="flex shrink-0 flex-wrap items-center gap-2 border-b bg-amber-500/5 px-5 py-2 text-xs" aria-label="模拟语音控制">
      <strong>模拟语音与整理 · 不使用真实麦克风或模型</strong>
      <input aria-label="模拟转写内容" class="min-w-64 flex-1 rounded border bg-background px-2 py-1.5" bind:value={transcript} />
      <button type="button" class="rounded border px-3 py-1.5 disabled:opacity-40" disabled={!$simulated.recording || $simulated.segment !== null} onclick={() => simulated.begin(transcript)}>开始一段语音</button>
      <button type="button" class="rounded border px-3 py-1.5 disabled:opacity-40" disabled={$simulated.segment === null} onclick={() => simulated.finish(transcript)}>完成转写</button>
      <span class="w-full text-muted-foreground">点击输入框的语音按钮开启会话。开始一段后切换字段，再完成转写，可检查旧片段仍写入原位置。下一段：{speechTargetLabel(nextTarget, (text) => t($locale, text))}</span>
    </section>
    <RambleSessionController bind:this={ramble} {capabilities} {workspace} session={rambleSession} speechDraftStorage={simulated.storage}
      {nextTarget} getNextSpeechTarget={() => targets.forRequest(documentTarget)} onWriteSpeech={writeSpeech}
      onInputText={writeInputText}
      embeddedConsole
      interactionLocked={submitted} onPageError={(message) => error = message}
      onOpenSpeechTarget={async (_requestId, _segmentId, target) => { if (target) { revealTarget = target; revealSequence += 1 } }} />
  {/if}
  {#if error}<p role="alert" class="m-0 border-b px-5 py-2 text-xs text-destructive">{error}</p>{/if}
  {#key index}
    <SessionWorkbench bind:this={view} {workspace} {transport} {capabilities} resolveHostProfile={previewHostProfile} formatTime={() => ''}
      draftDocumentJson={snapshot.documentJson || undefined} editorDocument={decodeFeedbackDraftDocument(snapshot.documentJson) ?? empty} {editorEpoch} draftBody={snapshot.bodyMarkdown} {activeActionId} onSelectAction={selectAction}
      rambelleStatusPortrait={rambelleIdle} onFiles={addInputFiles} onImportClipboard={(target) => ramble.importClipboardNow(target)}
      onPasteError={(cause) => error = String(cause)}
      rambleActive={$rambleSession.active} rambleEngaged={$rambleSession.engaged}
      inputActions={requestInputActions}
      onDraftChange={change} canSubmit={canSubmitWorkbench(workspace.workbench, readWorkbenchState(snapshot.documentJson), snapshot.bodyMarkdown) && !submitted} readOnly={submitted}
      onSubmit={() => void submit()} />
  {/key}
  {#if submitted}
    <aside role="status" class="max-h-48 overflow-auto border-t bg-muted p-4 text-sm">
      <p>预览提交完成。正式请求会把结构化结果写入反馈包；下面是独立的工作台答案与补充正文。</p>
      <pre class="whitespace-pre-wrap text-xs">{JSON.stringify({ answers: readWorkbenchState(snapshot.documentJson), notes: snapshot.bodyMarkdown }, null, 2)}</pre>
    </aside>
  {/if}
</main>

{#snippet requestInputActions()}<RequestSpeechTools />{/snippet}
