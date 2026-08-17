<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { open } from '@tauri-apps/plugin-dialog'
  import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification'
  import {
    BellRing,
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
    Sparkles,
    TerminalSquare,
    Trash2,
    Upload,
    Volume2,
    X,
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
  import MacPermissions from '$lib/MacPermissions.svelte'
  import appIcon from '../assets/rambledesk-app-icon.webp'
  import rambellePermission from '../assets/rambelle-states/state-permission.webp'
  import piLogoSvg from '../assets/pi-logo.svg?raw'
  import dshLogoSvg from '../assets/dsh-logo.svg?raw'
  import * as Select from '$lib/components/ui/select'
  import * as Tabs from '$lib/components/ui/tabs'
  import { DEFAULT_COOKING_SYSTEM_PROMPT } from '$lib/cooking'
  import { t } from '$lib/i18n'
  import { currentDesktopPlatform } from '$lib/platform'
  import {
    speechModelDescription,
    speechModelDisplayName,
    speechModelLanguages,
  } from '$lib/speechModelLabels'
  import {
    decodeCustomSoundBytes,
    discardCustomSoundCache,
    MAX_CUSTOM_SOUND_SECONDS,
    playNotificationSound,
  } from '$lib/notifications'
  import {
    cookingApiKey,
    cookingBaseUrl,
    cookingEnabled,
    cookingModel,
    cookingProvider,
    cookingReasoningEffort,
    cookingSystemPrompt,
    customNotificationSound,
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
    setCookingSystemPrompt,
    setCustomNotificationSound,
    setLocale,
    setNotificationPopupEnabled,
    setNotificationSound,
    setNotificationSoundEnabled,
    setNotificationVolume,
    setSpeechHotwords,
    setSpeechInputDevice,
    setSpeechModelId,
    setSpeechVadSilenceMs,
    setSpeechVadThreshold,
    setThemePreference,
    speechHotwords,
    speechInputDevice,
    speechModelId,
    speechVadSilenceMs,
    speechVadThreshold,
    themePreference,
    type CookingProvider,
    type CookingReasoningEffort,
    type CustomNotificationSound,
    type NotificationSound,
    type SpeechModelId,
    type ThemePreference,
  } from '$lib/preferences'

  type Section = 'general' | 'permissions' | 'notifications' | 'voice' | 'adapters' | 'about'

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
    hotwords_supported: boolean
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

  type NotificationSoundImportView = {
    id: string
    name: string
    bytes: number[]
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
  let customSoundBusy = false
  let customSoundError = ''
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
  let hotwordDraft = ''
  let unlistenModelProgress: UnlistenFn | null = null
  let unlistenStorageProgress: UnlistenFn | null = null
  let hasMacPermissions = false
  const isTauri = '__TAURI_INTERNALS__' in window
  const isMac = currentDesktopPlatform() === 'macOS'
  const isWindows = currentDesktopPlatform() === 'Windows'

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
      void refreshMacPermissionPresence()
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

  async function refreshMacPermissionPresence() {
    try {
      const permissions = await invoke<{ id: string; status: string }[]>('list_macos_permissions')
      hasMacPermissions = permissions.length > 0
    } catch {
      hasMacPermissions = false
    }
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
    if (modelBusy || !selectedSpeechModel || !confirm(tr('Delete the local speech model?'))) return
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
        ? tr('Data migrated. The new storage location takes effect after restarting RambleDesk.')
        : tr('This data storage location is already active.')
      toast.success(tr('Storage settings updated'), { description: storageMessage })
    } catch (cause) {
      storageError = messageFrom(cause)
      toast.error(tr('Storage settings failed'), { description: storageError })
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
      installMessage = tr('Generic MCP adapter config was written to {count} tools. Restart them to apply the change.', {
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
        tr('Pi native adapter installed; restart your Pi session to apply.') +
        (output.trim() ? `\n${output.trim()}` : '')
      if (output.trim().length === 0) {
        piInstallMessage += `\n${tr('The first install can take about ten seconds; please wait.')}`
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
        'DeepSeek Harness native adapter installed ({count} profile(s): {profiles}); restart dsh to apply.',
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
    if (isWindows) {
      setNotificationPopupEnabled(false)
      notificationPermissionError = tr(
        'Current unsigned Windows builds cannot show system banners. RambleDesk will not try to send them. Watch the inbox badge and use sound alerts instead.',
      )
      return
    }
    if (!isTauri) {
      notificationPermissionError = tr('System notifications are available only in the desktop app.')
      return
    }
    try {
      const permission = (await isPermissionGranted()) ? 'granted' : await requestPermission()
      if (permission === 'granted') {
        setNotificationPopupEnabled(true)
      } else {
        setNotificationPopupEnabled(false)
        notificationPermissionError = tr(
          'The operating system did not grant notification permission. Open System Settings → Notifications → RambleDesk and allow banners.',
        )
      }
    } catch (cause) {
      setNotificationPopupEnabled(false)
      notificationPermissionError = messageFrom(cause)
    }
  }

  function soundLabel(sound: NotificationSound) {
    if (sound === 'soft') return tr('Soft chime')
    if (sound === 'alert') return tr('Attention alert')
    if (sound === 'hakimi') return tr('Hakimi FM')
    if (sound === 'custom') return tr('Custom audio')
    return tr('Bright chime')
  }

  async function chooseCustomSound() {
    if (customSoundBusy || !isTauri) return
    customSoundBusy = true
    customSoundError = ''
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          { name: tr('Audio files'), extensions: ['mp3', 'wav', 'ogg', 'm4a', 'aac'] },
        ],
      })
      if (typeof selected !== 'string') return
      const imported = await invoke<NotificationSoundImportView>('import_notification_sound', {
        path: selected,
      })
      let duration = 0
      try {
        const decoded = await decodeCustomSoundBytes(imported.id, imported.bytes)
        duration = decoded.duration
      } catch {
        await invoke('remove_notification_sound', { id: imported.id }).catch(() => {})
        customSoundError = tr('Could not decode this audio file. Try a different one.')
        return
      }
      if (duration > MAX_CUSTOM_SOUND_SECONDS) {
        await invoke('remove_notification_sound', { id: imported.id }).catch(() => {})
        customSoundError = tr('Audio exceeds the 10-second limit. Trim it and try again.')
        return
      }
      // Cleanup of the previous sound happens only after validation succeeded.
      await invoke('commit_notification_sound', { id: imported.id }).catch(() => {})
      const next: CustomNotificationSound = { id: imported.id, name: imported.name }
      setCustomNotificationSound(next)
      setNotificationSound('custom')
    } catch (cause) {
      customSoundError = messageFrom(cause)
    } finally {
      customSoundBusy = false
    }
  }

  async function removeCustomSound() {
    const current = $customNotificationSound
    if (!current) return
    discardCustomSoundCache()
    if (isTauri) {
      await invoke('remove_notification_sound', { id: current.id }).catch(() => {})
    }
    setCustomNotificationSound(null)
    setNotificationSound('chime')
    customSoundError = ''
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
      <Dialog.Title>{tr('Settings')}</Dialog.Title>
      <Dialog.Description id="settings-description">
        {tr('Manage interface preferences and host adapters.')}
      </Dialog.Description>
    </Dialog.Header>

    {#key $locale}
    <Tabs.Root
      bind:value={activeSection}
      orientation="vertical"
      class="grid h-full min-h-0 grid-cols-[184px_minmax(0,1fr)] gap-0"
    >
      <aside class="flex min-h-0 flex-col border-r bg-muted/35 p-3">
        <div class="flex h-12 items-center gap-2 px-2">
          <img
            src={appIcon}
            alt=""
            draggable="false"
            class="size-7 shrink-0 rounded-md object-cover"
          />
          <div class="min-w-0">
            <strong class="block text-xs font-semibold">RambleDesk</strong>
            <span class="block text-[10px] text-muted-foreground">{tr('Settings')}</span>
          </div>
        </div>

        <Tabs.List
          variant="line"
          class="mt-3 flex w-full flex-col items-stretch gap-1 bg-transparent p-0"
        >
          <Tabs.Trigger value="general" class="h-9 w-full justify-start px-2.5">
            <MonitorCog data-icon="inline-start" />
            {tr('General')}
          </Tabs.Trigger>
          {#if hasMacPermissions}
            <Tabs.Trigger value="permissions" class="h-9 w-full justify-start px-2.5">
              <ShieldCheck data-icon="inline-start" />
              {tr('Permissions')}
            </Tabs.Trigger>
          {/if}
          <Tabs.Trigger value="notifications" class="h-9 w-full justify-start px-2.5">
            <BellRing data-icon="inline-start" />
            {tr('Notifications')}
          </Tabs.Trigger>
          <Tabs.Trigger value="voice" class="h-9 w-full justify-start px-2.5">
            <Mic data-icon="inline-start" />
            {tr('Voice')}
          </Tabs.Trigger>
          <Tabs.Trigger value="adapters" class="h-9 w-full justify-start px-2.5">
            <PlugZap data-icon="inline-start" />
            <span class="flex-1 text-left">{tr('Adapters')}</span>
            {#if installedHosts.length > 0}
              <Badge variant="secondary" class="h-5 px-1.5 text-[9px]">
                {installedHosts.length}
              </Badge>
            {/if}
          </Tabs.Trigger>
          <Tabs.Trigger value="about" class="h-9 w-full justify-start px-2.5">
            <Info data-icon="inline-start" />
            {tr('About')}
          </Tabs.Trigger>
        </Tabs.List>

        <div class="mt-auto flex gap-2 border-t pt-3 text-[10px] leading-4 text-muted-foreground">
          <ShieldCheck class="mt-0.5 size-3.5 shrink-0" />
          <span>{tr('Adapter configuration is written only to your user directory and preserves other adapters.')}</span>
        </div>
      </aside>

      <div class="flex min-h-0 min-w-0 flex-col">
        <header class="flex h-16 shrink-0 items-center border-b px-6">
          <div>
            <p class="m-0 text-[10px] font-medium uppercase text-muted-foreground">
              {activeSection === 'general'
                ? tr('Preferences')
                : activeSection === 'permissions'
                  ? tr('System permissions')
                  : activeSection === 'notifications'
                    ? tr('Alert methods')
                    : activeSection === 'voice'
                      ? tr('Voice input')
                      : activeSection === 'adapters'
                        ? tr('Host adapters')
                        : tr('Project information')}
            </p>
            <h2 class="m-0 mt-0.5 text-base font-semibold">
              {activeSection === 'general'
                ? tr('General')
                : activeSection === 'permissions'
                  ? tr('Permissions')
                  : activeSection === 'notifications'
                    ? tr('Notifications')
                    : activeSection === 'voice'
                      ? tr('Voice')
                      : activeSection === 'adapters'
                        ? tr('Adapters')
                        : tr('About')}
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
                  <h3 class="m-0 text-sm font-medium">{tr('Language')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('Choose the RambleDesk interface language.')}
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
                  <h3 class="m-0 text-sm font-medium">{tr('Appearance')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('Choose a light or dark appearance, or follow the operating system.')}
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
                    ? tr('System')
                    : $themePreference === 'light'
                      ? tr('Light')
                      : tr('Dark')}
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="system" label={tr('System')} />
                  <Select.Item value="light" label={tr('Light')} />
                  <Select.Item value="dark" label={tr('Dark')} />
                </Select.Content>
              </Select.Root>
            </section>

            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <Rocket class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('Getting started')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('Review initial setup for storage, voice, adapters, notifications, and Cooking.')}
                  </p>
                </div>
              </div>
              <Button variant="outline" onclick={onRestartOnboarding}>
                <Rocket data-icon="inline-start" />
                {tr('Run getting started again')}
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
                      <Badge variant="outline">{tr('Optional')}</Badge>
                    </div>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('Use an LLM to turn the Ramble draft into formal feedback before submission; the uncooked source remains in the feedback package.')}
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
                    <label for="cooking-provider" class="text-xs font-medium">{tr('Model provider')}</label>
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
                            : tr('OpenAI-compatible service')}
                      </Select.Trigger>
                      <Select.Content>
                        <Select.Item value="deepseek" label="DeepSeek" />
                        <Select.Item value="openai" label="OpenAI" />
                        <Select.Item value="compatible" label={tr('OpenAI-compatible service')} />
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
                    <label for="cooking-model" class="text-xs font-medium">{tr('Model name')}</label>
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
                    <label for="cooking-reasoning" class="text-xs font-medium">{tr('Reasoning effort')}</label>
                    <Select.Root
                      type="single"
                      value={$cookingReasoningEffort}
                      onValueChange={(value: string) =>
                        setCookingReasoningEffort(value as CookingReasoningEffort)}
                    >
                      <Select.Trigger id="cooking-reasoning" class="w-full">
                        {$cookingReasoningEffort === 'none'
                          ? tr('None')
                          : $cookingReasoningEffort === 'minimal'
                            ? 'minimal'
                            : $cookingReasoningEffort}
                      </Select.Trigger>
                      <Select.Content>
                        <Select.Item value="none" label={tr('None')} />
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
                  <div class="grid grid-cols-[140px_minmax(0,1fr)] items-start gap-4">
                    <label for="cooking-system-prompt" class="pt-2 text-xs font-medium">{tr('Cooking prompt')}</label>
                    <div class="grid gap-2">
                      <textarea
                        id="cooking-system-prompt"
                        rows="8"
                        value={$cookingSystemPrompt || DEFAULT_COOKING_SYSTEM_PROMPT}
                        class="min-h-40 w-full resize-y rounded-md border bg-background px-3 py-2 font-mono text-[11px] leading-5 outline-none focus-visible:ring-2 focus-visible:ring-ring"
                        oninput={(event) =>
                          setCookingSystemPrompt((event.currentTarget as HTMLTextAreaElement).value)}
                      ></textarea>
                      <div class="flex items-center justify-between gap-3">
                        <p class="m-0 text-[10px] leading-4 text-muted-foreground">
                          {tr('This prompt is sent with every cook, along with the request title, what happened, and the action list. Keep attachment:// image references if you edit it.')}
                        </p>
                        <Button
                          variant="outline"
                          size="xs"
                          disabled={!$cookingSystemPrompt}
                          onclick={() => setCookingSystemPrompt('')}
                        >
                          {tr('Reset to default')}
                        </Button>
                      </div>
                    </div>
                  </div>
                  <p class="m-0 text-[10px] leading-4 text-muted-foreground">
                    {tr('The API key is stored only in local settings on this device and is never written to feedback packages. Cooking sends the uncooked body to the selected model provider.')}
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
                    <h3 class="m-0 text-sm font-medium">{tr('Data storage location')}</h3>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('Feedback attachments, published packages, and speech models are stored here; the database and credentials remain in the system location.')}
                    </p>
                  </div>
                </div>
                <Button variant="outline" disabled={!isTauri || storageMigrating} onclick={() => void chooseDataStorage()}>
                  <FolderCog data-icon="inline-start" />
                  {tr('Change location…')}
                </Button>
              </div>
              <div class="ml-11 rounded-md border bg-muted/20 px-3 py-2 font-mono text-[10px] text-muted-foreground">
                {dataStorage?.selected_path ?? tr('Loading data storage location…')}
              </div>
            </section>
          </Tabs.Content>


          <Tabs.Content value="permissions" class="m-0 space-y-8 p-6 outline-none">
            <section class="border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <ShieldCheck class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('macOS permissions')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('Screen capture and voice transcription require macOS permissions. Grant them now or later in Settings → Permissions.')}
                  </p>
                </div>
              </div>
              <div class="ml-11 mt-5">
                <MacPermissions />
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
                  <h3 class="m-0 text-sm font-medium">{tr('System notifications')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {#if isWindows}
                      {tr('Current unsigned Windows builds cannot show system banners. RambleDesk will not try to send them. Watch the inbox badge and use sound alerts instead.')}
                    {:else}
                      {tr('On macOS, allow RambleDesk in System Settings → Notifications. Banners may stay hidden while this window is focused; check Notification Center if a request arrives while you are already here.')}
                    {/if}
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={$notificationPopupEnabled}
                aria-label={tr('System notifications')}
                disabled={isWindows}
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
                    <h3 class="m-0 text-sm font-medium">{tr('Sound alerts')}</h3>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('Sound is independent of system notifications and can play even when popup permission is disabled.')}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={$notificationSoundEnabled}
                  aria-label={tr('Sound alerts')}
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
                      <strong class="block text-xs font-medium">{tr('Alert sound')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('Choose the sound played for new requests and preview it immediately.')}
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
                          <Select.Item value="chime" label={tr('Bright chime')} />
                          <Select.Item value="soft" label={tr('Soft chime')} />
                          <Select.Item value="alert" label={tr('Attention alert')} />
                          <Select.Item value="hakimi" label={tr('Hakimi FM')} />
                          <Select.Item value="custom" label={tr('Custom audio')} />
                        </Select.Content>
                      </Select.Root>
                      <Button
                        variant="outline"
                        size="icon"
                        aria-label={tr('Preview alert sound')}
                        title={tr('Preview alert sound')}
                        onclick={() =>
                          void playNotificationSound(
                            $notificationSound,
                            $notificationVolume,
                            $notificationSound === 'custom' ? $customNotificationSound : null,
                          )}
                      >
                        <Play />
                      </Button>
                    </div>
                  </div>

                  {#if $notificationSound === 'custom'}
                    <div class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-6">
                      <div>
                        <strong class="block text-xs font-medium">{tr('Custom audio')}</strong>
                        <span class="mt-0.5 block text-[10px] text-muted-foreground">
                          {tr('Choose an audio file as the alert sound (up to 10 seconds and 5 MiB).')}
                        </span>
                      </div>
                      <div class="flex items-center gap-2">
                        <Button
                          variant="outline"
                          size="sm"
                          class="min-w-0 flex-1"
                          disabled={!isTauri || customSoundBusy}
                          onclick={() => void chooseCustomSound()}
                        >
                          {#if customSoundBusy}
                            <LoaderCircle class="animate-spin" data-icon="inline-start" />
                          {:else}
                            <Upload data-icon="inline-start" />
                          {/if}
                          {tr('Choose audio…')}
                        </Button>
                        {#if $customNotificationSound}
                          <Button
                            variant="outline"
                            size="icon"
                            aria-label={tr('Remove custom audio')}
                            title={tr('Remove custom audio')}
                            onclick={() => void removeCustomSound()}
                          >
                            <Trash2 />
                          </Button>
                        {/if}
                      </div>
                    </div>
                    {#if $customNotificationSound}
                      <p class="m-0 break-all text-[10px] text-muted-foreground">
                        {tr('Current alert sound: {name}', { name: $customNotificationSound.name })}
                      </p>
                    {/if}
                    {#if customSoundError}
                      <p class="m-0 text-xs text-destructive">{customSoundError}</p>
                    {/if}
                    {#if !isTauri}
                      <p class="m-0 text-[10px] text-muted-foreground">
                        {tr('Custom alert sounds are available only in the desktop app.')}
                      </p>
                    {/if}
                  {/if}

                  <div class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('Volume')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('Adjust alert sound volume.')}
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
                        aria-label={tr('Volume')}
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
                        label={`${speechModelDisplayName($locale, model.id, model.display_name)}${model.installed ? ` · ${tr('Installed')}` : ''}`}
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
                          {selectedSpeechModel.streaming ? tr('Live streaming') : tr('VAD segmented · Non-streaming')}
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
                  {tr('SenseVoice and FunASR-Nano use bundled Silero VAD to split long recordings; X-ASR continues to use streaming endpoints.')}
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
          </Tabs.Content>

          <Tabs.Content value="adapters" class="m-0 space-y-8 p-6 outline-none">
            <section class="border-b pb-8">
              <div class="flex items-start gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground [&_svg]:size-4">
                  {@html piLogoSvg}
                </span>
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <h3 class="m-0 text-sm font-medium">{tr('Pi native adapter')}</h3>
                    <Badge variant="secondary">{tr('Native wait')}</Badge>
                  </div>
                  <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                    {tr('The Pi package uses the local JSON API to request, get, wait, and cancel; waiting stays inside the Pi tool call.')}
                  </p>
                </div>
                <Button disabled={installingPi || !isTauri} onclick={installPiPackage}>
                  {#if installingPi}
                    <LoaderCircle class="animate-spin" data-icon="inline-start" />
                    {tr('Installing…')}
                  {:else}
                    <Download data-icon="inline-start" />
                    {tr('Install')}
                  {/if}
                </Button>
              </div>
              {#if piInstallMessage}
                <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                  <CheckCircle2 />
                  <Alert.Title>{tr('Installation complete')}</Alert.Title>
                  <Alert.Description class="whitespace-pre-wrap">{piInstallMessage}</Alert.Description>
                </Alert.Root>
              {/if}
              {#if piInstallError}
                <Alert.Root variant="destructive" class="mt-4">
                  <Alert.Title>{tr('Installation failed')}</Alert.Title>
                  <Alert.Description>{piInstallError}</Alert.Description>
                </Alert.Root>
              {/if}
            </section>

            <section class="border-b pb-8">
              <div class="flex items-start gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground [&_svg]:size-4">
                  {@html dshLogoSvg}
                </span>
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <h3 class="m-0 text-sm font-medium">{tr('DeepSeek Harness native adapter')}</h3>
                    <Badge variant="secondary">{tr('Native wait')}</Badge>
                  </div>
                  <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                    {tr('The Cordis plugin uses the local JSON API to request, get, wait, and cancel; waiting stays inside the dsh tool call, and it installs the ramble guide into the global skill directory.')}
                  </p>
                </div>
                <Button disabled={installingDsh || !isTauri} onclick={installDshPackage}>
                  {#if installingDsh}
                    <LoaderCircle class="animate-spin" data-icon="inline-start" />
                    {tr('Installing…')}
                  {:else}
                    <Download data-icon="inline-start" />
                    {tr('Install')}
                  {/if}
                </Button>
              </div>
              {#if dshInstallMessage}
                <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                  <CheckCircle2 />
                  <Alert.Title>{tr('Installation complete')}</Alert.Title>
                  <Alert.Description class="whitespace-pre-wrap">{dshInstallMessage}</Alert.Description>
                </Alert.Root>
              {/if}
              {#if dshInstallError}
                <Alert.Root variant="destructive" class="mt-4">
                  <Alert.Title>{tr('Installation failed')}</Alert.Title>
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
                      <h3 class="m-0 text-sm font-medium">{tr('Generic MCP adapter')}</h3>
                      <Badge variant="outline">{tr('Manual continuation')}</Badge>
                    </div>
                    <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                      {tr('Provides feedback tools to MCP-capable hosts; after submission or cancellation, a Resume Prompt guides the user back to the host session.')}
                    </p>
                  </div>
                  <div class="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      disabled={loadingHosts || installing || !isTauri}
                      aria-label={tr('Detect again')}
                      title={tr('Detect again')}
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
                          aria-label={genericAdapterOpen ? tr('Collapse') : tr('Expand')}
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
                      {tr('Detecting coding tools…')}
                    </div>
                  {:else if hosts.length === 0}
                    <p class="m-0 border-y py-5 text-center text-xs text-muted-foreground">
                      {isTauri ? tr('No supported hosts detected') : tr('Manage adapters in the desktop app')}
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
                              ? tr('Configured')
                              : host.installed
                                ? tr('Detected')
                                : tr('Not detected')}
                          </Badge>
                        </label>
                      {/each}
                    </div>
                  {/if}

                  <div class="mt-3 flex items-center justify-between gap-4">
                    <p class="m-0 text-[10px] leading-4 text-muted-foreground">
                      {tr('Only the RambleDesk MCP entry is updated; other host configuration is preserved.')}
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
                        ? tr('Configure selected ({count})', { count: selectedCount })
                        : tr('Select host')}
                    </Button>
                  </div>

                  {#if installMessage}
                    <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                      <CheckCircle2 />
                      <Alert.Title>{tr('Configuration complete')}</Alert.Title>
                      <Alert.Description>{installMessage}</Alert.Description>
                    </Alert.Root>
                  {/if}
                  {#if installError}
                    <Alert.Root variant="destructive" class="mt-4">
                      <Alert.Title>{tr('Configuration failed')}</Alert.Title>
                      <Alert.Description>{installError}</Alert.Description>
                    </Alert.Root>
                  {/if}
                </Collapsible.Content>
              </Collapsible.Root>
            </section>

            <Collapsible.Root bind:open={configurationOpen} class="border-t pt-5">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <strong class="block text-xs font-medium">{tr('Generic MCP configuration')}</strong>
                  <span class="block text-[10px] text-muted-foreground">
                    {tr('For manual configuration and troubleshooting only.')}
                  </span>
                </div>
                <Collapsible.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} variant="ghost" size="sm">
                      {configurationOpen ? tr('Collapse') : tr('View')}
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
                  <Alert.Title>{tr('Local credentials')}</Alert.Title>
                  <Alert.Description>
                    {tr('This configuration contains a local-only access token. Do not share it.')}
                  </Alert.Description>
                </Alert.Root>
                <pre class="mt-3 max-h-44 overflow-auto rounded-md border bg-muted/45 p-3 text-[10px] leading-4">{mcpConfiguration}</pre>
                <div class="mt-2 flex items-center justify-end gap-2">
                  {#if copyState === 'error'}
                    <span class="text-[10px] text-destructive">{tr('Clipboard unavailable; copy the configuration manually')}</span>
                  {/if}
                  <Button variant="outline" size="sm" onclick={copyConfiguration}>
                    {#if copyState === 'copied'}
                      <Check data-icon="inline-start" />
                      {tr('Copied')}
                    {:else}
                      <Clipboard data-icon="inline-start" />
                      {tr('Copy configuration')}
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
    {/key}
  </Dialog.Content>
</Dialog.Root>

{#if storageMigrating}
  <div class="fixed inset-0 z-[100] grid place-items-center bg-black/45 p-6 backdrop-blur-sm">
    <div class="w-full max-w-md rounded-xl border bg-background p-5 shadow-2xl">
      <div class="flex items-center gap-3">
        <LoaderCircle class="size-5 animate-spin text-primary" />
        <div>
          <h3 class="m-0 text-sm font-medium">{tr('Migrating data')}</h3>
          <p class="m-0 mt-1 text-xs text-muted-foreground">{tr('Do not quit RambleDesk. Restart after migration completes.')}</p>
        </div>
      </div>
      <div class="mt-5 h-2 overflow-hidden rounded-full bg-muted">
        <div class="h-full bg-primary transition-[width]" style={`width: ${storageMigration && storageMigration.total > 0 ? Math.min(100, storageMigration.copied / storageMigration.total * 100) : 2}%`}></div>
      </div>
      <p class="m-0 mt-2 text-right text-[10px] text-muted-foreground">
        {storageMigration && storageMigration.total > 0 ? `${Math.round(storageMigration.copied / storageMigration.total * 100)}%` : tr('Scanning existing data…')}
      </p>
    </div>
  </div>
{/if}
