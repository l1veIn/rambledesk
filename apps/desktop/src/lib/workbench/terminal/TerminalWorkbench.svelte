<script lang="ts">
  import { onMount } from 'svelte'
  import { ClipboardCopy, Maximize2, Play, Square, Terminal as TerminalIcon } from '@lucide/svelte'
  import '@xterm/xterm/css/xterm.css'
  import { Button } from '$lib/components/ui/button'
  import type { TerminalData } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { emptyTerminalState, TERMINAL_SESSION_LIMIT, type TerminalState } from '../terminalModel'
  import { terminalTextTail } from './terminalController'
  import type { TerminalWorkbenchController } from './terminalWorkbenchController'
  import { createXtermAdapter, type TerminalAdapter } from './xtermAdapter'
  import { terminalText } from './terminalI18n'
  import { isMissingTerminalSession } from './terminalErrors'
  import { initializeTerminalAppearance, terminalAppearance } from './terminalAppearance'
  import TerminalAppearancePicker from './TerminalAppearancePicker.svelte'

  export let data: TerminalData
  export let runtime: TerminalWorkbenchController | undefined = undefined
  export let state: TerminalState | null = null
  export let disabled = false
  export let readOnly = false
  export let onOpenReview: (() => void) | undefined = undefined
  export let onQuote: ((text: string) => void) | undefined = undefined

  let root: HTMLDivElement
  let adapter: TerminalAdapter | undefined
  let detachRuntime: (() => void) | undefined
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
  $: trialLimitReached = current.sessions.length >= TERMINAL_SESSION_LIMIT && latest?.status !== 'running'
  $: interactive = ready && !disabled && !readOnly && !busy && !error && latest?.status === 'running'
  $: adapter?.setInteractive(interactive)
  $: adapter?.setAppearance($terminalAppearance.theme)
  $: runtime?.setLocked(disabled || readOnly)
  $: if (readOnly && adapter) detach()

  function showError(cause: unknown) {
    error = isMissingTerminalSession(cause) ? tr('The terminal session was lost. Saved output is preserved; you can submit it with your feedback.')
      : cause instanceof Error ? cause.message : typeof cause === 'object' && cause && 'message' in cause
      ? String(cause.message) : tr('Terminal unavailable')
  }
  function resized(size: { cols: number; rows: number }) {
    if (resizeTimer) clearTimeout(resizeTimer)
    resizeTimer = setTimeout(() => { void runtime?.resize(size.cols, size.rows) }, 80)
  }
  function detach() {
    detachRuntime?.(); detachRuntime = undefined
    adapter?.dispose(); adapter = undefined
    ready = false
    busy = false
    if (resizeTimer) clearTimeout(resizeTimer)
  }
  async function initialize() {
    const next = await createXtermAdapter(root, {
      onData: (input) => { void runtime?.write(input).catch(() => undefined) },
      onResize: resized, onSelection: (text) => selection = text,
    }, $terminalAppearance.theme)
    if (!mounted || readOnly) { next.dispose(); return }
    adapter = next
    adapter.fit()
    if (!runtime) { showError(new Error(tr('Terminal unavailable'))); return }
    detachRuntime = runtime.attach?.({ renderer: adapter,
      onState: (next: TerminalState) => { state = next; current = next },
      onBusy: (value: boolean) => busy = value,
      onReady: () => ready = true,
      onError: showError,
    })
    runtime.setLocked(disabled || readOnly)
  }
  async function start() {
    if (!adapter || !runtime || disabled || readOnly || busy || trialLimitReached) return
    error = ''
    selection = ''
    try { const size = adapter.size(); await runtime.start(size.cols, size.rows); adapter?.focus() }
    catch { /* Controller presents the actionable connection error. */ }
  }
  async function stop() {
    error = ''
    try { await runtime?.stop() } catch { /* Keep the request editable for retry. */ }
  }
  function quoteSelection() {
    if (!selection.trim() || disabled || readOnly || !onQuote) return
    const text = terminalTextTail(selection.trim(), 8000)
    const heading = latest ? `${tr('Terminal trial')} · ${latest.cwd}` : tr('Terminal trial')
    onQuote(`${heading}\n\n${text.split('\n').map((line) => `> ${line}`).join('\n')}`)
  }
  onMount(() => {
    mounted = true
    const releaseAppearance = initializeTerminalAppearance()
    const observer = new ResizeObserver(() => adapter?.fit())
    if (!readOnly) {
      initialization = initialize()
      void initialization.catch(showError)
      observer.observe(root)
    }
    return () => { mounted = false; observer.disconnect(); detach(); releaseAppearance() }
  })
  // ANSI is rendered by xterm while live. History presents its saved screen plus a text transcript.
  function plainOutput(output: string) {
    return output.replace(/\x1b\][^\x07]*(?:\x07|\x1b\\)/g, '').replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '').replace(/\x1b[@-_]/g, '')
  }
</script>

<section class="terminal-workbench" data-terminal-workbench data-terminal-style={$terminalAppearance.id}
  style:--terminal-background={$terminalAppearance.theme.background} style:--terminal-foreground={$terminalAppearance.theme.foreground} style:--terminal-muted={$terminalAppearance.muted}>
  <header class="terminal-toolbar" data-terminal-toolbar>
    <div class="terminal-heading"><TerminalIcon class="size-4" /><span>{tr('Terminal trial')}</span></div>
    <div class="terminal-actions">
      <TerminalAppearancePicker />
      {#if !readOnly}
        {#if !latest || error || latest.status !== 'running'}
          <Button size="sm" variant="outline" disabled={!ready || disabled || busy || trialLimitReached} onclick={() => void start()}>
            <Play class="size-4" />{tr(latest?.status === 'stopped' || latest?.status === 'exited' ? 'Restart terminal' : error ? 'Retry connection' : 'Start terminal')}
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
          <Button size="sm" variant="ghost" data-tour="terminal-quote" disabled={!selection.trim() || disabled} onclick={quoteSelection}>
            <ClipboardCopy class="size-4" />{tr('Quote selected output')}
          </Button>
        {/if}
      {/if}
    </div>
  </header>
  <div class="terminal-context" data-tour="terminal-directory">
    <code title={data.cwd}>{data.cwd}</code>
    {#if latest}<span class="terminal-status" data-terminal-status>{tr(busy ? 'Connecting…' : latest.status === 'running' ? 'Running' : latest.status === 'stopped' ? 'Stopped' : 'Exited')}{latest.exit_code !== null ? ` · ${tr('Exit code')} ${latest.exit_code}` : ''}</span>{/if}
  </div>
  {#if !readOnly && trialLimitReached}<p class="terminal-limit">{tr('The 16 trial limit was reached. Submit feedback to keep the recorded trials.')}</p>{/if}
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
    <footer><span>{tr('Trial output is saved with your feedback.')}</span>{#if current.sessions.length > 1}<span>{tr('Recorded trials')}: {current.sessions.length}</span>{/if}{#if current.sessions.some((session) => session.truncated)}<span>{tr('Earlier output was truncated. The latest screen is preserved.')}</span>{/if}</footer>
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
  .terminal-limit { margin: 0; padding: 8px 12px; font-size: 12px; color: var(--muted-foreground); }
  .terminal-stage { flex: 1; min-height: 300px; position: relative; background: var(--terminal-background); overflow: hidden; }
  .terminal-surface { position: absolute; inset: 0; padding: 10px; overflow: hidden; }
  .terminal-surface :global(.xterm) { height: 100%; }
  .terminal-empty { position: absolute; inset: 0; display: grid; place-items: center; pointer-events: none; color: var(--terminal-muted); font-size: 12px; padding: 20px; text-align: center; }
  footer { display: flex; flex-wrap: wrap; gap: 4px 12px; padding: 6px 12px; font-size: 10px; color: var(--muted-foreground); }
  .terminal-error { margin: 0; padding: 8px 12px; font-size: 12px; color: var(--destructive); }
  .terminal-history { flex: 1; min-height: 0; overflow: auto; padding: 12px; }
  .terminal-history article + article { margin-top: 16px; }
  .terminal-history-heading { display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 12px; margin-bottom: 6px; }
  .terminal-history-heading span:last-child, .terminal-history p { color: var(--muted-foreground); font-size: 11px; overflow-wrap: anywhere; }
  .terminal-history pre { background: var(--terminal-background); color: var(--terminal-foreground); padding: 10px; border-radius: 5px; overflow: auto; font-size: 12px; line-height: 1.4; }
  .terminal-history summary { font-size: 11px; margin-top: 8px; cursor: pointer; }
  @media (max-width: 600px) { .terminal-toolbar { padding-inline: 8px; } .terminal-heading { width: 100%; } }
</style>
