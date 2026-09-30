<script lang="ts">
  import { onMount } from 'svelte'
  import { ClipboardCopy, Maximize2, Play, Square, Terminal as TerminalIcon } from '@lucide/svelte'
  import '@xterm/xterm/css/xterm.css'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { TerminalData, TerminalTrialSession } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { emptyTerminalState, type TerminalState } from '../terminalModel'
  import { canFillTerminalCommand, createTerminalController, terminalTextTail } from './terminalController'
  import { createXtermAdapter, type TerminalAdapter } from './xtermAdapter'
  import { terminalText } from './terminalI18n'
  import { isMissingTerminalSession } from './terminalErrors'

  export let requestId: string
  export let data: TerminalData
  export let transport: ApplicationTransport
  export let state: TerminalState | null = null
  export let disabled = false
  export let readOnly = false
  export let onChange: (state: TerminalState) => void
  export let onOpenReview: (() => void) | undefined = undefined
  export let onQuote: ((text: string) => void) | undefined = undefined
  export let onBusyChange: (busy: boolean) => void = () => {}

  let root: HTMLDivElement
  let adapter: TerminalAdapter | undefined
  let controller: ReturnType<typeof createTerminalController> | undefined
  let initialization: Promise<void> | undefined
  let ready = false
  let busy = false
  let mounted = false
  let selection = ''
  let error = ''
  let resizeTimer: ReturnType<typeof setTimeout> | undefined
  const tr = (source: string) => terminalText($locale, source)
  $: current = state ?? emptyTerminalState()
  $: latest = current.sessions.at(-1)
  $: interactive = ready && !disabled && !readOnly && !busy && !error && latest?.status === 'running'
  $: adapter?.setInteractive(interactive)
  $: controller?.setLocked(disabled || readOnly)
  $: if (readOnly && controller) detach()

  function remember(session: TerminalTrialSession) {
    if (!mounted || readOnly) return
    const existing = current.sessions.findIndex((item) => item.id === session.id)
    const sessions = [...current.sessions]
    if (existing === -1) sessions.push(session)
    else sessions[existing] = session
    state = { type: 'terminal', sessions }
    current = state
    onChange(state)
  }
  function showError(cause: unknown) {
    error = isMissingTerminalSession(cause) ? tr('The terminal session was lost. Saved output is preserved; you can submit it with your feedback.')
      : cause instanceof Error ? cause.message : typeof cause === 'object' && cause && 'message' in cause
      ? String(cause.message) : tr('Terminal unavailable')
  }
  function resized(size: { cols: number; rows: number }) {
    if (resizeTimer) clearTimeout(resizeTimer)
    resizeTimer = setTimeout(() => { void controller?.resize(size.cols, size.rows) }, 80)
  }
  function detach() {
    controller?.dispose(); controller = undefined
    adapter?.dispose(); adapter = undefined
    ready = false
    if (busy) { busy = false; onBusyChange(false) }
    if (resizeTimer) clearTimeout(resizeTimer)
  }
  async function initialize() {
    const next = await createXtermAdapter(root, {
      onData: (input) => { void controller?.write(input).catch(() => undefined) },
      onResize: resized, onSelection: (text) => selection = text,
    })
    if (!mounted || readOnly) { next.dispose(); return }
    adapter = next
    adapter.fit()
    if (latest?.output) await adapter.write(latest.output)
    if (!mounted || readOnly) { next.dispose(); return }
    controller = createTerminalController({
      initial: latest, renderer: adapter,
      runtime: {
        open: (cols, rows) => transport.call('openTerminalSession', { request_id: requestId, cols, rows }),
        read: (session_id, after_sequence) => transport.call('readTerminalSession', { request_id: requestId, session_id, after_sequence }),
        write: (session_id, input) => transport.call('writeTerminalSession', { request_id: requestId, session_id, data: input }),
        resize: (session_id, cols, rows) => transport.call('resizeTerminalSession', { request_id: requestId, session_id, cols, rows }),
        stop: (session_id) => transport.call('stopTerminalSession', { request_id: requestId, session_id }),
      }, onSession: remember, onBusy: (value) => { busy = value; onBusyChange(value) }, onError: showError,
    })
    controller.setLocked(disabled || readOnly)
    ready = true
    if (latest?.status === 'running') {
      try { await controller.reconnect() } catch { /* A lost session requires a manual start. */ }
    }
  }
  async function start() {
    if (!adapter || !controller || disabled || readOnly || busy) return
    error = ''
    try { const size = adapter.size(); await controller.start(size.cols, size.rows); adapter?.focus() }
    catch { /* Controller presents the actionable connection error. */ }
  }
  async function stop() {
    error = ''
    try { await controller?.stop() } catch { /* Keep the request editable for retry. */ }
  }
  function fillCommand(command: string) {
    if (!interactive || !canFillTerminalCommand(command)) return
    adapter?.paste(command)
    adapter?.focus()
  }
  function quoteSelection() {
    if (!selection.trim() || disabled || readOnly || !onQuote) return
    const text = terminalTextTail(selection.trim(), 8000)
    const heading = latest ? `${tr('Terminal trial')} · ${latest.cwd}` : tr('Terminal trial')
    onQuote(`${heading}\n\n${text.split('\n').map((line) => `> ${line}`).join('\n')}`)
  }
  /** Submit must stop and drain the PTY before the parent saves its final draft. */
  export async function prepareSubmission(): Promise<void> {
    if (readOnly) return
    await initialization
    if (!controller && latest?.status === 'running') throw new Error(error || tr('Terminal unavailable'))
    error = ''
    await controller?.prepareSubmission()
  }
  onMount(() => {
    mounted = true
    const observer = new ResizeObserver(() => adapter?.fit())
    if (!readOnly) {
      initialization = initialize()
      void initialization.catch(showError)
      observer.observe(root)
    }
    return () => { mounted = false; observer.disconnect(); detach() }
  })
  // ANSI is rendered by xterm while live. History presents its saved screen plus a text transcript.
  function plainOutput(output: string) {
    return output.replace(/\x1b\][^\x07]*(?:\x07|\x1b\\)/g, '').replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '').replace(/\x1b[@-_]/g, '')
  }
</script>

<section class="terminal-workbench" data-terminal-workbench>
  <header class="terminal-toolbar" data-terminal-toolbar>
    <div class="terminal-heading"><TerminalIcon class="size-4" /><span>{tr('Terminal trial')}</span></div>
    <div class="terminal-actions">
      {#if !readOnly}
        {#if !latest || error}
          <Button size="sm" variant="outline" disabled={!ready || disabled || busy} onclick={() => void start()}>
            <Play class="size-4" />{tr(error ? 'Retry connection' : 'Start terminal')}
          </Button>
        {:else if latest.status === 'running'}
          <Button size="sm" variant="outline" disabled={disabled || busy} onclick={() => void stop()}>
            <Square class="size-4" />{tr('Stop terminal')}
          </Button>
        {/if}
        {#if onOpenReview}
          <Button size="sm" variant="outline" disabled={disabled} onclick={onOpenReview}>
            <Maximize2 class="size-4" />{tr('Full screen trial')}
          </Button>
        {/if}
        {#if onQuote}
          <Button size="sm" variant="ghost" disabled={!selection.trim() || disabled} onclick={quoteSelection}>
            <ClipboardCopy class="size-4" />{tr('Quote selected output')}
          </Button>
        {/if}
      {/if}
    </div>
  </header>
  <div class="terminal-context">
    <code title={data.cwd}>{data.cwd}</code>
    {#if latest}<span class="terminal-status" data-terminal-status>{tr(busy ? 'Connecting…' : latest.status === 'running' ? 'Running' : latest.status === 'stopped' ? 'Stopped' : 'Exited')}{latest.exit_code !== null ? ` · ${tr('Exit code')} ${latest.exit_code}` : ''}</span>{/if}
  </div>
  {#if !readOnly && data.commands.length}
    <div class="terminal-suggestions" data-terminal-suggestions>
      <p>{tr('Click a command to fill it, then press Enter to run.')}</p>
      {#each data.commands as command (command.id)}
        <button type="button" class="terminal-command" title={command.description ?? command.title}
          aria-label={`${tr('Fill command')}: ${command.title}`} disabled={!interactive || !canFillTerminalCommand(command.command)}
          onclick={() => fillCommand(command.command)}>
          <span>{command.title}</span><code>{command.command}</code>
        </button>
      {/each}
    </div>
  {/if}
  {#if error}<p class="terminal-error" role="alert">{error}</p>{/if}
  {#if readOnly}
    <div class="terminal-history" data-terminal-history>
      {#each current.sessions as session (session.id)}
        <article>
          <div class="terminal-history-heading"><span>{tr('Saved trial')}</span><span>{session.cwd} · {session.shell}</span></div>
          <pre aria-label={tr('Saved terminal screen')}>{session.screen || plainOutput(session.output)}</pre>
          <details><summary>{tr('Terminal transcript')}</summary><pre>{plainOutput(session.output)}</pre></details>
          {#if session.truncated}<p>{tr('Earlier output was truncated. The latest screen is preserved.')}</p>{/if}
        </article>
      {:else}<p>{tr('No trial recorded yet.')}</p>{/each}
    </div>
  {:else}
    <div class="terminal-stage">
      <div bind:this={root} class="terminal-surface" data-terminal-surface></div>
      {#if !latest && !busy}<div class="terminal-empty">{tr('Start the terminal to try the CLI.')}</div>{/if}
    </div>
    <footer><span>{tr('Trial output is saved with your feedback.')}</span>{#if latest?.truncated}<span>{tr('Earlier output was truncated. The latest screen is preserved.')}</span>{/if}</footer>
  {/if}
</section>

<style>
  .terminal-workbench { display: flex; flex-direction: column; height: 100%; min-height: 520px; min-width: 0; overflow: hidden; border: 1px solid var(--border); border-radius: 8px; background: var(--background); }
  .terminal-toolbar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  .terminal-heading { display: flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; margin-right: auto; }
  .terminal-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .terminal-context { display: flex; flex-wrap: wrap; gap: 6px 12px; padding: 6px 12px; font-size: 11px; color: var(--muted-foreground); }
  .terminal-context code { min-width: 0; overflow-wrap: anywhere; flex: 1; }
  .terminal-status { white-space: nowrap; }
  .terminal-suggestions { padding: 4px 12px 10px; display: flex; flex-wrap: wrap; gap: 6px; max-height: 170px; overflow: auto; flex-shrink: 0; }
  .terminal-suggestions p { width: 100%; margin: 0 0 2px; font-size: 11px; color: var(--muted-foreground); }
  .terminal-command { max-width: 100%; display: flex; align-items: baseline; gap: 8px; border: 1px solid var(--border); border-radius: 5px; padding: 5px 8px; text-align: left; font-size: 11px; background: var(--muted); }
  .terminal-command span { color: var(--muted-foreground); white-space: nowrap; }
  .terminal-command code { overflow-wrap: anywhere; }
  .terminal-command:disabled { opacity: .5; cursor: default; }
  .terminal-stage { flex: 1; min-height: 300px; position: relative; background: #16191f; overflow: hidden; }
  .terminal-surface { position: absolute; inset: 0; padding: 10px; overflow: hidden; }
  .terminal-surface :global(.xterm) { height: 100%; }
  .terminal-empty { position: absolute; inset: 0; display: grid; place-items: center; pointer-events: none; color: #9ca3af; font-size: 12px; padding: 20px; text-align: center; }
  footer { display: flex; flex-wrap: wrap; gap: 4px 12px; padding: 6px 12px; font-size: 10px; color: var(--muted-foreground); }
  .terminal-error { margin: 0; padding: 8px 12px; font-size: 12px; color: var(--destructive); }
  .terminal-history { flex: 1; min-height: 0; overflow: auto; padding: 12px; }
  .terminal-history article + article { margin-top: 16px; }
  .terminal-history-heading { display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 12px; margin-bottom: 6px; }
  .terminal-history-heading span:last-child, .terminal-history p { color: var(--muted-foreground); font-size: 11px; overflow-wrap: anywhere; }
  .terminal-history pre { background: #16191f; color: #e5e7eb; padding: 10px; border-radius: 5px; overflow: auto; font-size: 12px; line-height: 1.4; }
  .terminal-history summary { font-size: 11px; margin-top: 8px; cursor: pointer; }
  @media (max-width: 600px) { .terminal-toolbar { padding-inline: 8px; } .terminal-heading { width: 100%; } .terminal-command { flex-direction: column; gap: 2px; } }
</style>
