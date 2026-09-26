<script lang="ts">
  import { Camera, ClipboardPaste, Paperclip } from '@lucide/svelte'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import { unavailableInputToolsState, useInputTools } from './inputToolsContext'
  import type { InputTarget } from '../domain/inputTarget'
  import { useVoiceInput } from '../speech/voiceInputContext'
  import VoiceInputButton from '../speech/VoiceInputButton.svelte'

  export let target: InputTarget | null = null
  export let disabled = false
  export let label = 'Speak here'
  const tools = useInputTools()
  const toolsState = tools?.state ?? unavailableInputToolsState
  const voice = useVoiceInput()
  let fileInput: HTMLInputElement
  let fileTarget: InputTarget | null = null
  $: locked = disabled || $toolsState.disabled || $toolsState.busy || !target || target.requestId !== $toolsState.requestId
  const tr = (source: string) => t($locale, source)

  function acquire(kind: 'capture' | 'paste') {
    if (locked || !tools || !target) return
    const destination = structuredClone(target)
    voice?.selectTarget(destination)
    void Promise.resolve().then(() => tools[kind](destination)).catch(tools.reportError)
  }
  function chooseFiles() {
    if (locked || !target) return
    fileTarget = structuredClone(target)
    voice?.selectTarget(fileTarget)
    fileInput.click()
  }
  function selectedFiles(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const files = Array.from(input.files ?? [])
    const destination = fileTarget
    input.value = ''; fileTarget = null
    if (!destination || !files.length || !tools) return
    void Promise.resolve().then(() => tools.files(destination, files)).catch(tools.reportError)
  }
</script>

<div class="flex min-w-0 flex-wrap items-center gap-1" role="group" aria-label={tr('Input tools')} data-input-toolbar>
  <VoiceInputButton {target} {disabled} {label} iconOnly toggle />
  <button type="button" class="input-tool" disabled={locked || !$toolsState.canCapture} aria-label={tr('Capture')} title={tr('Capture')}
    onpointerdown={(event) => event.preventDefault()} onclick={() => acquire('capture')}><Camera class="size-3.5" /></button>
  <button type="button" class="input-tool" disabled={locked || !$toolsState.canPaste} aria-label={tr('Clipboard')} title={tr('Clipboard')}
    onpointerdown={(event) => event.preventDefault()} onclick={() => acquire('paste')}><ClipboardPaste class="size-3.5" /></button>
  <button type="button" class="input-tool" disabled={locked} aria-label={tr('Choose files')} title={tr('Choose files')}
    onpointerdown={(event) => event.preventDefault()} onclick={chooseFiles}><Paperclip class="size-3.5" /></button>
  <input bind:this={fileInput} type="file" multiple hidden onchange={selectedFiles} oncancel={() => fileTarget = null} />
</div>

<style>
  .input-tool { display: inline-flex; flex-shrink: 0; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: 6px; color: var(--muted-foreground); }
  .input-tool:hover:not(:disabled) { background: var(--muted); color: var(--foreground); }
  .input-tool:focus-visible { outline: 2px solid var(--ring); outline-offset: 1px; }
  .input-tool:disabled { opacity: .35; }
</style>
