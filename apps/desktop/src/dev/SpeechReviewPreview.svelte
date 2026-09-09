<script lang="ts">
  import RecordingOverlay from '$lib/speech/RecordingOverlay.svelte'
  import type { RambleConsoleCommand } from '$lib/rambleConsole'
  import type { SpeechOverlayState } from '$lib/speech/speechOverlay'
  import type { DraftOperation } from '$lib/draftOperations'
  import { handleSpeechDraftCommand } from '$lib/workbench/speechDraftCommands'
  import { createSpeechDraftQueue, groupSpeechDrafts, type SpeechTarget } from '$lib/workbench/speechDraftQueue'
  import type { VoicePhase } from '$lib/workbench/types'

  const mainTarget: SpeechTarget = {
    requestId: 'preview-feedback', requestTitle: 'Login review',
    action: { actionId: 'login', actionIndex: 0, title: 'Check login behavior' },
  }
  const otherTarget: SpeechTarget = { requestId: 'preview-other-feedback', requestTitle: 'Navigation review', action: null }
  let nextId = 1
  let selectedGroupId: string | null = null
  let phase: VoicePhase = 'listening'
  let autoTidy = false
  let failNextTidy = false
  let locked = false
  let partial = ''
  let notice = ''
  let tidyCalls = 0
  let writes: Array<{ requestId: string; operation: DraftOperation }> = []
  const queue = createSpeechDraftQueue({
    write: async (requestId, operation) => { writes = [...writes, { requestId, operation }] },
    tidy: async (text) => {
      tidyCalls += 1
      const fail = failNextTidy
      failNextTidy = false
      await new Promise<void>((resolve) => window.setTimeout(resolve, 600))
      if (fail) throw new Error('Preview Tidy failed. The original speech is still available.')
      return text.replace(/\b(?:um|uh)\b[,，]?\s*/gi, '').replace(/嗯[，,]?\s*/g, '')
        .replace(/\b(\w+)(?:\s+\1)+\b/gi, '$1').replace(/[ \t]+/g, ' ').trim()
    },
  })

  $: groups = groupSpeechDrafts($queue.drafts)
  $: state = {
    enabled: true, opacity: 100, selectedGroupId,
    shortcuts: { speechAccept: 'Ctrl+Shift+Enter', speechDiscard: 'Ctrl+Shift+Backspace' },
    phase, level: phase === 'listening' ? 0.6 : 0, partial, error: '', target: mainTarget,
    groups, receipt: $queue.receipt, edit: $queue.edit,
  } satisfies SpeechOverlayState

  function addSpeech(long = false, other = false) {
    const id = `preview-${nextId++}`
    const text = long
      ? 'Um, the login button works works after I enter my email. The error message is visible, but the message should explain which field needs attention.\n\n嗯，登录表单能正常提交。输入错误密码时，页面会显示错误信息，但错误提示应当更明确。\n\nI also checked keyboard navigation and a narrow window. The focused field remains visible, and the submit button fits on screen.'
      : `Um, speech segment ${nextId - 1}: the button works works and the feedback stays in the right request.`
    void queue.enqueue(id, text, other ? otherTarget : mainTarget, true, autoTidy)
    if (!selectedGroupId) selectedGroupId = id
  }

  async function command(command: RambleConsoleCommand) {
    if (await handleSpeechDraftCommand(queue, command, locked)) return
    if (command.type === 'select-speech-group' && !$queue.edit) selectedGroupId = command.id
    if (command.type === 'toggle-recording') phase = phase === 'idle' ? 'listening' : 'idle'
    if (command.type === 'retry-recording') phase = 'listening'
    if (command.type === 'open-speech-target') notice = `Opened preview target: ${command.requestId}`
    if (command.type === 'exit') phase = 'idle'
  }

  function simulateConfirmShortcut() {
    const group = groups.find((item) => item.ids.includes(selectedGroupId ?? '')) ?? groups[0]
    if (group) void handleSpeechDraftCommand(queue, { type: 'accept-speech', ids: [...group.ids] }, locked)
  }

  addSpeech()
</script>

<main>
  <header class="intro">
    <p class="eyebrow">RambleDesk development preview</p>
    <h1>Speech review</h1>
    <p>Exercise the real speech queue and overlay. Tidy finishes in 600 ms; accepted speech appears in the write log below.</p>
  </header>

  <section class="controls" aria-label="Preview controls">
    <button onclick={() => addSpeech()}>Add speech segment</button>
    <button onclick={() => addSpeech(true)}>Add long speech</button>
    <button onclick={() => addSpeech(false, true)}>Add another target</button>
    <button onclick={() => phase = phase === 'idle' ? 'listening' : 'idle'}>{phase === 'idle' ? 'Start recording' : 'Pause recording'}</button>
    <button onclick={() => partial = partial ? '' : 'Incoming speech continues while the review remains open…'}>Toggle partial transcript</button>
    <button onclick={simulateConfirmShortcut}>Simulate confirm shortcut</button>
    <label><input type="checkbox" bind:checked={autoTidy} />Auto Tidy new speech</label>
    <label><input type="checkbox" bind:checked={failNextTidy} />Fail next Tidy</label>
    <label><input type="checkbox" bind:checked={locked} />Lock review writes</label>
  </section>

  <section class="debug-grid">
    <article aria-label="Pending preview drafts">
      <h2>Pending speech <span>{groups.length} groups</span></h2>
      {#each groups as group (group.ids[0])}
        <div class="draft-row">
          <strong>{group.requestTitle}</strong>
          <small>{group.ids.join(', ')} · {group.cleanupState} {group.busy ? '· busy' : ''}</small>
          <p>{group.text}</p>
        </div>
      {:else}<p class="muted">No pending speech.</p>{/each}
    </article>
    <article aria-label="Preview write log">
      <h2>Write log <span>{writes.length} writes · {tidyCalls} Tidy calls</span></h2>
      {#each writes as entry, index (index)}
        <div class="draft-row">
          <strong>{entry.requestId}</strong>
          {#if entry.operation.kind === 'appendSpeech'}
            <small>{entry.operation.segmentId} · {entry.operation.cleanupState ?? 'pending'}</small>
            <p>{entry.operation.text}</p>
          {/if}
        </div>
      {:else}<p class="muted">Confirm speech to see the accepted text here.</p>{/each}
      {#if notice}<p role="status">{notice}</p>{/if}
    </article>
  </section>

  <RecordingOverlay {state} draggable={false} onCommand={(next) => void command(next)} />
</main>

<style>
  main { min-height: 100vh; box-sizing: border-box; padding: 32px 32px 360px; background: var(--background); color: var(--foreground); }
  .intro, .controls, .debug-grid { max-width: 1120px; margin-inline: auto; }
  .intro { margin-bottom: 24px; }
  .eyebrow { color: var(--muted-foreground); font-size: 11px; letter-spacing: .12em; text-transform: uppercase; }
  h1 { font-size: 28px; margin: 8px 0; }
  .intro > p:last-child { max-width: 700px; color: var(--muted-foreground); font-size: 13px; line-height: 1.6; }
  .controls { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; padding: 16px; border: 1px solid var(--border); border-radius: 12px; }
  .controls button { border: 1px solid var(--border); border-radius: 6px; padding: 7px 10px; background: var(--card); font: inherit; font-size: 12px; cursor: pointer; }
  .controls label { display: flex; align-items: center; gap: 6px; font-size: 12px; }
  .controls button:hover { background: var(--muted); }
  .controls button:focus-visible, .controls input:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
  .debug-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; margin-top: 20px; }
  article { min-width: 0; padding: 16px; border: 1px solid var(--border); border-radius: 12px; }
  h2 { display: flex; align-items: center; justify-content: space-between; margin: 0 0 12px; font-size: 14px; }
  h2 span, small { font-size: 10px; color: var(--muted-foreground); font-weight: 400; }
  .draft-row + .draft-row { border-top: 1px solid var(--border); padding-top: 12px; }
  .draft-row strong, .draft-row small { display: block; }
  .draft-row strong { font-size: 12px; margin-bottom: 4px; }
  .draft-row p, .muted { font-size: 12px; line-height: 1.6; white-space: pre-wrap; overflow-wrap: anywhere; }
  .muted { color: var(--muted-foreground); }
  @media (max-width: 640px) { main { padding: 16px 16px 360px; } .debug-grid { grid-template-columns: 1fr; } }
</style>
