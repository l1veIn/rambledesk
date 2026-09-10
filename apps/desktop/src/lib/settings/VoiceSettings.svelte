<script lang="ts">
  import { onMount } from 'svelte'
  import { Check, Download, LoaderCircle, Mic, Plus, RefreshCw, Sparkles, Trash2, Volume2, X } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Select from '$lib/components/ui/select'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import ShortcutSettings from '$lib/settings/ShortcutSettings.svelte'
  import { t } from '$lib/i18n'
  import {
    DEFAULT_SPEECH_MODEL_ID,
    locale,
    setSpeechAutoTidy,
    setSpeechConfirmBeforeWrite,
    setSpeechHotwords,
    setSpeechInputDevice,
    setSpeechModelId,
    setSpeechOverlayEnabled,
    setSpeechOverlayOpacity,
    setSpeechVadSilenceMs,
    setSpeechVadThreshold,
    speechAutoTidy,
    speechConfirmBeforeWrite,
    speechHotwords,
    speechInputDevice,
    speechModelId,
    speechOverlayEnabled,
    speechOverlayOpacity,
    speechVadSilenceMs,
    speechVadThreshold,
    type SpeechModelId,
  } from '$lib/preferences'
  import {
    speechModelDescription,
    speechModelDisplayName,
    speechModelLanguages,
  } from '$lib/speech/speechModelLabels'
  import { resolveSupportedSpeechModelId } from '$lib/capabilities/speechModelSelection'

  type SpeechModelInfo = Awaited<ReturnType<WorkbenchCapabilities['speech']['implementation']['listModels']>>[number]
  type SpeechModelProgress = {
    model_id: string
    downloaded: number
    total: number
  }

  export let capabilities: WorkbenchCapabilities

  let speechInputDevices: string[] = []
  let speechDeviceError = ''
  let speechModels: SpeechModelInfo[] = []
  let modelProgress: SpeechModelProgress | null = null
  let modelBusy = false
  let modelError = ''
  let hotwordDraft = ''
  let unlistenModelProgress: (() => void) | null = null

  $: selectedSpeechModel =
    speechModels.find((model) => model.id === $speechModelId) ?? speechModels[0] ?? null

  onMount(() => {
    void refreshSpeechDevices()
    void refreshSpeechModels()
    unlistenModelProgress = capabilities.speech.implementation.onModelProgress(
      (progress) => (modelProgress = { ...progress }),
      () => undefined,
    )
    return () => unlistenModelProgress?.()
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return String(cause)
  }

  async function refreshSpeechModels() {
    modelError = ''
    try {
      speechModels = [...await capabilities.speech.implementation.listModels()]
      const supportedModelId = resolveSupportedSpeechModelId($speechModelId, speechModels)
      if (supportedModelId && supportedModelId !== $speechModelId) setSpeechModelId(supportedModelId)
    } catch (cause) {
      modelError = messageFrom(cause)
    }
  }

  async function downloadSpeechModel() {
    if (modelBusy || !selectedSpeechModel) return
    const modelId = selectedSpeechModel.id
    modelBusy = true
    modelError = ''
    modelProgress = { model_id: modelId, downloaded: 0, total: selectedSpeechModel.size_bytes }
    try {
      const updated = await capabilities.speech.implementation.downloadModel(modelId)
      speechModels = speechModels.map((model) => (model.id === updated.id ? updated : model))
    } catch (cause) {
      modelError = messageFrom(cause)
    } finally {
      modelBusy = false
    }
  }

  async function deleteSpeechModel() {
    if (modelBusy || !selectedSpeechModel || !confirm(tr('Delete the local speech model?'))) return
    const modelId = selectedSpeechModel.id
    modelBusy = true
    modelError = ''
    try {
      const updated = await capabilities.speech.implementation.deleteModel(modelId)
      speechModels = speechModels.map((model) => (model.id === updated.id ? updated : model))
      modelProgress = null
    } catch (cause) {
      modelError = messageFrom(cause)
    } finally {
      modelBusy = false
    }
  }

  function addHotword() {
    const next = hotwordDraft.trim()
    if (!next) return
    if ($speechHotwords.includes(next)) {
      hotwordDraft = ''
      return
    }
    setSpeechHotwords([...$speechHotwords, next])
    hotwordDraft = ''
  }

  function removeHotword(word: string) {
    setSpeechHotwords($speechHotwords.filter((item) => item !== word))
  }

  async function refreshSpeechDevices() {
    speechDeviceError = ''
    try {
      speechInputDevices = [...await capabilities.speech.implementation.listInputDevices()]
    } catch (cause) {
      speechDeviceError = messageFrom(cause)
    }
  }
</script>

            <section class="space-y-5 border-b pb-8">
              <div class="flex items-center justify-between gap-8">
                <div>
                  <h3 class="m-0 text-sm font-medium" id="speech-overlay-label">{tr('Show speech overlay')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground" id="speech-overlay-description">{tr('Show transcription above other windows. Drag the handle to move it; its position is remembered. Hiding it keeps recording and confirmation shortcuts available.')}</p>
                </div>
                <button type="button" role="switch" aria-checked={$speechOverlayEnabled} aria-labelledby="speech-overlay-label" aria-describedby="speech-overlay-description"
                  class={`relative h-6 w-11 shrink-0 rounded-full transition-colors focus-visible:outline-2 focus-visible:outline-ring ${$speechOverlayEnabled ? 'bg-primary' : 'bg-muted-foreground/30'}`}
                  onclick={() => setSpeechOverlayEnabled(!$speechOverlayEnabled)}>
                  <span class={`absolute top-0.5 size-5 rounded-full bg-background shadow-sm transition-transform ${$speechOverlayEnabled ? 'left-0.5 translate-x-5' : 'left-0.5'}`}></span>
                </button>
              </div>
              <div class="flex items-center gap-4">
                <label for="speech-overlay-opacity" class="text-xs">{tr('Overlay opacity')}</label>
                <input id="speech-overlay-opacity" type="range" min="30" max="100" step="1" value={$speechOverlayOpacity} disabled={!$speechOverlayEnabled}
                  class="max-w-64 flex-1 accent-primary disabled:opacity-40" oninput={(event) => setSpeechOverlayOpacity(Number(event.currentTarget.value))} />
                <output for="speech-overlay-opacity" class="w-10 text-xs tabular-nums text-muted-foreground">{$speechOverlayOpacity}%</output>
              </div>
              <p class="m-0 text-xs text-muted-foreground">{tr('Adjust background opacity while keeping the text readable.')}</p>
            </section>
            <section class="flex items-center justify-between gap-8 border-b pb-8">
              <div>
                <h3 class="m-0 text-sm font-medium" id="speech-confirm-label">{tr('Confirm before writing speech')}</h3>
                <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground" id="speech-confirm-description">
                  {tr('Keep transcribed speech pending until you write it to the feedback draft or discard it. Recording can continue while you review. When the overlay is hidden, review pending speech in the main window.')}
                </p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={$speechConfirmBeforeWrite}
                aria-labelledby="speech-confirm-label"
                aria-describedby="speech-confirm-description"
                class={`relative h-6 w-11 shrink-0 rounded-full transition-colors focus-visible:outline-2 focus-visible:outline-ring ${$speechConfirmBeforeWrite ? 'bg-primary' : 'bg-muted-foreground/30'}`}
                onclick={() => setSpeechConfirmBeforeWrite(!$speechConfirmBeforeWrite)}
              >
                <span class={`absolute top-0.5 size-5 rounded-full bg-background shadow-sm transition-transform ${$speechConfirmBeforeWrite ? 'left-0.5 translate-x-5' : 'left-0.5'}`}></span>
              </button>
            </section>
            {#if $speechConfirmBeforeWrite}
              <section class="space-y-3 border-b pb-8">
                <div class="flex items-center justify-between gap-8">
                  <div>
                    <h3 class="m-0 text-sm font-medium" id="speech-auto-tidy-label">{tr('Automatically tidy speech')}</h3>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground" id="speech-auto-tidy-description">
                      {tr('Tidy each transcribed segment before you review it. You still choose when to write it to feedback.')}
                    </p>
                  </div>
                  <button
                    type="button"
                    role="switch"
                    aria-checked={$speechAutoTidy}
                    aria-labelledby="speech-auto-tidy-label"
                    aria-describedby="speech-auto-tidy-description speech-auto-tidy-hint"
                    class={`relative h-6 w-11 shrink-0 rounded-full transition-colors focus-visible:outline-2 focus-visible:outline-ring ${$speechAutoTidy ? 'bg-primary' : 'bg-muted-foreground/30'}`}
                    onclick={() => setSpeechAutoTidy(!$speechAutoTidy)}
                  >
                    <span class={`absolute top-0.5 size-5 rounded-full bg-background shadow-sm transition-transform ${$speechAutoTidy ? 'left-0.5 translate-x-5' : 'left-0.5'}`}></span>
                  </button>
                </div>
                <p class="m-0 text-xs leading-5 text-muted-foreground" id="speech-auto-tidy-hint">
                  {tr('Tidy also has an Auto-tidy threshold under Post-processing → Tidy. Review both settings to avoid tidying the same text twice.')}
                </p>
              </section>
              <ShortcutSettings globalShortcuts={capabilities.globalShortcuts} onlyActions={['speechAccept', 'speechDiscard']} />
            {/if}
            <section class="grid grid-cols-[minmax(0,1fr)_280px] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <Mic class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('Microphone')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('Choose the input device used for Ramble recording.')}
                  </p>
                </div>
              </div>
              <div class="flex gap-2">
                <Select.Root
                  type="single"
                  value={$speechInputDevice || '__default__'}
                  onValueChange={(value: string) => setSpeechInputDevice(value === '__default__' ? '' : value)}
                >
                  <Select.Trigger class="min-w-0 flex-1">
                    {$speechInputDevice || tr('System default microphone')}
                  </Select.Trigger>
                  <Select.Content>
                    <Select.Item value="__default__" label={tr('System default microphone')} />
                    {#each speechInputDevices as device (device)}
                      <Select.Item value={device} label={device} />
                    {/each}
                  </Select.Content>
                </Select.Root>
                <Button variant="outline" size="icon" onclick={() => void refreshSpeechDevices()}>
                  <RefreshCw />
                </Button>
              </div>
              {#if speechDeviceError}
                <p class="col-span-2 m-0 text-xs text-destructive">{speechDeviceError}</p>
              {/if}
            </section>

            <section class="border-b pb-8">
              <div class="grid grid-cols-[minmax(0,1fr)_280px] items-center gap-8">
                <div class="flex gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <Download class="size-4" />
                  </span>
                  <div>
                    <div class="flex items-center gap-2">
                      <h3 class="m-0 text-sm font-medium">{tr('Transcription model')}</h3>
                      {#if selectedSpeechModel}
                        <Badge variant={selectedSpeechModel.installed ? 'secondary' : 'outline'}>
                          {selectedSpeechModel.installed ? tr('Installed') : tr('Not installed')}
                        </Badge>
                      {/if}
                    </div>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('Choose the local model for Ramble voice input. Each model can be downloaded or removed separately.')}
                    </p>
                  </div>
                </div>
                <Select.Root
                  type="single"
                  value={$speechModelId}
                  onValueChange={(value: string) => setSpeechModelId(value as SpeechModelId)}
                >
                  <Select.Trigger class="w-full">
                    {selectedSpeechModel
                      ? speechModelDisplayName(
                          $locale,
                          selectedSpeechModel.id,
                          selectedSpeechModel.display_name,
                        )
                      : tr('Loading models…')}
                  </Select.Trigger>
                  <Select.Content>
                    {#each speechModels as model (model.id)}
                      <Select.Item
                        value={model.id}
                        label={`${speechModelDisplayName($locale, model.id, model.display_name)}${model.id === DEFAULT_SPEECH_MODEL_ID ? ` · ${tr('Recommended')}` : ''}${model.installed ? ` · ${tr('Installed')}` : ''}`}
                      />
                    {/each}
                  </Select.Content>
                </Select.Root>
              </div>

              {#if selectedSpeechModel}
                <div class="ml-11 mt-4 rounded-md border bg-muted/20 p-4">
                  <div class="flex items-start justify-between gap-4">
                    <div class="min-w-0">
                      <div class="flex flex-wrap items-center gap-1.5">
                        {#if selectedSpeechModel.id === DEFAULT_SPEECH_MODEL_ID}
                          <Badge variant="secondary">{tr('Recommended')}</Badge>
                        {/if}
                        <Badge variant="outline">
                          {selectedSpeechModel.streaming ? tr('Live streaming') : tr('VAD segmented · Non-streaming')}
                        </Badge>
                        <span class="text-[10px] text-muted-foreground">
                          {Math.round(selectedSpeechModel.size_bytes / 1024 / 1024)} MB · {speechModelLanguages($locale, selectedSpeechModel.id, [...selectedSpeechModel.languages]).join(' / ')}
                        </span>
                      </div>
                      <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">
                        {speechModelDescription(
                          $locale,
                          selectedSpeechModel.id,
                          selectedSpeechModel.description,
                        )}
                      </p>
                      <p class="m-0 mt-1 truncate text-[10px] text-muted-foreground" title={selectedSpeechModel.path}>
                        {selectedSpeechModel.path}
                      </p>
                      <p class="m-0 mt-1 text-[10px] text-muted-foreground">
                        {tr('Model license')}：{selectedSpeechModel.license}
                      </p>
                    </div>
                    {#if selectedSpeechModel.installed}
                      <Button variant="outline" size="sm" disabled={modelBusy} onclick={deleteSpeechModel}>
                        <Trash2 data-icon="inline-start" />{tr('Delete')}
                      </Button>
                    {:else}
                      <Button size="sm" disabled={modelBusy} onclick={downloadSpeechModel}>
                        {#if modelBusy}
                          <LoaderCircle class="animate-spin" data-icon="inline-start" />
                        {:else}
                          <Download data-icon="inline-start" />
                        {/if}
                        {modelBusy ? tr('Downloading…') : modelError ? tr('Retry download') : tr('Download model')}
                      </Button>
                    {/if}
                  </div>
                  {#if modelBusy && modelProgress?.model_id === selectedSpeechModel.id}
                    <div class="mt-3">
                      <div class="mb-1 flex justify-between text-[10px] text-muted-foreground">
                        <span>{tr('Downloading and verifying…')}</span>
                        <span>{Math.min(100, Math.round(modelProgress.downloaded / Math.max(1, modelProgress.total) * 100))}%</span>
                      </div>
                      <div class="h-1.5 overflow-hidden rounded-full bg-muted">
                        <div class="h-full bg-primary transition-[width]" style={`width: ${Math.min(100, modelProgress.downloaded / Math.max(1, modelProgress.total) * 100)}%`}></div>
                      </div>
                    </div>
                  {/if}
                  {#if modelError}
                    <p class="m-0 mt-2 text-xs text-destructive">{modelError}</p>
                  {/if}
                </div>
              {/if}
            </section>

            {#if selectedSpeechModel?.hotwords_supported}
              <section class="border-b pb-8">
                <div class="flex items-start gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <Sparkles class="size-4" />
                  </span>
                  <div class="min-w-0 flex-1">
                    <h3 class="m-0 text-sm font-medium">{tr('Hotword library')}</h3>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('Bias transcription toward the terms you speak most often. Add or remove hotwords below.')}
                    </p>
                    <div class="mt-4 rounded-md border bg-muted/20 p-4">
                      <div class="flex flex-wrap gap-2">
                        {#each $speechHotwords as word (word)}
                          <span class="flex items-center gap-1 rounded-full border bg-background px-2 py-1 text-xs">
                            {word}
                            <button
                              type="button"
                              class="grid size-4 place-items-center rounded-full text-muted-foreground hover:bg-muted hover:text-foreground"
                              aria-label={tr('Remove hotword {word}', { word })}
                              title={tr('Remove hotword {word}', { word })}
                              onclick={() => removeHotword(word)}
                            >
                              <X class="size-3" />
                            </button>
                          </span>
                        {:else}
                          <span class="text-xs text-muted-foreground">{tr('No hotwords configured.')}</span>
                        {/each}
                      </div>
                      <div class="mt-3 flex items-center gap-2">
                        <input
                          class="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-xs"
                          type="text"
                          placeholder={tr('Add a hotword…')}
                          bind:value={hotwordDraft}
                          onkeydown={(event) => {
                            if (event.key === 'Enter') addHotword()
                          }}
                        />
                        <Button size="sm" disabled={!hotwordDraft.trim()} onclick={addHotword}>
                          {tr('Add')}
                        </Button>
                      </div>
                    </div>
                  </div>
                </div>
              </section>
            {/if}

            <section class="flex items-start gap-3">
              <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                <Volume2 class="size-4" />
              </span>
              <div class="min-w-0 flex-1">
                <h3 class="m-0 text-sm font-medium">{tr('Voice activity detection (VAD)')}</h3>
                <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                  {tr('Non-streaming models use bundled Silero VAD to split long recordings; streaming models use their own endpoints.')}
                </p>
                <div class="mt-4 grid gap-5 rounded-md border bg-muted/20 p-4">
                  <div class="grid grid-cols-[minmax(0,1fr)_280px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('Speech threshold')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('Raise it in noisy environments; lower it when quiet speech is often missed.')}
                      </span>
                    </div>
                    <div class="flex items-center gap-3">
                      <input
                        type="range"
                        min="5"
                        max="95"
                        step="5"
                        value={Math.round($speechVadThreshold * 100)}
                        class="min-w-0 flex-1 accent-primary"
                        aria-label={tr('VAD speech threshold')}
                        oninput={(event) =>
                          setSpeechVadThreshold(Number((event.currentTarget as HTMLInputElement).value) / 100)}
                      />
                      <span class="w-10 text-right text-[10px] tabular-nums text-muted-foreground">
                        {$speechVadThreshold.toFixed(2)}
                      </span>
                    </div>
                  </div>
                  <div class="grid grid-cols-[minmax(0,1fr)_280px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('Silence segmentation')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('After this much continuous silence, send the current speech segment to the non-streaming model.')}
                      </span>
                    </div>
                    <div class="flex items-center gap-3">
                      <input
                        type="range"
                        min="200"
                        max="5000"
                        step="100"
                        value={$speechVadSilenceMs}
                        class="min-w-0 flex-1 accent-primary"
                        aria-label={tr('VAD silence duration')}
                        oninput={(event) =>
                          setSpeechVadSilenceMs(Number((event.currentTarget as HTMLInputElement).value))}
                      />
                      <span class="w-12 text-right text-[10px] tabular-nums text-muted-foreground">
                        {($speechVadSilenceMs / 1000).toFixed(1)} s
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </section>
