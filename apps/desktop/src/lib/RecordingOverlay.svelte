<script lang="ts">
  import { Check, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, CircleAlert, GripHorizontal, LoaderCircle, Mic, Pause, RotateCcw, X } from '@lucide/svelte'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { RambleConsoleCommand } from './rambleConsole'
  import { selectedSpeechGroup, speechOverlayVisible, type SpeechOverlayState } from './speechOverlay'
  import { speechOverlayDrag } from './speechOverlayDrag'

  export let state: SpeechOverlayState
  export let embedded = false
  export let draggable = true
  export let onStartDrag: ((event: PointerEvent) => void) | undefined = undefined
  export let onCommand: (command: RambleConsoleCommand) => void = () => {}

  let expanded = false
  const wavePattern = [0.3, 0.45, 0.7, 0.5, 0.85, 0.65, 1, 0.6, 0.85, 0.5, 0.72, 0.9, 0.55, 0.7, 0.45, 0.3]
  $: group = selectedSpeechGroup(state)
  $: groupIndex = group ? state.groups.indexOf(group) : 0
  $: recording = state.phase === 'listening' || state.phase === 'processing'
  $: busy = state.phase === 'starting' || state.phase === 'stopping'
  $: target = group ?? state.receipt ?? state.target
  $: text = group?.text || state.receipt?.text || state.partial
  $: error = group?.error || state.error
  $: status = group?.error ? tr('Could not write speech')
    : group?.busy ? tr('Writing to feedback…')
    : group ? tr('Waiting for your confirmation')
    : state.receipt ? tr('Written to feedback')
    : state.phase === 'error' ? tr('Recording interrupted')
    : state.phase === 'starting' ? tr('Starting…')
    : state.phase === 'processing' || state.phase === 'stopping' ? tr('Transcribing…')
    : tr('Listening…')

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

{#if speechOverlayVisible(state)}
  <div class="speech-capsule-host" class:embedded use:speechOverlayDrag={!embedded && draggable} style={`--speech-overlay-opacity: ${state.opacity}`}>
    <section class="speech-capsule" class:has-error={!!error} aria-label={tr('Speech transcription')}>
      <header>
        {#if draggable}
          <button type="button" class="icon-button drag-handle" data-speech-drag-handle onpointerdown={onStartDrag} title={tr('Drag speech overlay')} aria-label={tr('Drag speech overlay')}><GripHorizontal size={16} /></button>
        {/if}
        <span class="status-icon" class:recording class:success={!!state.receipt && !group}>
          {#if error}<CircleAlert size={16} />
          {:else if state.receipt && !group}<Check size={16} />
          {:else if group?.busy || busy || state.phase === 'processing'}<LoaderCircle size={16} class="spin" />
          {:else}<Mic size={16} />{/if}
        </span>
        <span class="status-label" role="status">{status}</span>
        {#if recording && group}<span class="recording-dot" title={tr('Recording continues')} aria-label={tr('Recording continues')}></span>{/if}
        {#if state.target && state.phase !== 'idle' && state.phase !== 'error'}
          <button class="icon-button" disabled={busy} onclick={() => onCommand({ type: 'toggle-recording' })} title={tr('Pause recording')} aria-label={tr('Pause recording')}><Pause size={15} /></button>
        {/if}
        {#if state.phase === 'error' && group && state.target}
          <button class="icon-button" onclick={() => onCommand({ type: 'retry-recording' })} title={tr('Retry recording')} aria-label={tr('Retry recording')}><RotateCcw size={15} /></button>
        {/if}
        {#if text}<button class="icon-button" aria-expanded={expanded} onclick={() => expanded = !expanded} title={expanded ? tr('Collapse transcript') : tr('Expand transcript')} aria-label={expanded ? tr('Collapse transcript') : tr('Expand transcript')}>{#if expanded}<ChevronDown size={16} />{:else}<ChevronUp size={16} />{/if}</button>{/if}
      </header>

      {#if text}
        <div class="transcript" class:expanded><p>{expanded ? text : text.slice(-240)}</p></div>
      {:else}
        <div class="waveform" aria-label={tr('Microphone level')}>
          {#each wavePattern as weight, index (index)}
            <span style={`height: ${3 + Math.max(0, Math.min(1, state.level)) * weight * 27}px`}></span>
          {/each}
        </div>
      {/if}

      {#if group && state.partial}
        <p class="live-tail">{tr('Listening: {text}', { text: state.partial.slice(-90) })}</p>
      {/if}
      {#if error}<p class="error-message" role="alert">{error}</p>{/if}

      {#if target}
        <button class="destination" onclick={() => onCommand({ type: 'open-speech-target', requestId: target.requestId, segmentId: group ? undefined : state.receipt?.id })} title={`${tr('Open feedback document')} · ${target.requestTitle}${target.action ? ` · ${target.action.title}` : ''}`}>
          <span>{target.requestTitle}{target.action ? ` · ${target.action.title}` : ''}</span><ChevronRight size={13} />
        </button>
      {/if}

      {#if group}
        <footer>
          {#if state.groups.length > 1}
            <nav aria-label={tr('Pending speech groups')}>
              <button class="icon-button" disabled={groupIndex === 0} onclick={() => onCommand({ type: 'select-speech-group', id: state.groups[groupIndex - 1].ids[0] })} aria-label={tr('Previous group')}><ChevronLeft size={15} /></button>
              <span>{groupIndex + 1} / {state.groups.length}</span>
              <button class="icon-button" disabled={groupIndex === state.groups.length - 1} onclick={() => onCommand({ type: 'select-speech-group', id: state.groups[groupIndex + 1].ids[0] })} aria-label={tr('Next group')}><ChevronRight size={15} /></button>
            </nav>
          {:else}<span class="draft-note">{tr('Feedback draft')}</span>{/if}
          <button class="text-button discard" disabled={group.busy} title={`${tr('Discard')} · ${state.shortcuts.speechDiscard}`} onclick={() => onCommand({ type: 'discard-speech', ids: [...group.ids] })}>{tr('Discard')}</button>
          <button class="text-button primary" disabled={group.busy} title={`${tr('Write to feedback')} · ${state.shortcuts.speechAccept}`} onclick={() => onCommand({ type: 'accept-speech', ids: [...group.ids] })}>{#if group.busy}<LoaderCircle size={14} class="spin" />{:else}<Check size={14} />{/if}{group.error ? tr('Retry writing') : tr('Write to feedback')}</button>
        </footer>
      {:else if state.phase === 'error'}
        <footer>
          <button class="text-button discard" onclick={() => onCommand({ type: 'exit' })}><X size={14} />{tr('Close')}</button>
          <button class="text-button primary" onclick={() => onCommand({ type: 'retry-recording' })}><RotateCcw size={14} />{tr('Retry recording')}</button>
        </footer>
      {/if}
    </section>
  </div>
{/if}

<style>
  .speech-capsule-host { position: fixed; bottom: 24px; left: 50%; z-index: 80; width: min(420px, calc(100vw - 32px)); transform: translateX(-50%); pointer-events: none; }
  .speech-capsule-host.embedded { position: relative; bottom: auto; left: auto; width: 100%; transform: none; padding: 8px; }
  .speech-capsule { position: relative; isolation: isolate; pointer-events: auto; padding: 12px 14px 10px; border: 1px solid var(--border); border-radius: 22px; background: transparent; color: var(--foreground); box-shadow: 0 6px 22px #0002; font-size: 12px; }
  .speech-capsule::before { content: ''; position: absolute; inset: 0; z-index: -1; border-radius: inherit; background: var(--card); backdrop-filter: blur(calc(var(--speech-overlay-opacity, 95) * 0.18px)); opacity: calc(var(--speech-overlay-opacity, 95) / 100); }
  .speech-capsule.has-error { border-color: color-mix(in srgb, var(--destructive) 55%, var(--border)); }
  header, footer, nav, .destination, .text-button { display: flex; align-items: center; gap: 8px; }
  header { min-height: 24px; }
  .status-label { flex: 1; font-size: 11px; font-weight: 600; }
  .status-icon { display: flex; color: var(--muted-foreground); }
  .status-icon.recording { color: var(--destructive); }
  .status-icon.success { color: var(--success, #289b77); }
  .recording-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--destructive); }
  button { cursor: pointer; font: inherit; }
  button:disabled { opacity: .45; cursor: default; }
  button:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
  .icon-button { display: grid; place-items: center; width: 25px; height: 25px; border: 0; border-radius: 8px; background: transparent; color: var(--muted-foreground); }
  .icon-button:hover:not(:disabled), .discard:hover:not(:disabled) { background: var(--muted); color: var(--foreground); }
  .drag-handle { cursor: grab; touch-action: none; user-select: none; }
  .drag-handle:active { cursor: grabbing; }
  .transcript { margin: 7px 0; height: 40px; overflow: hidden; white-space: pre-wrap; overflow-wrap: anywhere; line-height: 20px; font-size: 13px; display: flex; flex-direction: column; justify-content: flex-end; }
  .transcript p { flex-shrink: 0; margin: 0; }
  .transcript.expanded { display: block; height: auto; min-height: 40px; max-height: min(230px, 40vh); overflow-y: auto; }
  .embedded .transcript.expanded { max-height: 230px; }
  .waveform { display: flex; gap: 3px; height: 40px; align-items: center; justify-content: center; margin: 7px 0; }
  .waveform span { width: 3px; border-radius: 4px; background: var(--primary); transition: height 90ms ease-out; }
  .destination { width: 100%; padding: 4px 0; border: 0; background: transparent; color: var(--muted-foreground); text-align: left; font-size: 10px; }
  .destination span { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .destination:hover { color: var(--primary); }
  .destination :global(svg) { flex-shrink: 0; }
  .live-tail, .error-message { margin: 6px 0; font-size: 11px; line-height: 16px; }
  .live-tail { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
  .error-message { color: var(--destructive); max-height: 64px; overflow-y: auto; overflow-wrap: anywhere; }
  footer { margin-top: 8px; gap: 6px; }
  nav, .draft-note { margin-right: auto; font-size: 10px; color: var(--muted-foreground); }
  nav { gap: 2px; }
  .text-button { justify-content: center; min-height: 30px; padding: 5px 10px; border: 0; border-radius: 10px; font-size: 11px; }
  .discard { background: transparent; color: var(--muted-foreground); }
  .primary { background: var(--primary); color: var(--primary-foreground); }
  .primary:hover:not(:disabled) { filter: brightness(1.1); }
  :global(.speech-capsule .spin) { animation: speech-spin 1s linear infinite; }
  @keyframes speech-spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { :global(.speech-capsule .spin) { animation: none; } .waveform span { transition: none; } }
</style>
