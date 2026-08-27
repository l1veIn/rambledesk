<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { open } from '@tauri-apps/plugin-dialog'
  import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification'
  import {
    BellRing,
    Check,
    ChefHat,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    Clipboard,
    Download,
    FolderCog,
    HardDrive,
    LoaderCircle,
    Mic,
    PlugZap,
    Rocket,
    ShieldCheck,
    Sparkles,
    Volume2,
  } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import MacPermissions from '$lib/MacPermissions.svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { toast } from '$lib/components/ui/sonner'
  import { t } from '$lib/i18n'
  import { currentDesktopPlatform } from '$lib/platform'
  import {
    speechModelDescription,
    speechModelDisplayName,
  } from '$lib/speechModelLabels'
  import piLogoSvg from '../assets/pi-logo.svg?raw'
  import dshLogoSvg from '../assets/dsh-logo.svg?raw'
  import {
    DEFAULT_SPEECH_MODEL_ID,
    cookingApiKey,
    cookingBaseUrl,
    cookingEnabled,
    cookingModel,
    cookingProvider,
    cookingReasoningEffort,
    finishOnboarding,
    locale,
    notificationPopupEnabled,
    notificationSoundEnabled,
    onboardingStep,
    setCookingApiKey,
    setCookingBaseUrl,
    setCookingEnabled,
    setCookingModel,
    setCookingProvider,
    setCookingReasoningEffort,
    setNotificationPopupEnabled,
    setNotificationSoundEnabled,
    setLocale,
    setOnboardingStep,
    setSpeechModelId,
    speechModelId,
    type CookingProvider,
    type CookingReasoningEffort,
    type SpeechModelId,
  } from '$lib/preferences'

  export let openWizard = false
  export let onClose: () => void = () => {}

  type StorageView = { active_path: string; selected_path: string; restart_required: boolean }
  type SpeechModel = {
    id: SpeechModelId
    display_name: string
    description: string
    size_bytes: number
    installed: boolean
    streaming: boolean
    languages: string[]
  }
  type ModelProgress = { model_id: string; downloaded: number; total: number }
  type McpHost = { id: string; name: string; installed: boolean; configured: boolean }
  type McpInstallResult = { action: 'created' | 'updated' | 'unchanged' }
  type PiPackageStatus = {
    cliAvailable: boolean
    installed: boolean
    sourceCount: number
    restartRequired: boolean
  }

  const baseSteps = ['Welcome', 'Storage', 'Voice input', 'Adapters', 'Notifications', 'Cooking', 'Finish']
  const macSteps = ['Welcome', 'Storage', 'Voice input', 'Permissions', 'Adapters', 'Notifications', 'Cooking', 'Finish']
  let steps = baseSteps
  let showMacPermissionStep = false
  const isTauri = '__TAURI_INTERNALS__' in window
  const isWindows = currentDesktopPlatform() === 'Windows'
  let step = 0
  let wasOpen = false
  let closing = false
  let storage: StorageView | null = null
  let storageBusy = false
  let storageRestartRequired = false
  let permissionRestartRequired = false
  let models: SpeechModel[] = []
  let modelBusy = false
  let modelProgress: ModelProgress | null = null
  let hosts: McpHost[] = []
  let hostsLoading = false
  let hostSelections = new Set<string>()
  let adapterBusy = false
  let piBusy = false
  let piStatus: PiPackageStatus | null = null
  let piStatusLoading = isTauri
  let dshBusy = false
  let promptCopyState: 'idle' | 'copied' | 'error' = 'idle'
  let notificationBusy = false
  let unlistenModelProgress: UnlistenFn | undefined

  $: selectedModel = models.find((model) => model.id === $speechModelId) ?? models[0]
  $: selectedHosts = [...hostSelections]
  $: modelProgressPercent = modelProgress
    ? Math.min(100, Math.round((modelProgress.downloaded / Math.max(1, modelProgress.total)) * 100))
    : 0
  $: if (openWizard && !wasOpen) {
    wasOpen = true
    closing = false
    step = Math.min(steps.length - 1, onboardingStep())
  }
  $: if (!openWizard && wasOpen) {
    wasOpen = false
    if (!closing) complete(false)
  }

  onMount(() => {
    if (isTauri) {
      void loadStorage()
      void loadModels()
      void loadHosts()
      void loadPiStatus()
      void loadMacPermissionStep()
      void listen<ModelProgress>('speech-model-progress', ({ payload }) => (modelProgress = payload)).then(
        (unlisten) => (unlistenModelProgress = unlisten),
      )
    }
    return () => unlistenModelProgress?.()
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function move(next: number) {
    step = Math.max(0, Math.min(steps.length - 1, next))
    setOnboardingStep(step)
  }

  async function loadMacPermissionStep() {
    try {
      const permissions = await invoke<{ id: string; status: string }[]>('list_macos_permissions')
      showMacPermissionStep = permissions.length > 0
      steps = showMacPermissionStep ? macSteps : baseSteps
      step = Math.max(0, Math.min(steps.length - 1, step))
    } catch {
      showMacPermissionStep = false
      steps = baseSteps
      step = Math.max(0, Math.min(steps.length - 1, step))
    }
  }

  function complete(showToast = true) {
    closing = true
    finishOnboarding()
    openWizard = false
    onClose()
    if (showToast) toast.success(tr('RambleDesk is ready'))
  }

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return String(cause)
  }

  function mb(bytes: number) {
    return `${Math.round(bytes / 1024 / 1024)} MB`
  }

  async function loadStorage() {
    try {
      storage = await invoke<StorageView>('get_data_storage_settings')
    } catch (cause) {
      toast.error(tr('Could not read the data storage location'), { description: messageFrom(cause) })
    }
  }

  async function chooseStorage() {
    if (!isTauri || storageBusy) return
    const path = await open({ directory: true, multiple: false })
    if (!path || Array.isArray(path)) return
    storageBusy = true
    try {
      storage = await invoke<StorageView>('set_data_storage_path', { path })
      storageRestartRequired = storage.restart_required
      toast.success(tr('Data storage location saved'))
    } catch (cause) {
      toast.error(tr('Could not change the data storage location'), { description: messageFrom(cause) })
    } finally {
      storageBusy = false
    }
  }

  async function restartForStorage() {
    setOnboardingStep(2)
    try {
      await invoke('restart_application')
    } catch (cause) {
      toast.error(tr('Could not restart RambleDesk'), { description: messageFrom(cause) })
    }
  }

  async function restartForPermissions() {
    setOnboardingStep(step)
    try {
      await invoke('restart_application')
    } catch (cause) {
      toast.error(tr('Could not restart RambleDesk'), { description: messageFrom(cause) })
    }
  }

  async function loadModels() {
    try {
      models = await invoke<SpeechModel[]>('list_speech_models')
    } catch (cause) {
      toast.error(tr('Could not read voice models'), { description: messageFrom(cause) })
    }
  }

  async function downloadModel() {
    if (!selectedModel || selectedModel.installed || modelBusy) return
    modelBusy = true
    modelProgress = { model_id: selectedModel.id, downloaded: 0, total: selectedModel.size_bytes }
    try {
      await invoke<SpeechModel>('download_speech_model', { modelId: selectedModel.id })
      await loadModels()
      toast.success(tr('Voice model installed'))
    } catch (cause) {
      toast.error(tr('Voice model download failed'), { description: messageFrom(cause) })
    } finally {
      modelBusy = false
    }
  }

  async function loadHosts() {
    hostsLoading = true
    try {
      hosts = await invoke<McpHost[]>('detect_generic_mcp_hosts')
      hostSelections = new Set(hosts.filter((host) => host.installed && !host.configured).map((host) => host.id))
    } catch (cause) {
      toast.error(tr('Could not detect adapter hosts'), { description: messageFrom(cause) })
    } finally {
      hostsLoading = false
    }
  }

  function toggleHost(id: string) {
    const next = new Set(hostSelections)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    hostSelections = next
  }

  async function installSelectedHosts() {
    if (!selectedHosts.length || adapterBusy) return
    adapterBusy = true
    try {
      const results = await invoke<McpInstallResult[]>('install_generic_mcp_hosts', { hostIds: selectedHosts })
      const changed = results.filter((result) => result.action !== 'unchanged').length
      toast.success(tr('Adapters configured'), { description: tr('Restart {count} host(s) before using them.', { count: changed }) })
      await loadHosts()
    } catch (cause) {
      toast.error(tr('Adapter installation failed'), { description: messageFrom(cause) })
    } finally {
      adapterBusy = false
    }
  }

  async function loadPiStatus() {
    piStatusLoading = true
    try {
      piStatus = await invoke<PiPackageStatus>('get_pi_package_status', { checkoutRoot: null })
    } catch {
      piStatus = null
    } finally {
      piStatusLoading = false
    }
  }

  async function installPi() {
    if (piBusy) return
    piBusy = true
    try {
      await invoke<string>('install_pi_package', { checkoutRoot: null })
      await loadPiStatus()
      toast.success(tr('Pi native adapter installed'))
    } catch (cause) {
      toast.error(tr('Pi adapter installation failed'), { description: messageFrom(cause) })
    } finally {
      piBusy = false
    }
  }

  async function installDsh() {
    if (dshBusy) return
    dshBusy = true
    try {
      await invoke('install_dsh_package', { checkoutRoot: null, profileId: null })
      toast.success(tr('DSH native adapter installed'))
    } catch (cause) {
      toast.error(tr('DSH adapter installation failed'), { description: messageFrom(cause) })
    } finally {
      dshBusy = false
    }
  }

  $: starterPrompt = t($locale, '/ramble Let\'s work on something together')

  async function copyStarterPrompt() {
    try {
      await navigator.clipboard.writeText(starterPrompt)
      promptCopyState = 'copied'
      window.setTimeout(() => {
        if (promptCopyState === 'copied') promptCopyState = 'idle'
      }, 2_000)
    } catch {
      promptCopyState = 'error'
    }
  }

  async function enableNotifications() {
    if (!isTauri || notificationBusy || isWindows) return
    notificationBusy = true
    try {
      const permission = (await isPermissionGranted()) ? 'granted' : await requestPermission()
      if (permission !== 'granted') throw new Error(tr('The operating system did not grant notification permission.'))
      setNotificationPopupEnabled(true)
      toast.success(tr('System notifications enabled'))
    } catch (cause) {
      setNotificationPopupEnabled(false)
      toast.error(tr('Could not enable system notifications'), { description: messageFrom(cause) })
    } finally {
      notificationBusy = false
    }
  }

  function chooseCookingProvider(provider: CookingProvider) {
    setCookingProvider(provider)
    if (provider === 'deepseek') {
      setCookingBaseUrl('https://api.deepseek.com/v1')
      setCookingModel('deepseek-v4-flash')
    } else if (provider === 'openai') {
      setCookingBaseUrl('https://api.openai.com/v1')
      setCookingModel('gpt-4.1-mini')
    }
  }
</script>

<Dialog.Root bind:open={openWizard}>
  <Dialog.Content
    showCloseButton={false}
    interactOutsideBehavior="ignore"
    escapeKeydownBehavior="ignore"
    class="flex max-h-[calc(100vh-2rem)] w-[min(760px,calc(100vw-2rem))] max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none"
    aria-describedby="onboarding-description"
  >
    <Dialog.Header class="shrink-0 border-b bg-muted/25 px-7 py-6">
      <div class="flex items-center gap-3">
        <span class="grid size-10 place-items-center rounded-xl bg-primary text-primary-foreground">
          <Sparkles class="size-5" />
        </span>
        <div>
          <Dialog.Title>{tr('Welcome to RambleDesk')}</Dialog.Title>
          <Dialog.Description id="onboarding-description" class="mt-1 text-xs">
            {tr('Finish common setup in a few steps. Every option can be changed later in Settings.')}
          </Dialog.Description>
        </div>
      </div>
      <div class="mt-5 flex gap-1.5" aria-label={tr('Setup progress')}>
        {#each steps as label, index}
          <span
            class={[
              'h-1 flex-1 rounded-full transition-colors',
              index <= step ? 'bg-primary' : 'bg-muted',
            ]}
            title={`${index + 1}. ${tr(label)}`}
          ></span>
        {/each}
      </div>
      <p class="m-0 mt-2 text-[10px] text-muted-foreground">{step + 1} / {steps.length} · {tr(steps[step])}</p>
    </Dialog.Header>

    <div class="min-h-0 flex-1 overflow-y-auto px-7 py-7">
      {#if steps[step] === 'Welcome'}
        <div class="mx-auto flex max-w-lg flex-col items-center text-center">
          <span class="grid size-16 place-items-center rounded-2xl bg-primary/10 text-primary"><Rocket class="size-8" /></span>
          <h2 class="mb-0 mt-5 text-xl font-semibold">{tr('Turn thoughts from an experience into useful feedback while they are fresh.')}</h2>
          <p class="mb-0 mt-3 text-sm leading-6 text-muted-foreground">
            {tr('RambleDesk captures text, voice, and screenshot feedback for your coding tools. Cooking can turn a raw Ramble into formal feedback.')}
          </p>
          <div class="mt-5 flex items-center gap-2 text-xs">
            <span class="text-muted-foreground">{tr('Interface language')}</span>
            <Button size="sm" variant={$locale === 'zh-CN' ? 'default' : 'outline'} onclick={() => setLocale('zh-CN')}>简体中文</Button>
            <Button size="sm" variant={$locale === 'en' ? 'default' : 'outline'} onclick={() => setLocale('en')}>English</Button>
          </div>
          <div class="mt-5 grid w-full grid-cols-3 gap-3 text-left text-xs">
            <div class="rounded-lg border bg-muted/20 p-3"><Mic class="mb-2 size-4 text-primary" />{tr('Local voice transcription')}</div>
            <div class="rounded-lg border bg-muted/20 p-3"><PlugZap class="mb-2 size-4 text-primary" />{tr('Coding-tool adapters')}</div>
            <div class="rounded-lg border bg-muted/20 p-3"><ChefHat class="mb-2 size-4 text-primary" />{tr('Optional AI Cooking')}</div>
          </div>
        </div>
      {:else if steps[step] === 'Storage'}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><HardDrive class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('Choose where data lives first')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('Feedback attachments, published packages, and voice models live in this folder. Set it first so later downloads and feedback go to the right place.')}</p></div></div>
          <div class="mt-6 rounded-lg border bg-muted/20 p-4"><p class="m-0 text-[10px] font-medium uppercase text-muted-foreground">{tr('Current folder')}</p><p class="mb-0 mt-2 break-all font-mono text-xs">{storage?.selected_path ?? tr('Loading data storage location…')}</p></div>
          <div class="mt-4 flex items-center justify-between gap-4"><p class="m-0 text-xs text-muted-foreground">{tr('The database and local credentials remain in the system app directory.')}</p><Button variant="outline" disabled={!isTauri || storageBusy} onclick={() => void chooseStorage()}>{#if storageBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<FolderCog data-icon="inline-start" />{/if}{tr('Choose another location…')}</Button></div>
          {#if storageRestartRequired}<div class="mt-5 rounded-lg border border-primary/30 bg-primary/5 p-4 text-xs leading-5 text-primary">{tr('The data location has been saved. Restart RambleDesk so the remaining setup uses it directly.')}</div>{/if}
        </section>
      {:else if steps[step] === 'Voice input'}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><Mic class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('Ramble quickly with voice')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('SenseVoice is the recommended default for reliable multilingual transcription. Audio stays on this device and is never uploaded. You can skip this and choose another model, microphone, or VAD settings later.')}</p></div></div>
          <div class="mt-6 rounded-lg border bg-muted/20 p-4">
            <label for="onboarding-model" class="text-xs font-medium">{tr('Transcription model')}</label>
            <select id="onboarding-model" class="mt-2 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$speechModelId} onchange={(event) => setSpeechModelId((event.currentTarget as HTMLSelectElement).value as SpeechModelId)}>
              {#each models as model (model.id)}<option value={model.id}>{speechModelDisplayName($locale, model.id, model.display_name)}{model.id === DEFAULT_SPEECH_MODEL_ID ? ` · ${tr('Recommended')}` : ''}{model.installed ? ` · ${tr('Installed')}` : ''}</option>{/each}
            </select>
            {#if selectedModel}<div class="mt-4 flex items-start justify-between gap-4"><div><div class="flex flex-wrap gap-2">{#if selectedModel.id === DEFAULT_SPEECH_MODEL_ID}<Badge variant="secondary">{tr('Recommended')}</Badge>{/if}<Badge variant={selectedModel.installed ? 'secondary' : 'outline'}>{selectedModel.installed ? tr('Installed') : mb(selectedModel.size_bytes)}</Badge><Badge variant="outline">{selectedModel.streaming ? tr('Live streaming') : tr('VAD segmented')}</Badge></div><p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{speechModelDescription($locale, selectedModel.id, selectedModel.description)}</p></div>{#if !selectedModel.installed}<Button disabled={modelBusy} onclick={() => void downloadModel()}>{#if modelBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<Download data-icon="inline-start" />{/if}{modelBusy ? `${modelProgressPercent}%` : selectedModel.id === DEFAULT_SPEECH_MODEL_ID ? tr('Download recommended model') : tr('Download model')}</Button>{/if}</div>{/if}
            {#if modelBusy}<div class="mt-4 h-1.5 overflow-hidden rounded bg-muted"><div class="h-full bg-primary transition-[width]" style={`width: ${modelProgressPercent}%`}></div></div>{/if}
          </div>
        </section>
        {:else if steps[step] === 'Permissions'}
          <section class="mx-auto max-w-xl">
            <div class="flex gap-3"><ShieldCheck class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('Grant Mac permissions')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('Screen capture and voice transcription require macOS permissions. Grant them now or later in Settings → Permissions.')}</p></div></div>
            <div class="mt-6">
              <MacPermissions bind:restartRequired={permissionRestartRequired} />
            </div>
          </section>
      {:else if steps[step] === 'Adapters'}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3">
            <PlugZap class="mt-0.5 size-6 text-primary" />
            <div>
              <h2 class="m-0 text-lg font-semibold">{tr('Connect your coding tools')}</h2>
              <p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('We recommend the Pi and DeepSeek Harness native adapters. You can also configure generic MCP hosts as needed.')}</p>
            </div>
          </div>

          <div class="mt-6 space-y-3">
            <div class="flex items-start gap-3 rounded-lg border bg-muted/20 p-3">
              <span class="grid size-8 shrink-0 place-items-center rounded-lg bg-muted text-muted-foreground [&_svg]:size-5">
                {@html piLogoSvg}
              </span>
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <h3 class="m-0 text-sm font-semibold">{tr('Pi native adapter')}</h3>
                  <Badge variant="secondary">{tr('Recommended')}</Badge>
                </div>
                <p class="mb-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('Pi waits for feedback in the same tool call, then automatically continues the current session. No copied or manually sent resume prompt is needed.')}</p>
              </div>
              <Button
                size="sm"
                class="shrink-0"
                disabled={piBusy || piStatusLoading || piStatus?.installed || !isTauri || piStatus?.cliAvailable === false}
                onclick={() => void installPi()}
              >
                {#if piBusy}
                  <LoaderCircle class="animate-spin" data-icon="inline-start" />
                  {tr('Installing…')}
                {:else if piStatusLoading}
                  <LoaderCircle class="animate-spin" data-icon="inline-start" />
                  {tr('Checking…')}
                {:else if piStatus?.installed}
                  <Check data-icon="inline-start" />
                  {tr('Installed')}
                {:else if piStatus && !piStatus.cliAvailable}
                  {tr('Pi CLI not detected')}
                {:else}
                  <Download data-icon="inline-start" />
                  {tr('Install Pi adapter')}
                {/if}
              </Button>
            </div>
            <div class="flex items-start gap-3 rounded-lg border bg-muted/20 p-3">
              <span class="grid size-8 shrink-0 place-items-center rounded-lg bg-muted text-muted-foreground [&_svg]:size-5">
                {@html dshLogoSvg}
              </span>
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <h3 class="m-0 text-sm font-semibold">{tr('DSH native adapter')}</h3>
                  <Badge variant="secondary">{tr('Native wait')}</Badge>
                </div>
                <p class="mb-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('DeepSeek Harness waits for feedback in the same tool call, then automatically continues.')}</p>
              </div>
              <Button size="sm" class="shrink-0" disabled={dshBusy || !isTauri} onclick={() => void installDsh()}>
                {#if dshBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<Download data-icon="inline-start" />{/if}
                {dshBusy ? tr('Installing…') : tr('Install DSH adapter')}
              </Button>
            </div>

          <details class="group rounded-lg border bg-muted/20">
            <summary class="flex cursor-pointer list-none items-center gap-3 p-3 text-xs outline-none [&::-webkit-details-marker]:hidden">
              <span class="grid size-8 shrink-0 place-items-center rounded-lg bg-muted">
                <PlugZap class="size-4 text-muted-foreground" />
              </span>
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="text-sm font-semibold">{tr('Generic MCP hosts')}</span>
                  <Badge variant="outline" title={tr('After submitting or cancelling, return to the host and continue the session manually.')}>{tr('Manual continuation')}</Badge>
                </div>
                <p class="mb-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('After submitting or cancelling, MCP hosts require you to return to the coding tool and continue with its resume prompt. RambleDesk only writes its own MCP entry and never overwrites other servers.')}</p>
              </div>
              <span class="ml-auto shrink-0 text-[10px] text-muted-foreground">{tr('Configure as needed')}</span>
              <ChevronDown class="size-4 shrink-0 text-muted-foreground transition-transform group-open:rotate-180" />
            </summary>
            <div class="border-t p-4">
              {#if hostsLoading}
                <p class="mb-0 mt-4 flex items-center gap-2 text-xs text-muted-foreground"><LoaderCircle class="size-4 animate-spin" />{tr('Detecting coding tools…')}</p>
              {:else if hosts.length === 0}
                <p class="mb-0 mt-4 text-xs leading-5 text-muted-foreground">{tr('No supported tools were detected. You can skip this step and install adapters later in Settings → Adapters.')}</p>
              {:else}
                <div class="mt-4 space-y-1">
                  {#each hosts as host (host.id)}
                    <label class={['flex items-center gap-3 rounded-md px-2 py-2 text-xs', host.installed ? 'cursor-pointer hover:bg-muted' : 'cursor-not-allowed opacity-55']}>
                      <input type="checkbox" class="size-3.5 accent-primary" checked={hostSelections.has(host.id)} disabled={!host.installed || adapterBusy} onchange={() => toggleHost(host.id)} />
                      <span class="flex-1 font-medium">{host.name}</span>
                      <Badge variant={host.configured ? 'secondary' : 'outline'}>{host.configured ? tr('Configured') : host.installed ? tr('Detected') : tr('Not detected')}</Badge>
                    </label>
                  {/each}
                </div>
                <Button class="mt-4" disabled={adapterBusy || selectedHosts.length === 0} onclick={() => void installSelectedHosts()}>
                  {#if adapterBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<PlugZap data-icon="inline-start" />{/if}
                  {tr('Install selected MCP adapters')}
                </Button>
              {/if}
            </div>
          </details>
          </div>
        </section>
      {:else if steps[step] === 'Notifications'}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><BellRing class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('Would you like notifications?')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{#if isWindows}{tr('Current unsigned Windows builds cannot show system banners. RambleDesk will not try to send them. Watch the inbox badge and use sound alerts instead.')}{:else}{tr('When a coding tool requests feedback, RambleDesk can show a system notification and play a sound.')}{/if}</p></div></div>
          <div class="mt-6 space-y-3 rounded-lg border bg-muted/20 p-4">
            <div class="flex items-center justify-between gap-4">
              <div><strong class="text-xs">{tr('System notifications')}</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{#if isWindows}{tr('System banners are not available on this Windows build.')}{:else}{tr('Show new feedback requests in the system notification center.')}{/if}</p></div>
              {#if isWindows}<Badge variant="secondary">{tr('Unavailable')}</Badge>{:else if $notificationPopupEnabled}<Badge variant="secondary">{tr('Enabled')}</Badge>{:else}<Button size="sm" disabled={notificationBusy || !isTauri} onclick={() => void enableNotifications()}>{#if notificationBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{/if}{tr('Allow notifications')}</Button>{/if}
            </div>
            <div class="flex items-center justify-between gap-4 border-t pt-3">
              <div><strong class="text-xs">{tr('Sound alerts')}</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{tr('Play a sound when a notification arrives.')}</p></div>
              <button type="button" role="switch" aria-label={tr('Sound alerts')} aria-checked={$notificationSoundEnabled} class={['relative h-[22px] w-10 rounded-full transition-colors', $notificationSoundEnabled ? 'bg-primary' : 'bg-input']} onclick={() => setNotificationSoundEnabled(!$notificationSoundEnabled)}><span class={['absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow transition-transform', $notificationSoundEnabled ? 'translate-x-5' : '']}></span></button>
            </div>
            <p class="m-0 border-t pt-3 text-[10px] leading-4 text-muted-foreground">{tr('Sound, volume, and other advanced notification options can be adjusted anytime in Settings → Notifications.')}</p>
          </div>
        </section>
      {:else if steps[step] === 'Cooking'}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><ChefHat class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('Enable Feedback Cooking?')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('Optional: use your own model service to turn a raw Ramble into formal feedback before submitting. The uncooked source is always preserved.')}</p></div></div>
          <div class="mt-6 rounded-lg border bg-muted/20 p-4"><div class="flex items-center justify-between gap-4"><div><strong class="text-xs">{tr('Full Cook')}</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{tr('An API key is required and the feedback body is sent to your selected service.')}</p></div><button type="button" role="switch" aria-label={tr('Full Cook')} aria-checked={$cookingEnabled} class={['relative h-[22px] w-10 rounded-full transition-colors', $cookingEnabled ? 'bg-primary' : 'bg-input']} onclick={() => setCookingEnabled(!$cookingEnabled)}><span class={['absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow transition-transform', $cookingEnabled ? 'translate-x-5' : '']}></span></button></div>
            {#if $cookingEnabled}<div class="mt-5 grid gap-3 border-t pt-4"><label class="text-xs font-medium">{tr('Model provider')}<select class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingProvider} onchange={(event) => chooseCookingProvider((event.currentTarget as HTMLSelectElement).value as CookingProvider)}><option value="deepseek">DeepSeek</option><option value="openai">OpenAI</option><option value="compatible">{tr('OpenAI-compatible service')}</option></select></label><label class="text-xs font-medium">Base URL<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" type="url" value={$cookingBaseUrl} oninput={(event) => setCookingBaseUrl((event.currentTarget as HTMLInputElement).value)} /></label><div class="grid grid-cols-2 gap-3"><label class="text-xs font-medium">{tr('Model name')}<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingModel} oninput={(event) => setCookingModel((event.currentTarget as HTMLInputElement).value)} /></label><label class="text-xs font-medium">{tr('Reasoning effort')}<select class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingReasoningEffort} onchange={(event) => setCookingReasoningEffort((event.currentTarget as HTMLSelectElement).value as CookingReasoningEffort)}><option value="none">{tr('None')}</option><option value="minimal">minimal</option><option value="low">low</option><option value="medium">medium</option><option value="high">high</option><option value="xhigh">xhigh</option><option value="max">max</option></select></label></div><label class="text-xs font-medium">API Key<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" type="password" autocomplete="off" value={$cookingApiKey} placeholder="sk-…" oninput={(event) => setCookingApiKey((event.currentTarget as HTMLInputElement).value)} /></label></div>{/if}
          </div>
        </section>
      {:else}
        <div class="mx-auto flex max-w-lg flex-col items-center pt-8 text-center">
          <span class="grid size-16 place-items-center rounded-2xl bg-success/10 text-success"><Check class="size-8" /></span>
          <h2 class="mb-0 mt-5 text-xl font-semibold">{tr('You are all set')}</h2>
          <p class="mb-0 mt-3 text-sm leading-6 text-muted-foreground">{tr('You can now start a Ramble directly from a coding tool. Every setting can be changed from the top-right Settings button.')}</p>
          <div class="mt-6 w-full rounded-lg border bg-muted/20 p-4 text-left">
            <div class="flex items-center justify-between gap-2">
              <p class="m-0 text-[10px] font-medium text-muted-foreground">{tr('Paste this example prompt into your coding agent or coding tool:')}</p>
              <Button variant="outline" size="sm" class="h-7 px-2 text-[10px]" onclick={() => void copyStarterPrompt()}>
                <Clipboard data-icon="inline-start" />
                {promptCopyState === 'copied' ? tr('Copied') : promptCopyState === 'error' ? tr('Copy failed') : tr('Copy')}
              </Button>
            </div>
            <code class="mt-2 block rounded-md bg-background px-3 py-2 text-left text-xs text-foreground">{starterPrompt}</code>
          </div>
          <div class="mt-3 flex items-center gap-2 rounded-lg border bg-muted/20 px-4 py-3 text-xs text-muted-foreground"><Volume2 class="size-4 text-primary" />{tr('Tip: the microphone is ready once a voice model finishes downloading.')}</div>
        </div>
      {/if}
    </div>

    <footer class="flex shrink-0 items-center justify-between border-t bg-muted/15 px-7 py-4">
      <Button variant="ghost" size="sm" onclick={() => complete(false)}>{tr('Set up later')}</Button>
      <div class="flex items-center gap-2">
        {#if step > 0 && !storageRestartRequired && !permissionRestartRequired}<Button variant="outline" size="sm" onclick={() => move(step - 1)}><ChevronLeft data-icon="inline-start" />{tr('Back')}</Button>{/if}
        {#if storageRestartRequired}<Button disabled={storageBusy} onclick={() => void restartForStorage()}><Rocket data-icon="inline-start" />{tr('Restart and continue')}</Button>
        {:else if permissionRestartRequired}<Button onclick={() => void restartForPermissions()}><Rocket data-icon="inline-start" />{tr('Restart and continue')}</Button>
        {:else if step === steps.length - 1}<Button onclick={() => complete()}><Check data-icon="inline-start" />{tr('Start using RambleDesk')}</Button>
        {:else}<Button disabled={storageBusy || modelBusy} onclick={() => move(step + 1)}>{steps[step] === 'Voice input' && !selectedModel?.installed ? tr('Skip voice setup') : tr('Continue')}<ChevronRight data-icon="inline-end" /></Button>{/if}
      </div>
    </footer>
  </Dialog.Content>
</Dialog.Root>
