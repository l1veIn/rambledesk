<script lang="ts">
  import { onMount } from 'svelte'
  import { BellRing, Check, ChefHat, ChevronLeft, ChevronRight, Download, FolderCog, HardDrive, LoaderCircle, MessageSquare, Mic, Rocket, ShieldCheck, Sparkles } from '@lucide/svelte'
  import MacPermissions from '$lib/settings/MacPermissions.svelte'
  import AgentCatalog from '$lib/agents/AgentCatalog.svelte'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { AgentConfig } from '$lib/generated/feedback'
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import type { SpeechModelInfo, SpeechModelProgress, WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import { resolveSupportedSpeechModelId } from '$lib/capabilities/speechModelSelection'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { t } from '$lib/i18n'
  import { diagnosticErrorCategory, recordClientDiagnostic, startClientDiagnostic } from '$lib/diagnostics/clientDiagnostics'
  import { speechModelDescription, speechModelDisplayName } from '$lib/speech/speechModelLabels'
  import { onboardingSteps, onboardingRestartStep, reconcileOnboardingStep, resumeOnboardingStep, type OnboardingStep } from '$lib/onboarding/onboardingSteps'
  import {
    DEFAULT_SPEECH_MODEL_ID, cookingApiKey, cookingBaseUrl, cookingEnabled, cookingModel, cookingProvider, cookingReasoningEffort,
    finishOnboarding, locale, notificationPopupEnabled, notificationSoundEnabled, onboardingStep,
    setCookingApiKey, setCookingBaseUrl, setCookingEnabled, setCookingModel, setCookingProvider, setCookingReasoningEffort,
    setLocale, setNotificationPopupEnabled, setNotificationSoundEnabled, setOnboardingStep, setSpeechModelId, speechModelId,
    type CookingProvider, type CookingReasoningEffort, type SpeechModelId,
  } from '$lib/preferences'

  export let openWizard = false
  export let onClose: () => void = () => {}
  export let onStartSession: (configId?: string) => Promise<void> = async () => {}
  export let transport: ApplicationTransport
  export let capabilities: WorkbenchCapabilities = createUnavailableWorkbenchCapabilities()
  const storageAvailable = capabilities.dataStorageAdministration.status.availability !== 'unavailable'
    && capabilities.serverPaths.status.availability !== 'unavailable'
  const voiceAvailable = capabilities.speech.status.availability !== 'unavailable'
  const platform = capabilities.windowControls.implementation.platform()
  const permissionsAvailable = platform === 'macOS' && capabilities.systemPermissions.status.availability !== 'unavailable'
  const notificationsAvailable = capabilities.notifications.status.availability !== 'unavailable'
  const windowControlsAvailable = capabilities.windowControls.status.availability !== 'unavailable'
  const isWindows = platform === 'Windows'
  // Include the potential Mac step before restoring numeric progress; the query
  // below can remove it while preserving the current step by its stable name.
  let steps = onboardingSteps({ storage: storageAvailable, voice: voiceAvailable, permissions: permissionsAvailable, notifications: notificationsAvailable })
  let step = resumeOnboardingStep(steps, onboardingStep())
  let selectedConfig: AgentConfig | null = null
  let visitedAgents = false
  let detectOnEntry = false
  let storage: { selected_path: string; restart_required: boolean } | null = null
  let storageBusy = false
  let restarting = false
  let permissionRestartRequired = false
  let models: SpeechModelInfo[] = []
  let modelsLoading = false
  let modelBusy = false
  let modelProgress: SpeechModelProgress | null = null
  let notificationBusy = false
  let mounted = false
  let starting = false
  let error = ''
  let finishFlow = startClientDiagnostic('onboarding', { source: 'onboarding', platform, resumed: step > 0, step_count: steps.length,
    storage_available: storageAvailable, voice_available: voiceAvailable, permissions_available: permissionsAvailable, notifications_available: notificationsAvailable })
  let finishStep: ReturnType<typeof startClientDiagnostic> | undefined
  let recordedStep: OnboardingStep | undefined
  $: if (mounted) observeStep(steps[step])
  $: isFinalStep = step === steps.length - 1
  $: selectedModel = models.find(model => model.id === $speechModelId) ?? models[0]
  $: modelProgressPercent = modelProgress ? Math.min(100, Math.round(modelProgress.downloaded / Math.max(1, modelProgress.total) * 100)) : 0
  $: if (steps[step] === 'agents' && !visitedAgents) { visitedAgents = true; detectOnEntry = true }
  $: if (steps[step] !== 'agents') detectOnEntry = false
  function localizedText(language: string) {
    return (zh: string, en: string) => language === 'zh-CN' ? zh : en
  }
  $: tr = localizedText($locale)
  $: translate = t.bind(null, $locale)
  function stepLabel(current: OnboardingStep, language: string) {
    const labels = { welcome: ['欢迎', 'Welcome'], storage: ['数据位置', 'Data storage'], voice: ['语音输入', 'Voice input'],
      permissions: ['系统权限', 'Permissions'], agents: ['连接智能体', 'Connect an agent'], notifications: ['通知', 'Notifications'],
      cooking: ['Cooking', 'Cooking'], finish: ['新建会话', 'New session'] }
    return labels[current][language === 'zh-CN' ? 0 : 1]
  }
  function mb(bytes: number) { return `${Math.round(bytes / 1024 / 1024)} MB` }
  function message(cause: unknown) { return cause instanceof Error ? cause.message : String(cause) }
  function observeStep(current: OnboardingStep) {
    if (recordedStep === current) return
    finishStep?.('ok', { reason: 'navigation' })
    recordedStep = current
    finishStep = startClientDiagnostic('onboarding_step', { source: 'onboarding', step: current, step_index: step, step_count: steps.length })
  }
  function actionDiagnostic(action: string, phase: string) {
    return startClientDiagnostic('onboarding_action', { source: 'onboarding', action, phase, step: steps[step] })
  }
  function move(next: number) {
    if (starting || storageBusy || modelBusy || storage?.restart_required || permissionRestartRequired) return
    recordClientDiagnostic({ activity: 'onboarding_action', outcome: 'ok', details: { source: 'onboarding', step: steps[step], next_step: steps[Math.max(0, Math.min(steps.length - 1, next))],
      action: next < step ? 'back' : 'next', selected: !!selectedConfig, installed: !!selectedModel?.installed } })
    step = Math.max(0, Math.min(steps.length - 1, next)); setOnboardingStep(step); error = ''
  }
  onMount(() => {
    mounted = true
    if (storageAvailable) void loadStorage()
    let unsubscribeProgress: (() => void) | undefined
    if (voiceAvailable) {
      void loadModels()
      unsubscribeProgress = capabilities.speech.implementation.onModelProgress(progress => {
        if (mounted && modelProgress?.model_id === progress.model_id) modelProgress = { ...progress }
      }, cause => { if (mounted) error = message(cause) })
    }
    if (permissionsAvailable) void loadPermissions()
    return () => { mounted = false; unsubscribeProgress?.(); finishStep?.('cancelled', { reason: 'unmounted' }); finishFlow('cancelled', { reason: 'unmounted' }) }
  })
  async function loadPermissions() {
    const finish = actionDiagnostic('probe', 'permissions')
    let includePermissions = false
    try { const permissions = await capabilities.systemPermissions.implementation.list(); includePermissions = permissions.length > 0; finish('ok', { permission_count: permissions.length, available: includePermissions }) }
    catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }) /* Unsupported APIs do not block onboarding. */ }
    if (!mounted || restarting) return
    const next = onboardingSteps({ storage: storageAvailable, voice: voiceAvailable, permissions: includePermissions, notifications: notificationsAvailable })
    step = reconcileOnboardingStep(steps, next, step)
    steps = next
    setOnboardingStep(step)
  }
  async function loadModels() {
    const finish = actionDiagnostic('refresh', 'model')
    modelsLoading = true
    try {
      const listed = await capabilities.speech.implementation.listModels()
      finish('ok', { model_count: listed.length })
      if (!mounted) return
      models = [...listed]
      const supported = resolveSupportedSpeechModelId($speechModelId, models)
      if (supported && supported !== $speechModelId) setSpeechModelId(supported)
    } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); if (mounted) error = message(cause) }
    finally { if (mounted) modelsLoading = false }
  }
  async function downloadModel() {
    if (!selectedModel || selectedModel.installed || modelBusy) return
    const model = selectedModel
    const finish = actionDiagnostic('download', 'model')
    modelBusy = true; error = ''
    modelProgress = { model_id: model.id, downloaded: 0, total: model.size_bytes }
    try { await capabilities.speech.implementation.downloadModel(model.id); finish('ok', { installed: true }); if (mounted) await loadModels() }
    catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); if (mounted) error = message(cause) }
    finally { if (mounted) modelBusy = false }
  }
  async function enableNotifications() {
    if (!notificationsAvailable || notificationBusy || isWindows) return
    const finish = actionDiagnostic('permission', 'notifications')
    notificationBusy = true; error = ''
    try {
      const current = await capabilities.notifications.implementation.permission()
      const permission = current === 'granted' ? current : await capabilities.notifications.implementation.requestPermission()
      if (permission !== 'granted') { finish('failed', { reason: 'permission_denied' }); throw new Error(translate('The operating system did not grant notification permission.')) }
      setNotificationPopupEnabled(true)
      finish('ok', { enabled: true })
    } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); setNotificationPopupEnabled(false); if (mounted) error = message(cause) }
    finally { if (mounted) notificationBusy = false }
  }
  function chooseCookingProvider(provider: CookingProvider) {
    recordClientDiagnostic({ activity: 'onboarding_action', outcome: 'ok', details: { source: 'onboarding', step: 'cooking', action: 'configure' } })
    setCookingProvider(provider)
    if (provider === 'deepseek') { setCookingBaseUrl('https://api.deepseek.com/v1'); setCookingModel('deepseek-v4-flash') }
    else if (provider === 'openai') { setCookingBaseUrl('https://api.openai.com/v1'); setCookingModel('gpt-4.1-mini') }
  }
  function toggleCooking() {
    const enabled = !$cookingEnabled
    recordClientDiagnostic({ activity: 'onboarding_action', outcome: 'ok', details: { source: 'onboarding', step: 'cooking', action: 'configure', enabled } })
    setCookingEnabled(enabled)
  }
  function toggleSound() {
    const enabled = !$notificationSoundEnabled
    recordClientDiagnostic({ activity: 'onboarding_action', outcome: 'ok', details: { source: 'onboarding', step: 'notifications', action: 'configure', enabled } })
    setNotificationSoundEnabled(enabled)
  }
  async function loadStorage() {
    const finish = actionDiagnostic('refresh', 'storage')
    try { storage = await capabilities.dataStorageAdministration.implementation.read(); finish('ok', { restart_required: storage.restart_required }) }
    catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); error = message(cause) }
  }
  async function chooseStorage() {
    if (storageBusy || starting) return
    const finish = actionDiagnostic('select', 'storage')
    storageBusy = true; error = ''
    try {
      const path = await capabilities.serverPaths.implementation.chooseDirectory()
      if (path) storage = await capabilities.dataStorageAdministration.implementation.select(path)
      finish(path ? 'ok' : 'cancelled', { restart_required: !!storage?.restart_required })
    } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); error = message(cause) }
    finally { storageBusy = false }
  }
  async function restart(reason: 'storage' | 'permissions') {
    if (!windowControlsAvailable) return
    const finish = startClientDiagnostic('onboarding_action', { source: 'onboarding', action: 'restart', phase: reason, step: steps[step], next_step: steps[onboardingRestartStep(steps, reason)] })
    restarting = true; storageBusy = true; error = ''; setOnboardingStep(onboardingRestartStep(steps, reason))
    try { await capabilities.windowControls.implementation.restart(); finish('ok') }
    catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); error = message(cause); storageBusy = false; restarting = false }
  }
  async function complete() {
    if (starting || storageBusy || modelBusy || storage?.restart_required || permissionRestartRequired) return
    const finish = actionDiagnostic('open', 'completion')
    starting = true; error = ''
    try {
      await onStartSession(selectedConfig?.id)
      finishOnboarding(); openWizard = false; onClose()
      finish('ok', { selected: !!selectedConfig })
      finishStep?.(isFinalStep ? 'ok' : 'skipped', { reason: 'completed' })
      finishFlow(isFinalStep ? 'ok' : 'skipped', { step: steps[step], selected: !!selectedConfig })
    } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); error = message(cause); starting = false }
  }
</script>

<Dialog.Root bind:open={openWizard}>
  <Dialog.Content showCloseButton={false} interactOutsideBehavior="ignore" escapeKeydownBehavior="ignore"
    class="flex max-h-[calc(100vh-2rem)] w-[min(1020px,calc(100vw-2rem))] max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none">
    <Dialog.Header class="shrink-0 border-b bg-muted/25 px-7 py-5">
      <div class="flex items-center gap-3">
        <span class="grid size-10 place-items-center rounded-xl bg-primary text-primary-foreground"><Sparkles class="size-5" /></span>
        <div><Dialog.Title>{tr('开始使用 RambleDesk', 'Get started with RambleDesk')}</Dialog.Title>
          <Dialog.Description class="mt-1 text-xs">{tr('连接设备上的智能体，在这里开始会话、交流和反馈。', 'Connect an agent on your device, then start a conversation and share feedback here.')}</Dialog.Description></div>
      </div>
      <div class="mt-4 flex gap-1.5" aria-label={tr('设置进度', 'Setup progress')}>{#each steps as _, index}<span class={['h-1 flex-1 rounded-full', index <= step ? 'bg-primary' : 'bg-muted']}></span>{/each}</div>
      <p class="m-0 mt-2 text-[10px] text-muted-foreground">{step + 1} / {steps.length} · {stepLabel(steps[step], $locale)}</p>
    </Dialog.Header>
    <div class="min-h-0 flex-1 overflow-y-auto px-7 py-6">
      {#if steps[step] === 'welcome'}
        <section class="mx-auto flex max-w-xl flex-col items-center py-7 text-center">
          <span class="grid size-16 place-items-center rounded-2xl bg-primary/10 text-primary"><MessageSquare class="size-8" /></span>
          <h2 class="mb-0 mt-5 text-xl font-semibold">{tr('从一个会话开始', 'Start with a conversation')}</h2>
          <p class="mb-0 mt-3 text-sm leading-6 text-muted-foreground">{tr('RambleDesk 通过 ACP 连接你设备上的智能体。选择项目、描述任务，再用文字、语音或截图提供反馈。', 'RambleDesk connects to agents on your device through ACP. Choose a project, describe your task, and provide feedback with text, voice, or screenshots.')}</p>
          <ol class="mt-6 grid w-full gap-3 text-left text-sm sm:grid-cols-3"><li class="rounded-lg border p-4">1. {tr('检测与连接智能体', 'Find and connect an agent')}</li><li class="rounded-lg border p-4">2. {tr('选择项目目录', 'Choose a project folder')}</li><li class="rounded-lg border p-4">3. {tr('开始会话', 'Start a conversation')}</li></ol>
          <p class="mb-0 mt-4 text-xs leading-5 text-muted-foreground">{tr('引导会依次设置数据位置、语音、系统权限、智能体连接、通知和可选的 Cooking。所有选项之后都可在设置中调整。', 'This guide covers data storage, voice, system permissions, agent connections, notifications, and optional Cooking. Every option can be changed later in Settings.')}</p>
          <div class="mt-6 flex items-center gap-2"><span class="text-xs text-muted-foreground">{tr('界面语言', 'Interface language')}</span><Button size="sm" variant={$locale === 'zh-CN' ? 'default' : 'outline'} onclick={() => setLocale('zh-CN')}>简体中文</Button><Button size="sm" variant={$locale === 'en' ? 'default' : 'outline'} onclick={() => setLocale('en')}>English</Button></div>
        </section>
      {:else if steps[step] === 'storage'}
        <section class="mx-auto max-w-xl py-5">
          <div class="flex gap-3"><HardDrive class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('选择数据保存位置', 'Choose where data is stored')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('附件、反馈文件和语音模型会保存在这里。可以使用默认位置，也可以在开始前更改。', 'Attachments, feedback files, and voice models are stored here. Keep the default location or choose another before starting.')}</p></div></div>
          <p class="mt-6 break-all rounded-lg border bg-muted/20 p-4 font-mono text-xs">{storage?.selected_path ?? tr('正在读取…', 'Loading…')}</p>
          <div class="flex flex-wrap items-center justify-between gap-3"><p class="m-0 text-xs text-muted-foreground">{tr('数据库和本地凭据仍保存在系统应用目录。', 'The database and local credentials stay in the system app directory.')}</p><Button variant="outline" disabled={storageBusy} onclick={() => void chooseStorage()}><FolderCog class="size-4" />{tr('更改位置', 'Change location')}</Button></div>
          {#if storage?.restart_required}<p class="mt-4 text-xs leading-5">{tr('需要重启以启用新位置，重启后会继续剩余设置。', 'Restart to use the new location, then continue the remaining setup.')}</p>{/if}
        </section>
      {:else if steps[step] === 'voice'}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><Mic class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{translate('Ramble quickly with voice')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{translate('SenseVoice is the recommended default for reliable multilingual transcription. Audio stays on this device and is never uploaded. You can skip this and choose another model, microphone, or VAD settings later.')}</p></div></div>
          <div class="mt-6 rounded-lg border bg-muted/20 p-4">
            <label for="onboarding-model" class="text-xs font-medium">{translate('Transcription model')}</label>
            <select id="onboarding-model" disabled={modelBusy || modelsLoading} class="mt-2 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$speechModelId} onchange={(event) => setSpeechModelId((event.currentTarget as HTMLSelectElement).value as SpeechModelId)}>
              {#each models as model (model.id)}<option value={model.id}>{speechModelDisplayName($locale, model.id, model.display_name)}{model.id === DEFAULT_SPEECH_MODEL_ID ? ` · ${translate('Recommended')}` : ''}{model.installed ? ` · ${translate('Installed')}` : ''}</option>{/each}
            </select>
            {#if selectedModel}<div class="mt-4 flex items-start justify-between gap-4"><div><div class="flex flex-wrap gap-2">{#if selectedModel.id === DEFAULT_SPEECH_MODEL_ID}<Badge variant="secondary">{translate('Recommended')}</Badge>{/if}<Badge variant={selectedModel.installed ? 'secondary' : 'outline'}>{selectedModel.installed ? translate('Installed') : mb(selectedModel.size_bytes)}</Badge><Badge variant="outline">{selectedModel.streaming ? translate('Live streaming') : translate('VAD segmented')}</Badge></div><p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{speechModelDescription($locale, selectedModel.id, selectedModel.description)}</p></div>{#if !selectedModel.installed}<Button disabled={modelBusy} onclick={() => void downloadModel()}>{#if modelBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<Download data-icon="inline-start" />{/if}{modelBusy ? `${modelProgressPercent}%` : selectedModel.id === DEFAULT_SPEECH_MODEL_ID ? translate('Download recommended model') : translate('Download model')}</Button>{/if}</div>{/if}
            {#if modelBusy}<div class="mt-4 h-1.5 overflow-hidden rounded bg-muted"><div class="h-full bg-primary transition-[width]" style={`width: ${modelProgressPercent}%`}></div></div>{/if}
          </div>
        </section>
        {:else if steps[step] === 'permissions'}
          <section class="mx-auto max-w-xl">
            <div class="flex gap-3"><ShieldCheck class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{translate('Grant Mac permissions')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{translate('Screen capture and voice transcription require macOS permissions. Grant them now or later in Settings → Permissions.')}</p></div></div>
            <div class="mt-6">
              <MacPermissions
                bind:restartRequired={permissionRestartRequired}
                systemPermissions={capabilities.systemPermissions}
                notifications={capabilities.notifications}
                windowControls={capabilities.windowControls}
              />
            </div>
          </section>
      {:else if steps[step] === 'agents'}
        <div class="mb-5 rounded-lg bg-primary/5 px-4 py-3 text-xs leading-5">{tr('首次检测会查找本机程序并检查 ACP 连接，可能需要一些时间。之后打开设置不会重复检测，你可以按需手动触发。', 'The first scan finds local programs and checks ACP connections, which may take a little time. Opening Settings later will reuse results; further scans are manual.')}</div>
        <AgentCatalog {transport} autoDetect={detectOnEntry} initialConfigId={selectedConfig?.id} onReady={(config) => selectedConfig = config} />
      {:else if steps[step] === 'notifications'}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><BellRing class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{translate('Would you like notifications?')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{#if isWindows}{translate('Current unsigned Windows builds cannot show system banners. RambleDesk will not try to send them. Watch the inbox badge and use sound alerts instead.')}{:else}{translate('When a coding tool requests feedback, RambleDesk can show a system notification and play a sound.')}{/if}</p></div></div>
          <div class="mt-6 space-y-3 rounded-lg border bg-muted/20 p-4">
            <div class="flex items-center justify-between gap-4">
              <div><strong class="text-xs">{translate('System notifications')}</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{#if isWindows}{translate('System banners are not available on this Windows build.')}{:else}{translate('Show new feedback requests in the system notification center.')}{/if}</p></div>
              {#if isWindows}<Badge variant="secondary">{translate('Unavailable')}</Badge>{:else if $notificationPopupEnabled}<Badge variant="secondary">{translate('Enabled')}</Badge>{:else}<Button size="sm" disabled={notificationBusy || capabilities.notifications.status.availability === 'unavailable'} onclick={() => void enableNotifications()}>{#if notificationBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{/if}{translate('Allow notifications')}</Button>{/if}
            </div>
            <div class="flex items-center justify-between gap-4 border-t pt-3">
              <div><strong class="text-xs">{translate('Sound alerts')}</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{translate('Play a sound when a notification arrives.')}</p></div>
              <button type="button" role="switch" aria-label={translate('Sound alerts')} aria-checked={$notificationSoundEnabled} class={['relative h-[22px] w-10 rounded-full transition-colors', $notificationSoundEnabled ? 'bg-primary' : 'bg-input']} onclick={toggleSound}><span class={['absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow transition-transform', $notificationSoundEnabled ? 'translate-x-5' : '']}></span></button>
            </div>
            <p class="m-0 border-t pt-3 text-[10px] leading-4 text-muted-foreground">{translate('Sound, volume, and other advanced notification options can be adjusted anytime in Settings → Notifications.')}</p>
          </div>
        </section>
      {:else if steps[step] === 'cooking'}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><ChefHat class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{translate('Enable Feedback Cooking?')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{translate('Optional: use your own model service to turn a raw Ramble into formal feedback before submitting. The uncooked source is always preserved.')}</p></div></div>
          <div class="mt-6 rounded-lg border bg-muted/20 p-4"><div class="flex items-center justify-between gap-4"><div><strong class="text-xs">Cooking</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{translate('An API key is required and the feedback body is sent to your selected service.')}</p></div><button type="button" role="switch" aria-label="Cooking" aria-checked={$cookingEnabled} class={['relative h-[22px] w-10 rounded-full transition-colors', $cookingEnabled ? 'bg-primary' : 'bg-input']} onclick={toggleCooking}><span class={['absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow transition-transform', $cookingEnabled ? 'translate-x-5' : '']}></span></button></div>
            {#if $cookingEnabled}<div class="mt-5 grid gap-3 border-t pt-4"><label class="text-xs font-medium">{translate('Model provider')}<select class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingProvider} onchange={(event) => chooseCookingProvider((event.currentTarget as HTMLSelectElement).value as CookingProvider)}><option value="deepseek">DeepSeek</option><option value="openai">OpenAI</option><option value="compatible">{translate('OpenAI-compatible service')}</option></select></label><label class="text-xs font-medium">Base URL<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" type="url" value={$cookingBaseUrl} oninput={(event) => setCookingBaseUrl((event.currentTarget as HTMLInputElement).value)} /></label><div class="grid grid-cols-2 gap-3"><label class="text-xs font-medium">{translate('Model name')}<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingModel} oninput={(event) => setCookingModel((event.currentTarget as HTMLInputElement).value)} /></label><label class="text-xs font-medium">{translate('Reasoning effort')}<select class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingReasoningEffort} onchange={(event) => setCookingReasoningEffort((event.currentTarget as HTMLSelectElement).value as CookingReasoningEffort)}><option value="none">{translate('None')}</option><option value="minimal">minimal</option><option value="low">low</option><option value="medium">medium</option><option value="high">high</option><option value="xhigh">xhigh</option><option value="max">max</option></select></label></div><label class="text-xs font-medium">API Key<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" type="password" autocomplete="off" value={$cookingApiKey} placeholder="sk-…" oninput={(event) => setCookingApiKey((event.currentTarget as HTMLInputElement).value)} /></label></div>{/if}
          </div>
        </section>
      {:else}
        <section class="mx-auto flex max-w-xl flex-col items-center py-8 text-center">
          <span class="grid size-14 place-items-center rounded-2xl bg-primary/10 text-primary">{#if selectedConfig}<Check class="size-7" />{:else}<MessageSquare class="size-7" />{/if}</span>
          <h2 class="mb-0 mt-5 text-xl font-semibold">{tr('开始你的第一个会话', 'Start your first session')}</h2>
          <p class="mb-0 mt-3 text-sm leading-6 text-muted-foreground">{selectedConfig ? tr('已选择 ' + selectedConfig.name + '。接下来选择项目目录，输入任务即可开始。', selectedConfig.name + ' is selected. Next, choose a project folder and enter your task.') : tr('接下来会打开新建会话页。你可以在那里选择智能体、继续完成连接，再开始任务。', 'The new-session page will open next. Choose an agent there, finish connecting it, and start your task.')}</p>
          <p class="mb-0 mt-3 text-xs leading-5 text-muted-foreground">{tr('登录或模型访问遇到问题时，按照智能体的配置指引处理后重试。', 'If sign-in or model access needs attention, follow the agent’s setup instructions and retry.')}</p>
        </section>
      {/if}
      {#if error}<p role="alert" class="mt-4 break-words rounded-lg border border-destructive/30 p-3 text-xs">{error}</p>{/if}
    </div>
    <footer class="flex shrink-0 flex-wrap items-center justify-between gap-3 border-t bg-muted/15 px-7 py-4">
      {#if !isFinalStep}<Button variant="ghost" size="sm" disabled={starting || storageBusy || modelBusy || storage?.restart_required || permissionRestartRequired} onclick={() => void complete()}>{tr('稍后设置', 'Set up later')}</Button>{:else}<span></span>{/if}
      <div class="flex items-center gap-2">
        {#if step > 0}<Button variant="outline" size="sm" disabled={starting || storageBusy || modelBusy || storage?.restart_required || permissionRestartRequired} onclick={() => move(step - 1)}><ChevronLeft class="size-4" />{tr('上一步', 'Back')}</Button>{/if}
        {#if storage?.restart_required}<Button disabled={storageBusy || !windowControlsAvailable} onclick={() => void restart('storage')}><Rocket class="size-4" />{tr('重启并继续', 'Restart and continue')}</Button>
        {:else if permissionRestartRequired}<Button disabled={storageBusy || !windowControlsAvailable} onclick={() => void restart('permissions')}><Rocket class="size-4" />{tr('重启并继续', 'Restart and continue')}</Button>
        {:else if isFinalStep}<Button disabled={starting} onclick={() => void complete()}>{#if starting}<LoaderCircle class="size-4 animate-spin" />{:else}<MessageSquare class="size-4" />{/if}{tr(starting ? '正在打开…' : '新建会话', starting ? 'Opening…' : 'New session')}</Button>
        {:else}<Button disabled={starting || storageBusy || modelBusy} onclick={() => move(step + 1)}>{steps[step] === 'agents' && !selectedConfig ? tr('稍后连接', 'Connect later') : steps[step] === 'voice' && !selectedModel?.installed ? translate('Skip voice setup') : tr('继续', 'Continue')}<ChevronRight class="size-4" /></Button>{/if}
      </div>
    </footer>
  </Dialog.Content>
</Dialog.Root>
