<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { open } from '@tauri-apps/plugin-dialog'
  import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification'
  import {
    BellRing,
    Bot,
    Check,
    CheckCircle2,
    ChefHat,
    ChevronDown,
    Clipboard,
    Download,
    FolderCog,
    Languages,
    LoaderCircle,
    Mic,
    MonitorCog,
    Info,
    Play,
    PlugZap,
    RefreshCw,
    Rocket,
    ShieldCheck,
    TerminalSquare,
    Trash2,
    Volume2,
  } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import * as Alert from '$lib/components/ui/alert'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import * as Dialog from '$lib/components/ui/dialog'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import { toast } from '$lib/components/ui/sonner'
  import AboutSettings from '$lib/AboutSettings.svelte'
  import rambellePermission from '../assets/rambelle-states/state-permission.png'
  import piLogoSvg from '../assets/pi-logo.svg?raw'
  import * as Select from '$lib/components/ui/select'
  import * as Tabs from '$lib/components/ui/tabs'
  import { t } from '$lib/i18n'
  import {
    speechModelDescription,
    speechModelDisplayName,
    speechModelLanguages,
  } from '$lib/speechModelLabels'
  import { playNotificationSound } from '$lib/notifications'
  import {
    cookingApiKey,
    cookingBaseUrl,
    cookingEnabled,
    cookingModel,
    cookingProvider,
    cookingReasoningEffort,
    locale,
    notificationPopupEnabled,
    notificationSound,
    notificationSoundEnabled,
    notificationVolume,
    setCookingApiKey,
    setCookingBaseUrl,
    setCookingEnabled,
    setCookingModel,
    setCookingProvider,
    setCookingReasoningEffort,
    setLocale,
    setNotificationPopupEnabled,
    setNotificationSound,
    setNotificationSoundEnabled,
    setNotificationVolume,
    setSpeechInputDevice,
    setSpeechModelId,
    setSpeechVadSilenceMs,
    setSpeechVadThreshold,
    setThemePreference,
    speechInputDevice,
    speechModelId,
    speechVadSilenceMs,
    speechVadThreshold,
    themePreference,
    type CookingProvider,
    type CookingReasoningEffort,
    type NotificationSound,
    type SpeechModelId,
    type ThemePreference,
  } from '$lib/preferences'

  type Section = 'general' | 'notifications' | 'voice' | 'adapters' | 'about'

  export let mcpConfiguration = ''
  export let initialSection: Section = 'general'
  export let onClose: () => void = () => {}
  export let onRestartOnboarding: () => void = () => {}
  export let updateInstallBlocked = false

  type DataStorageView = {
    active_path: string
    selected_path: string
    restart_required: boolean
  }

  type SpeechModelInfo = {
    id: SpeechModelId
    engine_id: string
    display_name: string
    description: string
    size_bytes: number
    installed: boolean
    path: string
    missing_files: string[]
    streaming: boolean
    languages: string[]
    license: string
  }

  type StorageMigrationProgress = {
    copied: number
    total: number
  }

  type SpeechModelProgress = {
    model_id: string
    downloaded: number
    total: number
  }

  type McpHostView = {
    id: string
    name: string
    iconSvg: string
    installed: boolean
    configured: boolean
    configPath: string
    restartRequired: boolean
  }

  type McpInstallResult = {
    hostId: string
    action: 'created' | 'updated' | 'unchanged'
    configPath: string
    restartRequired: boolean
  }

  type DshInstallResult = {
    profileId: string
    profileDir: string
    patchPath: string
    action: 'created' | 'updated' | 'unchanged'
    restartRequired: boolean
  }

  let dialogOpen = true
  let closeDelivered = false
  let activeSection: Section = initialSection
  let hosts: McpHostView[] = []
  let selectedIds = new Set<string>()
  let loadingHosts = true
  let installing = false
  let installMessage = ''
  let installError = ''
  let installingPi = false
  let piInstallMessage = ''
  let piInstallError = ''
  let installingDsh = false
  let dshInstallMessage = ''
  let dshInstallError = ''
  let copyState: 'idle' | 'copied' | 'error' = 'idle'
  let genericAdapterOpen = true
  let configurationOpen = false
  let notificationPermissionError = ''
  let dataStorage: DataStorageView | null = null
  let storageMessage = ''
  let storageError = ''
  let storageMigration: StorageMigrationProgress | null = null
  let storageMigrating = false
  let speechInputDevices: string[] = []
  let speechDeviceError = ''
  let speechModels: SpeechModelInfo[] = []
  let modelProgress: SpeechModelProgress | null = null
  let modelBusy = false
  let modelError = ''
  let unlistenModelProgress: UnlistenFn | null = null
  let unlistenStorageProgress: UnlistenFn | null = null
  const isTauri = '__TAURI_INTERNALS__' in window

  $: installedHosts = hosts.filter((host) => host.installed)
  $: selectedCount = selectedIds.size
  $: selectedSpeechModel =
    speechModels.find((model) => model.id === $speechModelId) ?? speechModels[0] ?? null
  $: if (!dialogOpen && !closeDelivered) {
    closeDelivered = true
    onClose()
  }

  onMount(() => {
    if (isTauri) {
      void refreshHosts()
      void refreshDataStorage()
      void refreshSpeechDevices()
      void refreshSpeechModels()
      void listen<SpeechModelProgress>('speech-model-progress', ({ payload }) => {
        modelProgress = payload
      }).then((unlisten) => (unlistenModelProgress = unlisten))
      void listen<StorageMigrationProgress>('storage-migration-progress', ({ payload }) => {
        storageMigration = payload
      }).then((unlisten) => (unlistenStorageProgress = unlisten))
    } else loadingHosts = false
    return () => {
      unlistenModelProgress?.()
      unlistenStorageProgress?.()
    }
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function refreshSpeechModels() {
    modelError = ''
    try {
      speechModels = await invoke<SpeechModelInfo[]>('list_speech_models')
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
      const updated = await invoke<SpeechModelInfo>('download_speech_model', { modelId })
      speechModels = speechModels.map((model) => (model.id === updated.id ? updated : model))
    } catch (cause) {
      modelError = messageFrom(cause)
    } finally {
      modelBusy = false
    }
  }

  async function deleteSpeechModel() {
    if (modelBusy || !selectedSpeechModel || !confirm(tr('确定删除本地语音模型吗？'))) return
    const modelId = selectedSpeechModel.id
    modelBusy = true
    modelError = ''
    try {
      const updated = await invoke<SpeechModelInfo>('delete_speech_model', { modelId })
      speechModels = speechModels.map((model) => (model.id === updated.id ? updated : model))
      modelProgress = null
    } catch (cause) {
      modelError = messageFrom(cause)
    } finally {
      modelBusy = false
    }
  }

  async function refreshSpeechDevices() {
    speechDeviceError = ''
    try {
      speechInputDevices = await invoke<string[]>('list_speech_input_devices')
    } catch (cause) {
      speechDeviceError = messageFrom(cause)
    }
  }

  async function refreshDataStorage() {
    try {
      dataStorage = await invoke<DataStorageView>('get_data_storage_settings')
    } catch (cause) {
      storageError = messageFrom(cause)
    }
  }

  async function chooseDataStorage() {
    storageError = ''
    storageMessage = ''
    try {
      const selected = await open({ directory: true, multiple: false })
      if (!selected || Array.isArray(selected)) return
      storageMigrating = true
      storageMigration = { copied: 0, total: 0 }
      dataStorage = await invoke<DataStorageView>('set_data_storage_path', { path: selected })
      storageMessage = dataStorage.restart_required
        ? tr('数据已迁移；新的数据存储位置将在重启 RambleDesk 后生效。')
        : tr('当前已使用这个数据存储位置。')
      toast.success(tr('存储设置已更新'), { description: storageMessage })
    } catch (cause) {
      storageError = messageFrom(cause)
      toast.error(tr('存储设置失败'), { description: storageError })
    } finally {
      storageMigrating = false
    }
  }

  async function refreshHosts() {
    loadingHosts = true
    installError = ''
    try {
      hosts = await invoke<McpHostView[]>('detect_generic_mcp_hosts')
      selectedIds = new Set(
        hosts
          .filter((host) => host.installed && !host.configured)
          .map((host) => host.id),
      )
    } catch (cause) {
      installError = messageFrom(cause)
    } finally {
      loadingHosts = false
    }
  }

  function toggleHost(host: McpHostView) {
    if (!host.installed || installing) return
    const next = new Set(selectedIds)
    if (next.has(host.id)) next.delete(host.id)
    else next.add(host.id)
    selectedIds = next
  }

  async function installSelected() {
    if (selectedIds.size === 0 || installing) return
    installing = true
    installError = ''
    installMessage = ''
    try {
      const results = await invoke<McpInstallResult[]>('install_generic_mcp_hosts', {
        hostIds: [...selectedIds],
      })
      const changed = results.filter((result) => result.action !== 'unchanged').length
      installMessage = tr('已为 {count} 个工具写入通用 MCP 适配器配置；重启这些工具后生效。', {
        count: changed,
      })
      await refreshHosts()
    } catch (cause) {
      installError = messageFrom(cause)
    } finally {
      installing = false
    }
  }

  async function copyConfiguration() {
    try {
      await navigator.clipboard.writeText(mcpConfiguration)
      copyState = 'copied'
    } catch {
      copyState = 'error'
    }
  }

  async function installPiPackage() {
    if (installingPi) return
    installingPi = true
    piInstallError = ''
    piInstallMessage = ''
    try {
      const output = await invoke<string>('install_pi_package', {
        checkoutRoot: null,
      })
      piInstallMessage =
        tr('已安装 Pi 原生适配器，重启 Pi 会话后生效。') +
        (output.trim() ? `\n${output.trim()}` : '')
      if (output.trim().length === 0) {
        piInstallMessage += `\n${tr('首次安装可能耗时十几秒，请稍候。')}`
      }
    } catch (cause) {
      piInstallError = messageFrom(cause)
    } finally {
      installingPi = false
    }
  }

  async function installDshPackage() {
    if (installingDsh) return
    installingDsh = true
    dshInstallError = ''
    dshInstallMessage = ''
    try {
      const results = await invoke<DshInstallResult[]>('install_dsh_package', {
        checkoutRoot: null,
        profileId: null,
      })
      const changed = results.filter((result) => result.action !== 'unchanged').length
      dshInstallMessage = tr(
        '已安装 DeepSeek Harness 原生适配器（{count} 个 profile：{profiles}），重启 dsh 后生效。',
        {
          count: changed,
          profiles: results.map((result) => result.profileId).join(', '),
        },
      )
    } catch (cause) {
      dshInstallError = messageFrom(cause)
    } finally {
      installingDsh = false
    }
  }

  async function togglePopupNotifications(enabled: boolean) {
    notificationPermissionError = ''
    if (!enabled) {
      setNotificationPopupEnabled(false)
      return
    }
    if (!isTauri) {
      notificationPermissionError = tr('系统弹窗通知只在桌面应用中可用。')
      return
    }
    try {
      const permission = (await isPermissionGranted()) ? 'granted' : await requestPermission()
      if (permission === 'granted') {
        setNotificationPopupEnabled(true)
      } else {
        setNotificationPopupEnabled(false)
        notificationPermissionError = tr('操作系统没有授予弹窗通知权限。')
      }
    } catch (cause) {
      setNotificationPopupEnabled(false)
      notificationPermissionError = messageFrom(cause)
    }
  }

  function soundLabel(sound: NotificationSound) {
    if (sound === 'soft') return tr('柔和提示')
    if (sound === 'alert') return tr('醒目提示')
    return tr('清脆双音')
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

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return String(cause)
  }
</script>

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content
    class="h-[min(680px,calc(100vh-5rem))] w-[min(940px,calc(100vw-3rem))] max-w-none gap-0 overflow-hidden p-0 sm:max-w-none"
    aria-describedby="settings-description"
  >
    <Dialog.Header class="sr-only">
      <Dialog.Title>{tr('设置')}</Dialog.Title>
      <Dialog.Description id="settings-description">
        {tr('管理界面偏好和宿主适配器。')}
      </Dialog.Description>
    </Dialog.Header>

    <Tabs.Root
      bind:value={activeSection}
      orientation="vertical"
      class="grid h-full min-h-0 grid-cols-[184px_minmax(0,1fr)] gap-0"
    >
      <aside class="flex min-h-0 flex-col border-r bg-muted/35 p-3">
        <div class="flex h-12 items-center gap-2 px-2">
          <span class="grid size-7 place-items-center rounded-md bg-primary text-xs font-bold text-primary-foreground">
            R
          </span>
          <div class="min-w-0">
            <strong class="block text-xs font-semibold">RambleDesk</strong>
            <span class="block text-[10px] text-muted-foreground">{tr('设置')}</span>
          </div>
        </div>

        <Tabs.List
          variant="line"
          class="mt-3 flex w-full flex-col items-stretch gap-1 bg-transparent p-0"
        >
          <Tabs.Trigger value="general" class="h-9 w-full justify-start px-2.5">
            <MonitorCog data-icon="inline-start" />
            {tr('通用')}
          </Tabs.Trigger>
          <Tabs.Trigger value="notifications" class="h-9 w-full justify-start px-2.5">
            <BellRing data-icon="inline-start" />
            {tr('通知')}
          </Tabs.Trigger>
          <Tabs.Trigger value="voice" class="h-9 w-full justify-start px-2.5">
            <Mic data-icon="inline-start" />
            {tr('语音')}
          </Tabs.Trigger>
          <Tabs.Trigger value="adapters" class="h-9 w-full justify-start px-2.5">
            <PlugZap data-icon="inline-start" />
            <span class="flex-1 text-left">{tr('适配器')}</span>
            {#if installedHosts.length > 0}
              <Badge variant="secondary" class="h-5 px-1.5 text-[9px]">
                {installedHosts.length}
              </Badge>
            {/if}
          </Tabs.Trigger>
          <Tabs.Trigger value="about" class="h-9 w-full justify-start px-2.5">
            <Info data-icon="inline-start" />
            {tr('关于')}
          </Tabs.Trigger>
        </Tabs.List>

        <div class="mt-auto flex gap-2 border-t pt-3 text-[10px] leading-4 text-muted-foreground">
          <ShieldCheck class="mt-0.5 size-3.5 shrink-0" />
          <span>{tr('适配器配置只写入当前用户目录，并保留其他适配器。')}</span>
        </div>
      </aside>

      <div class="flex min-h-0 min-w-0 flex-col">
        <header class="flex h-16 shrink-0 items-center border-b px-6">
          <div>
            <p class="m-0 text-[10px] font-medium uppercase text-muted-foreground">
              {activeSection === 'general'
                ? tr('偏好设置')
                : activeSection === 'notifications'
                  ? tr('提醒方式')
                  : activeSection === 'voice'
                    ? tr('语音输入')
                    : activeSection === 'adapters'
                      ? tr('宿主适配')
                      : tr('项目信息')}
            </p>
            <h2 class="m-0 mt-0.5 text-base font-semibold">
              {activeSection === 'general'
                ? tr('通用')
                : activeSection === 'notifications'
                  ? tr('通知')
                  : activeSection === 'voice'
                    ? tr('语音')
                    : activeSection === 'adapters'
                      ? tr('适配器')
                      : tr('关于')}
            </h2>
          </div>
        </header>

        <ScrollArea class="min-h-0 flex-1">
          <Tabs.Content value="general" class="m-0 space-y-8 p-6 outline-none">
            <section class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <Languages class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('语言')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('选择 RambleDesk 的界面语言。')}
                  </p>
                </div>
              </div>
              <Select.Root
                type="single"
                value={$locale}
                onValueChange={(value: string) => setLocale(value as 'zh-CN' | 'en')}
              >
                <Select.Trigger class="w-full">
                  {$locale === 'zh-CN' ? '简体中文' : 'English'}
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="zh-CN" label="简体中文" />
                  <Select.Item value="en" label="English" />
                </Select.Content>
              </Select.Root>
            </section>

            <section class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <MonitorCog class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('外观')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('选择界面明暗模式，也可以跟随操作系统。')}
                  </p>
                </div>
              </div>
              <Select.Root
                type="single"
                value={$themePreference}
                onValueChange={(value: string) => setThemePreference(value as ThemePreference)}
              >
                <Select.Trigger class="w-full">
                  {$themePreference === 'system'
                    ? tr('跟随系统')
                    : $themePreference === 'light'
                      ? tr('浅色')
                      : tr('深色')}
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="system" label={tr('跟随系统')} />
                  <Select.Item value="light" label={tr('浅色')} />
                  <Select.Item value="dark" label={tr('深色')} />
                </Select.Content>
              </Select.Root>
            </section>

            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <Rocket class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('新手引导')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('重新查看数据位置、语音、适配器、通知和 Cooking 的初始设置。')}
                  </p>
                </div>
              </div>
              <Button variant="outline" onclick={onRestartOnboarding}>
                <Rocket data-icon="inline-start" />
                {tr('再次启用新手引导')}
              </Button>
            </section>

            <section class="border-b pb-8">
              <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8">
                <div class="flex gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <ChefHat class="size-4" />
                  </span>
                  <div>
                    <div class="flex items-center gap-2">
                      <h3 class="m-0 text-sm font-medium">Cooking</h3>
                      <Badge variant="outline">{tr('可选')}</Badge>
                    </div>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('提交前用大模型把 Ramble 原稿整理成正式反馈；uncooked 原稿仍会保存在反馈包中。')}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={$cookingEnabled}
                  aria-label="Cooking"
                  class={[
                    'relative h-[22px] w-10 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                    $cookingEnabled ? 'bg-primary' : 'bg-input',
                  ]}
                  onclick={() => setCookingEnabled(!$cookingEnabled)}
                >
                  <span
                    class={[
                      'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform',
                      $cookingEnabled ? 'translate-x-5' : 'translate-x-0',
                    ]}
                  ></span>
                </button>
              </div>

              {#if $cookingEnabled}
                <div class="ml-11 mt-5 grid gap-4 rounded-md border bg-muted/20 p-4">
                  <div class="grid grid-cols-[140px_minmax(0,1fr)] items-center gap-4">
                    <label for="cooking-provider" class="text-xs font-medium">{tr('模型服务')}</label>
                    <Select.Root
                      type="single"
                      value={$cookingProvider}
                      onValueChange={(value: string) => chooseCookingProvider(value as CookingProvider)}
                    >
                      <Select.Trigger id="cooking-provider" class="w-full">
                        {$cookingProvider === 'deepseek'
                          ? 'DeepSeek'
                          : $cookingProvider === 'openai'
                            ? 'OpenAI'
                            : tr('OpenAI 兼容服务')}
                      </Select.Trigger>
                      <Select.Content>
                        <Select.Item value="deepseek" label="DeepSeek" />
                        <Select.Item value="openai" label="OpenAI" />
                        <Select.Item value="compatible" label={tr('OpenAI 兼容服务')} />
                      </Select.Content>
                    </Select.Root>
                  </div>
                  <div class="grid grid-cols-[140px_minmax(0,1fr)] items-center gap-4">
                    <label for="cooking-base-url" class="text-xs font-medium">Base URL</label>
                    <input
                      id="cooking-base-url"
                      type="url"
                      value={$cookingBaseUrl}
                      placeholder="https://api.example.com/v1"
                      class="h-9 w-full rounded-md border bg-background px-3 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      oninput={(event) => setCookingBaseUrl((event.currentTarget as HTMLInputElement).value)}
                    />
                  </div>
                  <div class="grid grid-cols-[140px_minmax(0,1fr)] items-center gap-4">
                    <label for="cooking-model" class="text-xs font-medium">{tr('模型名称')}</label>
                    <input
                      id="cooking-model"
                      type="text"
                      value={$cookingModel}
                      placeholder="deepseek-v4-flash"
                      class="h-9 w-full rounded-md border bg-background px-3 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      oninput={(event) => setCookingModel((event.currentTarget as HTMLInputElement).value)}
                    />
                  </div>
                  <div class="grid grid-cols-[140px_minmax(0,1fr)] items-center gap-4">
                    <label for="cooking-reasoning" class="text-xs font-medium">{tr('思考强度')}</label>
                    <Select.Root
                      type="single"
                      value={$cookingReasoningEffort}
                      onValueChange={(value: string) =>
                        setCookingReasoningEffort(value as CookingReasoningEffort)}
                    >
                      <Select.Trigger id="cooking-reasoning" class="w-full">
                        {$cookingReasoningEffort === 'none'
                          ? tr('不使用')
                          : $cookingReasoningEffort === 'minimal'
                            ? 'minimal'
                            : $cookingReasoningEffort}
                      </Select.Trigger>
                      <Select.Content>
                        <Select.Item value="none" label={tr('不使用')} />
                        <Select.Item value="minimal" label="minimal" />
                        <Select.Item value="low" label="low" />
                        <Select.Item value="medium" label="medium" />
                        <Select.Item value="high" label="high" />
                        <Select.Item value="xhigh" label="xhigh" />
                        <Select.Item value="max" label="max" />
                      </Select.Content>
                    </Select.Root>
                  </div>
                  <div class="grid grid-cols-[140px_minmax(0,1fr)] items-center gap-4">
                    <label for="cooking-api-key" class="text-xs font-medium">API Key</label>
                    <input
                      id="cooking-api-key"
                      type="password"
                      value={$cookingApiKey}
                      autocomplete="off"
                      placeholder="sk-…"
                      class="h-9 w-full rounded-md border bg-background px-3 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      oninput={(event) => setCookingApiKey((event.currentTarget as HTMLInputElement).value)}
                    />
                  </div>
                  <p class="m-0 text-[10px] leading-4 text-muted-foreground">
                    {tr('API Key 仅保存在当前设备的本地设置中，不会写入反馈包；Cooking 会把 uncooked 正文发送给所选模型服务。')}
                  </p>
                </div>
              {/if}
            </section>

            <section class="grid gap-4">
              <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8">
                <div class="flex gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <FolderCog class="size-4" />
                  </span>
                  <div>
                    <h3 class="m-0 text-sm font-medium">{tr('数据存储位置')}</h3>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('反馈附件、已发布反馈包和语音模型存放在这里；数据库与凭证仍保留在系统目录。')}
                    </p>
                  </div>
                </div>
                <Button variant="outline" disabled={!isTauri || storageMigrating} onclick={() => void chooseDataStorage()}>
                  <FolderCog data-icon="inline-start" />
                  {tr('更改位置…')}
                </Button>
              </div>
              <div class="ml-11 rounded-md border bg-muted/20 px-3 py-2 font-mono text-[10px] text-muted-foreground">
                {dataStorage?.selected_path ?? tr('正在读取数据存储位置…')}
              </div>
            </section>
          </Tabs.Content>

          <Tabs.Content value="notifications" class="m-0 space-y-8 p-6 outline-none">
            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <BellRing class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('系统弹窗')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('新请求到达时使用 Windows、macOS 或 Linux 的系统消息通知。')}
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={$notificationPopupEnabled}
                aria-label={tr('系统弹窗')}
                class={[
                  'relative h-[22px] w-10 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                  $notificationPopupEnabled ? 'bg-primary' : 'bg-input',
                ]}
                onclick={() => void togglePopupNotifications(!$notificationPopupEnabled)}
              >
                <span
                  class={[
                    'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform',
                    $notificationPopupEnabled ? 'translate-x-5' : 'translate-x-0',
                  ]}
                ></span>
              </button>
              {#if notificationPermissionError}
                <div class="col-span-2 flex items-center gap-3 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2">
                  <img src={rambellePermission} alt="" class="size-14 shrink-0 object-contain" aria-hidden="true" />
                  <p class="m-0 text-xs text-destructive">{notificationPermissionError}</p>
                </div>
              {/if}
            </section>

            <section class="border-b pb-8">
              <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8">
                <div class="flex gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <Volume2 class="size-4" />
                  </span>
                  <div>
                    <h3 class="m-0 text-sm font-medium">{tr('声音提醒')}</h3>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('声音与系统弹窗相互独立；即使弹窗权限关闭也可以响铃。')}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={$notificationSoundEnabled}
                  aria-label={tr('声音提醒')}
                  class={[
                    'relative h-[22px] w-10 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                    $notificationSoundEnabled ? 'bg-primary' : 'bg-input',
                  ]}
                  onclick={() => setNotificationSoundEnabled(!$notificationSoundEnabled)}
                >
                  <span
                    class={[
                      'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform',
                      $notificationSoundEnabled ? 'translate-x-5' : 'translate-x-0',
                    ]}
                  ></span>
                </button>
              </div>

              {#if $notificationSoundEnabled}
                <div class="ml-11 mt-5 grid gap-5 rounded-md border bg-muted/20 p-4">
                  <div class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('提示音')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('选择新请求到达时播放的声音，并可立即试听。')}
                      </span>
                    </div>
                    <div class="flex items-center gap-2">
                      <Select.Root
                        type="single"
                        value={$notificationSound}
                        onValueChange={(value: string) => setNotificationSound(value as NotificationSound)}
                      >
                        <Select.Trigger class="min-w-0 flex-1">
                          {soundLabel($notificationSound)}
                        </Select.Trigger>
                        <Select.Content>
                          <Select.Item value="chime" label={tr('清脆双音')} />
                          <Select.Item value="soft" label={tr('柔和提示')} />
                          <Select.Item value="alert" label={tr('醒目提示')} />
                        </Select.Content>
                      </Select.Root>
                      <Button
                        variant="outline"
                        size="icon"
                        aria-label={tr('试听提示音')}
                        title={tr('试听提示音')}
                        onclick={() => void playNotificationSound($notificationSound, $notificationVolume)}
                      >
                        <Play />
                      </Button>
                    </div>
                  </div>

                  <div class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('音量')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('调整提示音音量。')}
                      </span>
                    </div>
                    <div class="flex items-center gap-3">
                      <input
                        type="range"
                        min="0"
                        max="100"
                        step="5"
                        value={$notificationVolume}
                        class="min-w-0 flex-1 accent-primary"
                        aria-label={tr('音量')}
                        oninput={(event) =>
                          setNotificationVolume(Number((event.currentTarget as HTMLInputElement).value))}
                      />
                      <span class="w-9 text-right text-[10px] tabular-nums text-muted-foreground">
                        {$notificationVolume}%
                      </span>
                    </div>
                  </div>
                </div>
              {/if}
            </section>
          </Tabs.Content>

          <Tabs.Content value="voice" class="m-0 space-y-8 p-6 outline-none">
            <section class="grid grid-cols-[minmax(0,1fr)_280px] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <Mic class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('麦克风')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('选择 Ramble 录音使用的输入设备。')}
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
                    {$speechInputDevice || tr('系统默认麦克风')}
                  </Select.Trigger>
                  <Select.Content>
                    <Select.Item value="__default__" label={tr('系统默认麦克风')} />
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
                      <h3 class="m-0 text-sm font-medium">{tr('转录模型')}</h3>
                      {#if selectedSpeechModel}
                        <Badge variant={selectedSpeechModel.installed ? 'secondary' : 'outline'}>
                          {selectedSpeechModel.installed ? tr('已安装') : tr('未安装')}
                        </Badge>
                      {/if}
                    </div>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('选择用于 Ramble 语音输入的本地模型；每个模型可单独下载或删除。')}
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
                      : tr('正在读取模型…')}
                  </Select.Trigger>
                  <Select.Content>
                    {#each speechModels as model (model.id)}
                      <Select.Item
                        value={model.id}
                        label={`${speechModelDisplayName($locale, model.id, model.display_name)}${model.installed ? ` · ${tr('已安装')}` : ''}`}
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
                        <Badge variant="outline">
                          {selectedSpeechModel.streaming ? tr('流式实时') : tr('VAD 分段 · 非流式')}
                        </Badge>
                        <span class="text-[10px] text-muted-foreground">
                          {Math.round(selectedSpeechModel.size_bytes / 1024 / 1024)} MB · {speechModelLanguages($locale, selectedSpeechModel.id, selectedSpeechModel.languages).join(' / ')}
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
                        {tr('模型许可')}：{selectedSpeechModel.license}
                      </p>
                    </div>
                    {#if selectedSpeechModel.installed}
                      <Button variant="outline" size="sm" disabled={modelBusy} onclick={deleteSpeechModel}>
                        <Trash2 data-icon="inline-start" />{tr('删除')}
                      </Button>
                    {:else}
                      <Button size="sm" disabled={modelBusy} onclick={downloadSpeechModel}>
                        {#if modelBusy}
                          <LoaderCircle class="animate-spin" data-icon="inline-start" />
                        {:else}
                          <Download data-icon="inline-start" />
                        {/if}
                        {modelBusy ? tr('下载中…') : modelError ? tr('重试下载') : tr('下载模型')}
                      </Button>
                    {/if}
                  </div>
                  {#if modelBusy && modelProgress?.model_id === selectedSpeechModel.id}
                    <div class="mt-3">
                      <div class="mb-1 flex justify-between text-[10px] text-muted-foreground">
                        <span>{tr('正在下载并校验…')}</span>
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

            <section class="flex items-start gap-3">
              <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                <Volume2 class="size-4" />
              </span>
              <div class="min-w-0 flex-1">
                <h3 class="m-0 text-sm font-medium">{tr('语音活动检测（VAD）')}</h3>
                <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                  {tr('SenseVoice 和 FunASR-Nano 使用内置 Silero VAD 自动切分长录音；X-ASR 仍按流式端点分段。')}
                </p>
                <div class="mt-4 grid gap-5 rounded-md border bg-muted/20 p-4">
                  <div class="grid grid-cols-[minmax(0,1fr)_280px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('声音阈值')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('环境嘈杂时调高；轻声讲话经常漏检时调低。')}
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
                        aria-label={tr('VAD 声音阈值')}
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
                      <strong class="block text-xs font-medium">{tr('静音分段')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('连续静音达到该时长后，将当前语音段送入非流式模型。')}
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
                        aria-label={tr('VAD 静音分段时长')}
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
          </Tabs.Content>

          <Tabs.Content value="adapters" class="m-0 space-y-8 p-6 outline-none">
            <section class="border-b pb-8">
              <div class="flex items-start gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground [&_svg]:size-4">
                  {@html piLogoSvg}
                </span>
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <h3 class="m-0 text-sm font-medium">{tr('Pi 原生适配器')}</h3>
                    <Badge variant="secondary">{tr('原生等待')}</Badge>
                  </div>
                  <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                    {tr('Pi package 通过本地 JSON API 请求、查询、等待和取消；等待发生在 Pi 工具调用内。')}
                  </p>
                </div>
                <Button disabled={installingPi || !isTauri} onclick={installPiPackage}>
                  {#if installingPi}
                    <LoaderCircle class="animate-spin" data-icon="inline-start" />
                    {tr('正在安装…')}
                  {:else}
                    <Download data-icon="inline-start" />
                    {tr('安装')}
                  {/if}
                </Button>
              </div>
              {#if piInstallMessage}
                <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                  <CheckCircle2 />
                  <Alert.Title>{tr('安装完成')}</Alert.Title>
                  <Alert.Description class="whitespace-pre-wrap">{piInstallMessage}</Alert.Description>
                </Alert.Root>
              {/if}
              {#if piInstallError}
                <Alert.Root variant="destructive" class="mt-4">
                  <Alert.Title>{tr('安装失败')}</Alert.Title>
                  <Alert.Description>{piInstallError}</Alert.Description>
                </Alert.Root>
              {/if}
            </section>

            <section class="border-b pb-8">
              <div class="flex items-start gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground [&_svg]:size-4">
                  <Bot class="size-4" />
                </span>
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <h3 class="m-0 text-sm font-medium">{tr('DeepSeek Harness 原生适配器')}</h3>
                    <Badge variant="secondary">{tr('原生等待')}</Badge>
                  </div>
                  <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                    {tr('Cordis 插件通过本地 JSON API 请求、查询、等待和取消；等待发生在 dsh 工具调用内，并向全局 skill 目录安装 ramble 引导。')}
                  </p>
                </div>
                <Button disabled={installingDsh || !isTauri} onclick={installDshPackage}>
                  {#if installingDsh}
                    <LoaderCircle class="animate-spin" data-icon="inline-start" />
                    {tr('正在安装…')}
                  {:else}
                    <Download data-icon="inline-start" />
                    {tr('安装')}
                  {/if}
                </Button>
              </div>
              {#if dshInstallMessage}
                <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                  <CheckCircle2 />
                  <Alert.Title>{tr('安装完成')}</Alert.Title>
                  <Alert.Description class="whitespace-pre-wrap">{dshInstallMessage}</Alert.Description>
                </Alert.Root>
              {/if}
              {#if dshInstallError}
                <Alert.Root variant="destructive" class="mt-4">
                  <Alert.Title>{tr('安装失败')}</Alert.Title>
                  <Alert.Description>{dshInstallError}</Alert.Description>
                </Alert.Root>
              {/if}
            </section>

            <section>
              <Collapsible.Root bind:open={genericAdapterOpen}>
                <div class="flex items-start gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <TerminalSquare class="size-4" />
                  </span>
                  <div class="min-w-0 flex-1">
                    <div class="flex flex-wrap items-center gap-2">
                      <h3 class="m-0 text-sm font-medium">{tr('通用 MCP 适配器')}</h3>
                      <Badge variant="outline">{tr('手动继续')}</Badge>
                    </div>
                    <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                      {tr('为支持 MCP 的宿主提供反馈工具；提交或取消后由 Resume Prompt 引导用户继续宿主会话。')}
                    </p>
                  </div>
                  <div class="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      disabled={loadingHosts || installing || !isTauri}
                      aria-label={tr('重新检测')}
                      title={tr('重新检测')}
                      onclick={refreshHosts}
                    >
                      <RefreshCw class={loadingHosts ? 'animate-spin' : ''} />
                    </Button>
                    <Collapsible.Trigger>
                      {#snippet child({ props })}
                        <Button
                          {...props}
                          variant="ghost"
                          size="icon-sm"
                          aria-label={genericAdapterOpen ? tr('收起') : tr('展开')}
                        >
                          <ChevronDown
                            class={[
                              'transition-transform',
                              genericAdapterOpen ? 'rotate-180' : '',
                            ]}
                          />
                        </Button>
                      {/snippet}
                    </Collapsible.Trigger>
                  </div>
                </div>

                <Collapsible.Content class="pt-4">
                  {#if loadingHosts}
                    <div class="flex h-24 items-center justify-center gap-2 text-xs text-muted-foreground">
                      <LoaderCircle class="size-4 animate-spin" />
                      {tr('正在检测 Coding 工具…')}
                    </div>
                  {:else if hosts.length === 0}
                    <p class="m-0 border-y py-5 text-center text-xs text-muted-foreground">
                      {isTauri ? tr('没有检测到支持的宿主') : tr('请在桌面应用中管理适配器')}
                    </p>
                  {:else}
                    <div class="divide-y border-y">
                      {#each hosts as host (host.id)}
                        <label
                          class={[
                            'flex min-h-12 items-center gap-3 px-2 py-2 text-xs transition-colors',
                            host.installed
                              ? 'cursor-pointer hover:bg-muted/60'
                              : 'cursor-not-allowed opacity-50',
                          ]}
                        >
                          <input
                            type="checkbox"
                            class="size-3.5 accent-primary"
                            checked={selectedIds.has(host.id)}
                            disabled={!host.installed || installing}
                            onchange={() => toggleHost(host)}
                          />
                          <span class="grid size-5 shrink-0 place-items-center [&_svg]:size-4">
                            {@html host.iconSvg}
                          </span>
                          <span class="min-w-0 flex-1">
                            <strong class="block truncate font-medium">{host.name}</strong>
                            <span class="block truncate text-[10px] text-muted-foreground" title={host.configPath}>
                              {host.configPath}
                            </span>
                          </span>
                          <Badge variant={host.configured ? 'secondary' : 'outline'}>
                            {host.configured
                              ? tr('已配置')
                              : host.installed
                                ? tr('已检测')
                                : tr('未检测到')}
                          </Badge>
                        </label>
                      {/each}
                    </div>
                  {/if}

                  <div class="mt-3 flex items-center justify-between gap-4">
                    <p class="m-0 text-[10px] leading-4 text-muted-foreground">
                      {tr('只更新 RambleDesk 的 MCP 条目；不会覆盖宿主中的其他配置。')}
                    </p>
                    <Button
                      disabled={selectedCount === 0 || installing || !isTauri}
                      onclick={installSelected}
                    >
                      {#if installing}
                        <LoaderCircle class="animate-spin" data-icon="inline-start" />
                      {:else}
                        <PlugZap data-icon="inline-start" />
                      {/if}
                      {selectedCount > 0
                        ? tr('配置所选（{count}）', { count: selectedCount })
                        : tr('选择宿主')}
                    </Button>
                  </div>

                  {#if installMessage}
                    <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                      <CheckCircle2 />
                      <Alert.Title>{tr('配置完成')}</Alert.Title>
                      <Alert.Description>{installMessage}</Alert.Description>
                    </Alert.Root>
                  {/if}
                  {#if installError}
                    <Alert.Root variant="destructive" class="mt-4">
                      <Alert.Title>{tr('配置失败')}</Alert.Title>
                      <Alert.Description>{installError}</Alert.Description>
                    </Alert.Root>
                  {/if}
                </Collapsible.Content>
              </Collapsible.Root>
            </section>

            <Collapsible.Root bind:open={configurationOpen} class="border-t pt-5">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <strong class="block text-xs font-medium">{tr('通用 MCP 配置')}</strong>
                  <span class="block text-[10px] text-muted-foreground">
                    {tr('仅用于手动配置和故障排查。')}
                  </span>
                </div>
                <Collapsible.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} variant="ghost" size="sm">
                      {configurationOpen ? tr('收起') : tr('查看')}
                      <ChevronDown
                        data-icon="inline-end"
                        class={['transition-transform', configurationOpen ? 'rotate-180' : '']}
                      />
                    </Button>
                  {/snippet}
                </Collapsible.Trigger>
              </div>
              <Collapsible.Content class="pt-3">
                <Alert.Root>
                  <ShieldCheck />
                  <Alert.Title>{tr('本机凭证')}</Alert.Title>
                  <Alert.Description>
                    {tr('配置中包含仅限本机使用的访问令牌，请勿发送给他人。')}
                  </Alert.Description>
                </Alert.Root>
                <pre class="mt-3 max-h-44 overflow-auto rounded-md border bg-muted/45 p-3 text-[10px] leading-4">{mcpConfiguration}</pre>
                <div class="mt-2 flex items-center justify-end gap-2">
                  {#if copyState === 'error'}
                    <span class="text-[10px] text-destructive">{tr('无法访问剪贴板，请手动复制')}</span>
                  {/if}
                  <Button variant="outline" size="sm" onclick={copyConfiguration}>
                    {#if copyState === 'copied'}
                      <Check data-icon="inline-start" />
                      {tr('已复制')}
                    {:else}
                      <Clipboard data-icon="inline-start" />
                      {tr('复制配置')}
                    {/if}
                  </Button>
                </div>
              </Collapsible.Content>
            </Collapsible.Root>
          </Tabs.Content>

          <Tabs.Content value="about" class="m-0 p-6 outline-none">
            <AboutSettings installBlocked={updateInstallBlocked} />
          </Tabs.Content>
        </ScrollArea>
      </div>
    </Tabs.Root>
  </Dialog.Content>
</Dialog.Root>

{#if storageMigrating}
  <div class="fixed inset-0 z-[100] grid place-items-center bg-black/45 p-6 backdrop-blur-sm">
    <div class="w-full max-w-md rounded-xl border bg-background p-5 shadow-2xl">
      <div class="flex items-center gap-3">
        <LoaderCircle class="size-5 animate-spin text-primary" />
        <div>
          <h3 class="m-0 text-sm font-medium">{tr('正在迁移数据')}</h3>
          <p class="m-0 mt-1 text-xs text-muted-foreground">{tr('请勿退出 RambleDesk。迁移完成后需要重启。')}</p>
        </div>
      </div>
      <div class="mt-5 h-2 overflow-hidden rounded-full bg-muted">
        <div class="h-full bg-primary transition-[width]" style={`width: ${storageMigration && storageMigration.total > 0 ? Math.min(100, storageMigration.copied / storageMigration.total * 100) : 2}%`}></div>
      </div>
      <p class="m-0 mt-2 text-right text-[10px] text-muted-foreground">
        {storageMigration && storageMigration.total > 0 ? `${Math.round(storageMigration.copied / storageMigration.total * 100)}%` : tr('正在扫描旧数据…')}
      </p>
    </div>
  </div>
{/if}
