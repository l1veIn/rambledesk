<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { open } from '@tauri-apps/plugin-dialog'
  import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification'
  import {
    BellRing,
    Check,
    ChefHat,
    ChevronLeft,
    ChevronRight,
    Download,
    FolderCog,
    HardDrive,
    LoaderCircle,
    Mic,
    PlugZap,
    Rocket,
    Sparkles,
    Volume2,
  } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { toast } from '$lib/components/ui/sonner'
  import { t } from '$lib/i18n'
  import {
    speechModelDescription,
    speechModelDisplayName,
  } from '$lib/speechModelLabels'
  import piLogo from '../assets/pi-logo.svg'
  import {
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

  const steps = ['欢迎', '数据位置', '语音输入', '适配器', '通知', 'Cooking', '完成']
  const isTauri = '__TAURI_INTERNALS__' in window
  let step = 0
  let wasOpen = false
  let closing = false
  let storage: StorageView | null = null
  let storageBusy = false
  let storageRestartRequired = false
  let models: SpeechModel[] = []
  let modelBusy = false
  let modelProgress: ModelProgress | null = null
  let hosts: McpHost[] = []
  let hostsLoading = false
  let hostSelections = new Set<string>()
  let adapterBusy = false
  let piBusy = false
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

  function complete(showToast = true) {
    closing = true
    finishOnboarding()
    openWizard = false
    onClose()
    if (showToast) toast.success(tr('RambleDesk 已准备就绪'))
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
      toast.error(tr('无法读取数据存储位置'), { description: messageFrom(cause) })
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
      toast.success(tr('数据存储位置已保存'))
    } catch (cause) {
      toast.error(tr('无法更改数据存储位置'), { description: messageFrom(cause) })
    } finally {
      storageBusy = false
    }
  }

  async function restartForStorage() {
    setOnboardingStep(2)
    try {
      await invoke('restart_application')
    } catch (cause) {
      toast.error(tr('无法重启 RambleDesk'), { description: messageFrom(cause) })
    }
  }

  async function loadModels() {
    try {
      models = await invoke<SpeechModel[]>('list_speech_models')
    } catch (cause) {
      toast.error(tr('无法读取语音模型'), { description: messageFrom(cause) })
    }
  }

  async function downloadModel() {
    if (!selectedModel || selectedModel.installed || modelBusy) return
    modelBusy = true
    modelProgress = { model_id: selectedModel.id, downloaded: 0, total: selectedModel.size_bytes }
    try {
      await invoke<SpeechModel>('download_speech_model', { modelId: selectedModel.id })
      await loadModels()
      toast.success(tr('语音模型已安装'))
    } catch (cause) {
      toast.error(tr('语音模型下载失败'), { description: messageFrom(cause) })
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
      toast.error(tr('无法检测适配器宿主'), { description: messageFrom(cause) })
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
      toast.success(tr('适配器已配置'), { description: tr('请重启 {count} 个宿主后使用。', { count: changed }) })
      await loadHosts()
    } catch (cause) {
      toast.error(tr('适配器安装失败'), { description: messageFrom(cause) })
    } finally {
      adapterBusy = false
    }
  }

  async function installPi() {
    if (piBusy) return
    piBusy = true
    try {
      await invoke<string>('install_pi_package', { checkoutRoot: null })
      toast.success(tr('Pi 原生适配器已安装'))
    } catch (cause) {
      toast.error(tr('Pi 适配器安装失败'), { description: messageFrom(cause) })
    } finally {
      piBusy = false
    }
  }

  async function enableNotifications() {
    if (!isTauri || notificationBusy) return
    notificationBusy = true
    try {
      const permission = (await isPermissionGranted()) ? 'granted' : await requestPermission()
      if (permission !== 'granted') throw new Error(tr('操作系统没有授予弹窗通知权限。'))
      setNotificationPopupEnabled(true)
      toast.success(tr('系统通知已开启'))
    } catch (cause) {
      setNotificationPopupEnabled(false)
      toast.error(tr('无法开启系统通知'), { description: messageFrom(cause) })
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
    class="flex max-h-[calc(100vh-2rem)] w-[min(760px,calc(100vw-2rem))] max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none"
    aria-describedby="onboarding-description"
  >
    <Dialog.Header class="shrink-0 border-b bg-muted/25 px-7 py-6">
      <div class="flex items-center gap-3">
        <span class="grid size-10 place-items-center rounded-xl bg-primary text-primary-foreground">
          <Sparkles class="size-5" />
        </span>
        <div>
          <Dialog.Title>{tr('欢迎使用 RambleDesk')}</Dialog.Title>
          <Dialog.Description id="onboarding-description" class="mt-1 text-xs">
            {tr('几步完成常用配置；所有选项之后都可在设置中调整。')}
          </Dialog.Description>
        </div>
      </div>
      <div class="mt-5 flex gap-1.5" aria-label={tr('引导进度')}>
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
      {#if step === 0}
        <div class="mx-auto flex max-w-lg flex-col items-center text-center">
          <span class="grid size-16 place-items-center rounded-2xl bg-primary/10 text-primary"><Rocket class="size-8" /></span>
          <h2 class="mb-0 mt-5 text-xl font-semibold">{tr('把体验中的想法，及时变成可用反馈。')}</h2>
          <p class="mb-0 mt-3 text-sm leading-6 text-muted-foreground">
            {tr('RambleDesk 收集文字、语音和截图反馈，交给你的 Coding 工具；Cooking 还能把原始 Ramble 整理成正式反馈。')}
          </p>
          <div class="mt-5 flex items-center gap-2 text-xs">
            <span class="text-muted-foreground">{tr('界面语言')}</span>
            <Button size="sm" variant={$locale === 'zh-CN' ? 'default' : 'outline'} onclick={() => setLocale('zh-CN')}>简体中文</Button>
            <Button size="sm" variant={$locale === 'en' ? 'default' : 'outline'} onclick={() => setLocale('en')}>English</Button>
          </div>
          <div class="mt-5 grid w-full grid-cols-3 gap-3 text-left text-xs">
            <div class="rounded-lg border bg-muted/20 p-3"><Mic class="mb-2 size-4 text-primary" />{tr('本地语音转录')}</div>
            <div class="rounded-lg border bg-muted/20 p-3"><PlugZap class="mb-2 size-4 text-primary" />{tr('Coding 工具适配')}</div>
            <div class="rounded-lg border bg-muted/20 p-3"><ChefHat class="mb-2 size-4 text-primary" />{tr('可选 AI Cooking')}</div>
          </div>
        </div>
      {:else if step === 1}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><HardDrive class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('先决定数据放在哪里')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('反馈附件、已发布反馈包和语音模型都会存放在此目录。先设置它，后续下载和反馈就直接写入正确位置。')}</p></div></div>
          <div class="mt-6 rounded-lg border bg-muted/20 p-4"><p class="m-0 text-[10px] font-medium uppercase text-muted-foreground">{tr('当前目录')}</p><p class="mb-0 mt-2 break-all font-mono text-xs">{storage?.selected_path ?? tr('正在读取数据存储位置…')}</p></div>
          <div class="mt-4 flex items-center justify-between gap-4"><p class="m-0 text-xs text-muted-foreground">{tr('数据库与本地凭证仍留在系统应用目录。')}</p><Button variant="outline" disabled={!isTauri || storageBusy} onclick={() => void chooseStorage()}>{#if storageBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<FolderCog data-icon="inline-start" />{/if}{tr('选择其他位置…')}</Button></div>
          {#if storageRestartRequired}<div class="mt-5 rounded-lg border border-primary/30 bg-primary/5 p-4 text-xs leading-5 text-primary">{tr('数据目录已保存。现在重启 RambleDesk，后续步骤会直接使用新位置。')}</div>{/if}
        </section>
      {:else if step === 2}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><Mic class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('用语音快速 Ramble')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('推荐下载一个本地转录模型：音频仅在本机处理，不会上传。也可以跳过，稍后在设置中选择模型、麦克风和 VAD。')}</p></div></div>
          <div class="mt-6 rounded-lg border bg-muted/20 p-4">
            <label for="onboarding-model" class="text-xs font-medium">{tr('转录模型')}</label>
            <select id="onboarding-model" class="mt-2 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$speechModelId} onchange={(event) => setSpeechModelId((event.currentTarget as HTMLSelectElement).value as SpeechModelId)}>
              {#each models as model (model.id)}<option value={model.id}>{speechModelDisplayName($locale, model.id, model.display_name)}{model.installed ? ` · ${tr('已安装')}` : ''}</option>{/each}
            </select>
            {#if selectedModel}<div class="mt-4 flex items-start justify-between gap-4"><div><div class="flex gap-2"><Badge variant={selectedModel.installed ? 'secondary' : 'outline'}>{selectedModel.installed ? tr('已安装') : mb(selectedModel.size_bytes)}</Badge><Badge variant="outline">{selectedModel.streaming ? tr('流式实时') : tr('VAD 分段')}</Badge></div><p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{speechModelDescription($locale, selectedModel.id, selectedModel.description)}</p></div>{#if !selectedModel.installed}<Button disabled={modelBusy} onclick={() => void downloadModel()}>{#if modelBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<Download data-icon="inline-start" />{/if}{modelBusy ? `${modelProgressPercent}%` : tr('下载推荐模型')}</Button>{/if}</div>{/if}
            {#if modelBusy}<div class="mt-4 h-1.5 overflow-hidden rounded bg-muted"><div class="h-full bg-primary transition-[width]" style={`width: ${modelProgressPercent}%`}></div></div>{/if}
          </div>
        </section>
      {:else if step === 3}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3">
            <PlugZap class="mt-0.5 size-6 text-primary" />
            <div>
              <h2 class="m-0 text-lg font-semibold">{tr('连接你的 Coding 工具')}</h2>
              <p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('推荐 Pi 的原生自动继续；也可以按需配置通用 MCP 宿主。')}</p>
            </div>
          </div>

          <div class="mt-6 rounded-xl border-2 border-primary/30 bg-primary/5 p-5">
            <div class="flex items-start gap-4">
              <span class="grid size-14 shrink-0 place-items-center rounded-xl bg-primary shadow-sm">
                <img src={piLogo} alt="Pi" class="size-9 brightness-0 invert" />
              </span>
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <h3 class="m-0 text-base font-semibold">{tr('Pi 原生自动继续')}</h3>
                  <Badge variant="secondary">{tr('推荐')}</Badge>
                </div>
                <p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{tr('Pi 在同一个工具调用内等待反馈完成，再自动继续当前会话；无需复制或手动发送恢复提示。')}</p>
                <div class="mt-3 flex flex-wrap gap-2 text-[10px]">
                  <span class="rounded-full border border-primary/20 bg-background px-2 py-1 text-primary" title={tr('反馈完成后 Pi 会在当前工具调用中自动继续。')}>{tr('原生自动继续')}</span>
                  <span class="rounded-full border bg-background px-2 py-1 text-muted-foreground" title={tr('不需要回到 Coding 工具手动恢复对话。')}>{tr('无需手动继续')}</span>
                </div>
              </div>
              <Button size="sm" disabled={piBusy || !isTauri} onclick={() => void installPi()}>
                {#if piBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<Download data-icon="inline-start" />{/if}
                {piBusy ? tr('正在安装…') : tr('安装 Pi 适配器')}
              </Button>
            </div>
          </div>

          <details class="mt-4 rounded-lg border bg-muted/20">
            <summary class="cursor-pointer list-none px-4 py-3 text-xs outline-none [&::-webkit-details-marker]:hidden">
              <span class="flex items-center gap-2">
                <PlugZap class="size-4 text-muted-foreground" />
                <span class="font-medium">{tr('通用 MCP 宿主')}</span>
                <Badge variant="outline" title={tr('提交或取消后，需要手动回到宿主继续当前会话。')}>{tr('手动继续')}</Badge>
                <span class="ml-auto text-[10px] text-muted-foreground">{tr('按需配置')}</span>
              </span>
            </summary>
            <div class="border-t p-4">
              <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('MCP 宿主在提交或取消后需要手动回到 Coding 工具，使用恢复提示继续会话。RambleDesk 只写入自己的 MCP 配置，不会覆盖其他服务器。')}</p>
              {#if hostsLoading}
                <p class="mb-0 mt-4 flex items-center gap-2 text-xs text-muted-foreground"><LoaderCircle class="size-4 animate-spin" />{tr('正在检测 Coding 工具…')}</p>
              {:else if hosts.length === 0}
                <p class="mb-0 mt-4 text-xs leading-5 text-muted-foreground">{tr('尚未检测到支持的工具。可跳过此步，之后在“设置 → 适配器”安装。')}</p>
              {:else}
                <div class="mt-4 space-y-1">
                  {#each hosts as host (host.id)}
                    <label class={['flex items-center gap-3 rounded-md px-2 py-2 text-xs', host.installed ? 'cursor-pointer hover:bg-muted' : 'cursor-not-allowed opacity-55']}>
                      <input type="checkbox" class="size-3.5 accent-primary" checked={hostSelections.has(host.id)} disabled={!host.installed || adapterBusy} onchange={() => toggleHost(host.id)} />
                      <span class="flex-1 font-medium">{host.name}</span>
                      <Badge variant={host.configured ? 'secondary' : 'outline'}>{host.configured ? tr('已配置') : host.installed ? tr('已检测') : tr('未检测到')}</Badge>
                    </label>
                  {/each}
                </div>
                <Button class="mt-4" disabled={adapterBusy || selectedHosts.length === 0} onclick={() => void installSelectedHosts()}>
                  {#if adapterBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<PlugZap data-icon="inline-start" />{/if}
                  {tr('安装所选 MCP 适配器')}
                </Button>
              {/if}
            </div>
          </details>
        </section>
      {:else if step === 4}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><BellRing class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('需要通知提醒吗？')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('当 Coding 工具请求反馈时，RambleDesk 可以显示系统弹窗并播放声音。')}</p></div></div>
          <div class="mt-6 space-y-3 rounded-lg border bg-muted/20 p-4">
            <div class="flex items-center justify-between gap-4">
              <div><strong class="text-xs">{tr('系统弹窗')}</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{tr('在系统通知中心显示新反馈请求。')}</p></div>
              {#if $notificationPopupEnabled}<Badge variant="secondary">{tr('已开启')}</Badge>{:else}<Button size="sm" disabled={notificationBusy || !isTauri} onclick={() => void enableNotifications()}>{#if notificationBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{/if}{tr('允许通知')}</Button>{/if}
            </div>
            <div class="flex items-center justify-between gap-4 border-t pt-3">
              <div><strong class="text-xs">{tr('声音提醒')}</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{tr('通知到达时播放提示音。')}</p></div>
              <button type="button" role="switch" aria-label={tr('声音提醒')} aria-checked={$notificationSoundEnabled} class={['relative h-[22px] w-10 rounded-full transition-colors', $notificationSoundEnabled ? 'bg-primary' : 'bg-input']} onclick={() => setNotificationSoundEnabled(!$notificationSoundEnabled)}><span class={['absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow transition-transform', $notificationSoundEnabled ? 'translate-x-5' : '']}></span></button>
            </div>
            <p class="m-0 border-t pt-3 text-[10px] leading-4 text-muted-foreground">{tr('提示音、音量等高级通知选项可随时在“设置 → 通知”中调整。')}</p>
          </div>
        </section>
      {:else if step === 5}
        <section class="mx-auto max-w-xl">
          <div class="flex gap-3"><ChefHat class="mt-0.5 size-6 text-primary" /><div><h2 class="m-0 text-lg font-semibold">{tr('要启用 Feedback Cooking 吗？')}</h2><p class="mb-0 mt-2 text-sm leading-6 text-muted-foreground">{tr('可选：提交前用你自己的模型服务把原始 Ramble 整理为正式反馈。原始 uncooked 正文始终保留。')}</p></div></div>
          <div class="mt-6 rounded-lg border bg-muted/20 p-4"><div class="flex items-center justify-between gap-4"><div><strong class="text-xs">Cooking</strong><p class="mb-0 mt-1 text-[10px] text-muted-foreground">{tr('需要 API Key，且正文会发送给你选择的服务。')}</p></div><button type="button" role="switch" aria-label="Cooking" aria-checked={$cookingEnabled} class={['relative h-[22px] w-10 rounded-full transition-colors', $cookingEnabled ? 'bg-primary' : 'bg-input']} onclick={() => setCookingEnabled(!$cookingEnabled)}><span class={['absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow transition-transform', $cookingEnabled ? 'translate-x-5' : '']}></span></button></div>
            {#if $cookingEnabled}<div class="mt-5 grid gap-3 border-t pt-4"><label class="text-xs font-medium">{tr('模型服务')}<select class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingProvider} onchange={(event) => chooseCookingProvider((event.currentTarget as HTMLSelectElement).value as CookingProvider)}><option value="deepseek">DeepSeek</option><option value="openai">OpenAI</option><option value="compatible">{tr('OpenAI 兼容服务')}</option></select></label><label class="text-xs font-medium">Base URL<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" type="url" value={$cookingBaseUrl} oninput={(event) => setCookingBaseUrl((event.currentTarget as HTMLInputElement).value)} /></label><div class="grid grid-cols-2 gap-3"><label class="text-xs font-medium">{tr('模型名称')}<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingModel} oninput={(event) => setCookingModel((event.currentTarget as HTMLInputElement).value)} /></label><label class="text-xs font-medium">{tr('思考强度')}<select class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" value={$cookingReasoningEffort} onchange={(event) => setCookingReasoningEffort((event.currentTarget as HTMLSelectElement).value as CookingReasoningEffort)}><option value="none">{tr('不使用')}</option><option value="minimal">minimal</option><option value="low">low</option><option value="medium">medium</option><option value="high">high</option><option value="xhigh">xhigh</option><option value="max">max</option></select></label></div><label class="text-xs font-medium">API Key<input class="mt-1.5 h-9 w-full rounded-md border bg-background px-3 text-xs" type="password" autocomplete="off" value={$cookingApiKey} placeholder="sk-…" oninput={(event) => setCookingApiKey((event.currentTarget as HTMLInputElement).value)} /></label></div>{/if}
          </div>
        </section>
      {:else}
        <div class="mx-auto flex max-w-lg flex-col items-center pt-8 text-center">
          <span class="grid size-16 place-items-center rounded-2xl bg-success/10 text-success"><Check class="size-8" /></span>
          <h2 class="mb-0 mt-5 text-xl font-semibold">{tr('准备完成')}</h2>
          <p class="mb-0 mt-3 text-sm leading-6 text-muted-foreground">{tr('现在可以从 Coding 工具直接开始 Ramble 了。所有设置均可在右上角设置中更改。')}</p>
          <div class="mt-6 w-full rounded-lg border bg-muted/20 p-4 text-left">
            <p class="m-0 text-[10px] font-medium text-muted-foreground">{tr('把这句示例提示词粘贴到 Coding agent / Coding 工具中：')}</p>
            <code class="mt-2 block rounded-md bg-background px-3 py-2 text-xs text-foreground">{tr('今天我们用 RambleDesk 开发')}</code>
          </div>
          <div class="mt-3 flex items-center gap-2 rounded-lg border bg-muted/20 px-4 py-3 text-xs text-muted-foreground"><Volume2 class="size-4 text-primary" />{tr('提示：语音模型下载完成后即可使用麦克风。')}</div>
        </div>
      {/if}
    </div>

    <footer class="flex shrink-0 items-center justify-between border-t bg-muted/15 px-7 py-4">
      <Button variant="ghost" size="sm" onclick={() => complete(false)}>{tr('稍后设置')}</Button>
      <div class="flex items-center gap-2">
        {#if step > 0 && !storageRestartRequired}<Button variant="outline" size="sm" onclick={() => move(step - 1)}><ChevronLeft data-icon="inline-start" />{tr('上一步')}</Button>{/if}
        {#if storageRestartRequired}<Button disabled={storageBusy} onclick={() => void restartForStorage()}><Rocket data-icon="inline-start" />{tr('重启并继续')}</Button>
        {:else if step === steps.length - 1}<Button onclick={() => complete()}><Check data-icon="inline-start" />{tr('开始使用')}</Button>
        {:else}<Button disabled={storageBusy || modelBusy} onclick={() => move(step + 1)}>{step === 2 && !selectedModel?.installed ? tr('跳过语音设置') : tr('继续')}<ChevronRight data-icon="inline-end" /></Button>{/if}
      </div>
    </footer>
  </Dialog.Content>
</Dialog.Root>
