<script lang="ts">
  import {
    ArchiveRestore,
    BellRing,
    Check,
    CheckCircle2,
    ChevronDown,
    Clipboard,
    Download,
    FolderCog,
    Globe2,
    Languages,
    LoaderCircle,
    Mic,
    MonitorCog,
    Info,
    Keyboard,
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
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import type {
    WebAccessStatus,
    WorkbenchCapabilities,
  } from '$lib/capabilities/workbenchCapabilities'
  import {
    settleWebAccessMutation,
    webAccessDisplayState as resolveWebAccessDisplayState,
    webAccessRunningActionsEnabled,
    webAccessToggleTarget,
    type WebAccessTransientPhase,
  } from '$lib/capabilities/webAccessState'
  import { resolveSupportedSpeechModelId } from '$lib/capabilities/speechModelSelection'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import { toast } from '$lib/components/ui/sonner'
  import AboutSettings from '$lib/AboutSettings.svelte'
  import MacPermissions from '$lib/MacPermissions.svelte'
  import PostProcessingSettings from '$lib/PostProcessingSettings.svelte'
  import ShortcutSettings from '$lib/ShortcutSettings.svelte'
  import appIcon from '../assets/rambledesk-app-icon.webp'
  import rambellePermission from '../assets/rambelle-states/state-permission.webp'
  import piLogoSvg from '../assets/pi-logo.svg?raw'
  import dshLogoSvg from '../assets/dsh-logo.svg?raw'
  import * as Select from '$lib/components/ui/select'
  import * as Tabs from '$lib/components/ui/tabs'
  import { t } from '$lib/i18n'
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
    DEFAULT_SPEECH_MODEL_ID,
    customNotificationSound,
    locale,
    notificationPopupEnabled,
    notificationSound,
    notificationSoundEnabled,
    notificationVolume,
    setCustomNotificationSound,
    setLocale,
    setNotificationPopupEnabled,
    setNotificationSound,
    setNotificationSoundEnabled,
    setNotificationVolume,
    setSpeechHotwords,
    setSpeechInputDevice,
    setSpeechConfirmBeforeWrite,
    setSpeechModelId,
    setSpeechVadSilenceMs,
    setSpeechVadThreshold,
    setThemePreference,
    speechHotwords,
    speechInputDevice,
    speechConfirmBeforeWrite,
    speechOverlayEnabled,
    speechOverlayOpacity,
    setSpeechOverlayEnabled,
    setSpeechOverlayOpacity,
    speechModelId,
    speechVadSilenceMs,
    speechVadThreshold,
    themePreference,
    type CustomNotificationSound,
    type NotificationSound,
    type SpeechModelId,
    type ThemePreference,
  } from '$lib/preferences'
  import {
    resolveSettingsSection,
    settingsSectionAvailability,
  } from '$lib/workspace/settingsCapabilitySections'
  import { applySettingsSectionCommand } from '$lib/workspace/settingsSectionCommand'
  import type { SettingsSection } from '$lib/workbench/types'

  const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

  type Section = SettingsSection

  export let mcpConfiguration = ''
  export let initialSection: Section = 'general'
  export let sectionSelectionEpoch = 0
  export let onRestartOnboarding: () => void = () => {}
  export let onOpenArchived: () => void = () => {}
  export let onOpenRambelleProfile: () => void = () => {}
  export let updateInstallBlocked = false
  export let capabilities: WorkbenchCapabilities = unavailableCapabilities

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
    missing_files: readonly string[]
    streaming: boolean
    hotwords_supported: boolean
    languages: readonly string[]
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

  type PiPackageStatus = {
    cliAvailable: boolean
    installed: boolean
    sourceCount: number
    restartRequired: boolean
  }

  const platform = capabilities.windowControls.implementation.platform()
  const sectionAvailability = settingsSectionAvailability(capabilities.manifest, platform)
  const initialSectionResolution = resolveSettingsSection(initialSection, sectionAvailability)
  let activeSection: Section = initialSectionResolution.activeSection
  let sectionCommandState = {
    activeSection: initialSection,
    appliedEpoch: sectionSelectionEpoch,
  }
  let initialDesktopOnlyNoticePending = initialSectionResolution.showDesktopOnlyNotice
  let hosts: McpHostView[] = []
  let selectedIds = new Set<string>()
  let loadingHosts = true
  let installing = false
  let installMessage = ''
  let installError = ''
  let piStatus: PiPackageStatus | null = null
  let piStatusLoading = true
  let piAction: 'install' | 'uninstall' | null = null
  let piLastAction: 'status' | 'install' | 'uninstall' = 'status'
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
  let unlistenModelProgress: (() => void) | null = null
  let unlistenStorageProgress: (() => void) | null = null
  let webAccessStatus: WebAccessStatus | null = null
  let webAccessPhase: WebAccessTransientPhase = 'loading'
  let webAccessActionError = ''
  let webAccessRefreshError = ''
  const isWindows = platform === 'Windows'
  const onboardingAvailable =
    capabilities.dataStorageAdministration.status.availability !== 'unavailable' ||
    capabilities.speech.status.availability !== 'unavailable' ||
    capabilities.hostIntegrationAdministration.status.availability !== 'unavailable' ||
    capabilities.notifications.status.availability !== 'unavailable' ||
    capabilities.webAccessAdministration.status.availability !== 'unavailable'
  const dataStorageSettingsAvailable =
    capabilities.dataStorageAdministration.status.availability !== 'unavailable' &&
    capabilities.serverPaths.status.availability !== 'unavailable'

  $: installedHosts = hosts.filter((host) => host.installed)
  $: selectedCount = selectedIds.size
  $: selectedSpeechModel =
    speechModels.find((model) => model.id === $speechModelId) ?? speechModels[0] ?? null
  $: webAccessDisplayState = resolveWebAccessDisplayState(webAccessStatus, webAccessPhase)
  $: webAccessRunningActions = webAccessRunningActionsEnabled(webAccessStatus, webAccessPhase)
  $: webAccessError = webAccessStatus?.state === 'failed'
    ? webAccessStatus.failure.message
    : webAccessRefreshError || webAccessActionError
  $: {
    const nextSectionCommandState = applySettingsSectionCommand(
      sectionCommandState,
      initialSection,
      sectionSelectionEpoch,
    )
    if (nextSectionCommandState !== sectionCommandState) {
      sectionCommandState = nextSectionCommandState
      const resolution = resolveSettingsSection(
        nextSectionCommandState.activeSection,
        sectionAvailability,
      )
      activeSection = resolution.activeSection
      if (resolution.showDesktopOnlyNotice) {
        toast.info(tr('This settings section is available only in the desktop app.'))
      }
    }
  }

  onMount(() => {
    if (initialDesktopOnlyNoticePending) {
      initialDesktopOnlyNoticePending = false
      toast.info(tr('This settings section is available only in the desktop app.'))
    }
    if (sectionAvailability.adapters) {
      void refreshHosts()
      void refreshPiStatus()
    } else {
      loadingHosts = false
      piStatusLoading = false
    }
    if (dataStorageSettingsAvailable) {
      void refreshDataStorage()
      unlistenStorageProgress = capabilities.dataStorageAdministration.implementation.onProgress(
        (progress) => (storageMigration = { ...progress }),
        () => undefined,
      )
    }
    if (sectionAvailability.voice) {
      void refreshSpeechDevices()
      void refreshSpeechModels()
      unlistenModelProgress = capabilities.speech.implementation.onModelProgress(
        (progress) => (modelProgress = { ...progress }),
        () => undefined,
      )
    }
    if (capabilities.webAccessAdministration.status.availability !== 'unavailable') {
      void refreshWebAccessStatus()
    }
    return () => {
      unlistenModelProgress?.()
      unlistenStorageProgress?.()
    }
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function refreshWebAccessStatus() {
    if (capabilities.webAccessAdministration.status.availability === 'unavailable') return
    webAccessPhase = 'loading'
    webAccessActionError = ''
    webAccessRefreshError = ''
    try {
      webAccessStatus = await capabilities.webAccessAdministration.implementation.status()
    } catch (cause) {
      webAccessStatus = null
      webAccessRefreshError = messageFrom(cause)
    } finally {
      webAccessPhase = null
    }
  }

  async function toggleWebAccess() {
    if (
      capabilities.webAccessAdministration.status.availability === 'unavailable' ||
      webAccessPhase !== null
    ) return
    const enabled = webAccessToggleTarget(webAccessStatus)
    if (enabled === null) return
    webAccessPhase = enabled ? 'starting' : 'stopping'
    webAccessActionError = ''
    webAccessRefreshError = ''
    const result = await settleWebAccessMutation(
      capabilities.webAccessAdministration.implementation,
      enabled,
    )
    webAccessStatus = result.status
    webAccessPhase = null
    webAccessActionError = result.operationError ? messageFrom(result.operationError) : ''
    webAccessRefreshError = result.refreshError
      ? tr('Could not verify the current Web Access status: {error}', {
          error: messageFrom(result.refreshError),
        })
      : ''
  }

  async function openWebAccess() {
    if (!webAccessRunningActions) return
    try {
      await capabilities.webAccessAdministration.implementation.open()
    } catch (cause) {
      await refreshWebAccessAfterActionFailure(cause)
    }
  }

  async function copyWebAccessToken() {
    if (!webAccessRunningActions) return
    try {
      await capabilities.webAccessAdministration.implementation.copyToken()
      toast.success(tr('Web Access token copied.'))
    } catch (cause) {
      await refreshWebAccessAfterActionFailure(cause)
    }
  }

  async function refreshWebAccessAfterActionFailure(actionCause: unknown) {
    const actionError = messageFrom(actionCause)
    webAccessPhase = 'loading'
    webAccessRefreshError = ''
    try {
      webAccessStatus = await capabilities.webAccessAdministration.implementation.status()
      webAccessActionError = webAccessStatus.state === 'failed' ? '' : actionError
    } catch (refreshCause) {
      webAccessStatus = null
      webAccessActionError = actionError
      webAccessRefreshError = tr('Could not verify the current Web Access status: {error}', {
        error: messageFrom(refreshCause),
      })
    } finally {
      webAccessPhase = null
    }
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

  async function refreshDataStorage() {
    try {
      dataStorage = await capabilities.dataStorageAdministration.implementation.read()
    } catch (cause) {
      storageError = messageFrom(cause)
    }
  }

  async function chooseDataStorage() {
    storageError = ''
    storageMessage = ''
    try {
      const selected = await capabilities.serverPaths.implementation.chooseDirectory()
      if (!selected) return
      storageMigrating = true
      storageMigration = { copied: 0, total: 0 }
      dataStorage = await capabilities.dataStorageAdministration.implementation.select(selected)
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
      hosts = [...await capabilities.hostIntegrationAdministration.implementation.detectGenericMcpHosts()]
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
      const results = await capabilities.hostIntegrationAdministration.implementation.installGenericMcpHosts([...selectedIds])
      const changed = results.filter((result) => result.action !== 'unchanged').length
      if (changed > 0) {
        installMessage = tr('Generic MCP adapter config was written to {count} tools. Restart them to apply the change.', {
          count: changed,
        })
      } else {
        installMessage = tr('Generic MCP adapter config is already up to date for {count} tools.', {
          count: results.length,
        })
      }
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

  async function refreshPiStatus(reportError = true) {
    piStatusLoading = true
    try {
      piStatus = await capabilities.hostIntegrationAdministration.implementation.piStatus()
    } catch (cause) {
      piStatus = null
      if (reportError) {
        piLastAction = 'status'
        piInstallError = messageFrom(cause)
      }
    } finally {
      piStatusLoading = false
    }
  }

  async function installPiPackage() {
    if (piAction) return
    piAction = 'install'
    piLastAction = 'install'
    piInstallError = ''
    piInstallMessage = ''
    try {
      const output = await capabilities.hostIntegrationAdministration.implementation.installPi()
      piInstallMessage =
        tr('Pi native adapter installed; restart your Pi session to apply.') +
        (output.trim() ? `\n${output.trim()}` : '')
      if (output.trim().length === 0) {
        piInstallMessage += `\n${tr('The first install can take about ten seconds; please wait.')}`
      }
    } catch (cause) {
      piInstallError = messageFrom(cause)
    } finally {
      await refreshPiStatus(false)
      piAction = null
    }
  }

  async function uninstallPiPackage() {
    if (piAction || !piStatus?.installed || !confirm(tr('Uninstall the Pi native adapter?'))) return
    piAction = 'uninstall'
    piLastAction = 'uninstall'
    piInstallError = ''
    piInstallMessage = ''
    try {
      const output = await capabilities.hostIntegrationAdministration.implementation.uninstallPi()
      piInstallMessage =
        tr('Pi native adapter uninstalled; restart your Pi session to apply.') +
        (output.trim() ? `\n${output.trim()}` : '')
    } catch (cause) {
      piInstallError = messageFrom(cause)
    } finally {
      await refreshPiStatus(false)
      piAction = null
    }
  }

  async function installDshPackage() {
    if (installingDsh) return
    installingDsh = true
    dshInstallError = ''
    dshInstallMessage = ''
    try {
      const results = await capabilities.hostIntegrationAdministration.implementation.installDsh()
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
    if (capabilities.notifications.status.availability === 'unavailable') {
      notificationPermissionError = tr('System notifications are available only in the desktop app.')
      return
    }
    try {
      const currentPermission = await capabilities.notifications.implementation.permission()
      const permission = currentPermission === 'granted'
        ? 'granted'
        : await capabilities.notifications.implementation.requestPermission()
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
    if (customSoundBusy || capabilities.notifications.status.availability === 'unavailable') return
    customSoundBusy = true
    customSoundError = ''
    try {
      const selected = await capabilities.serverPaths.implementation.chooseFile({
        extensions: ['mp3', 'wav', 'ogg', 'm4a', 'aac'],
      })
      if (!selected) return
      const imported = await capabilities.notifications.implementation.importSound(selected)
      let duration = 0
      try {
        const decoded = await decodeCustomSoundBytes(imported.id, [...imported.bytes])
        duration = decoded.duration
      } catch {
        await capabilities.notifications.implementation.removeSound(imported.id).catch(() => {})
        customSoundError = tr('Could not decode this audio file. Try a different one.')
        return
      }
      if (duration > MAX_CUSTOM_SOUND_SECONDS) {
        await capabilities.notifications.implementation.removeSound(imported.id).catch(() => {})
        customSoundError = tr('Audio exceeds the 10-second limit. Trim it and try again.')
        return
      }
      // Cleanup of the previous sound happens only after validation succeeded.
      await capabilities.notifications.implementation.commitSound(imported.id).catch(() => {})
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
    if (capabilities.notifications.status.availability !== 'unavailable') {
      await capabilities.notifications.implementation.removeSound(current.id).catch(() => {})
    }
    setCustomNotificationSound(null)
    setNotificationSound('chime')
    customSoundError = ''
  }

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return String(cause)
  }
</script>

<div class="settings-workspace-root h-full min-h-0 overflow-hidden bg-background">
  <div class="sr-only">
    <h2>{tr('Settings')}</h2>
    <p>{tr('Manage interface preferences and host adapters.')}</p>
  </div>

    {#key $locale}
    <Tabs.Root
      bind:value={activeSection}
      orientation="vertical"
      class="settings-layout grid h-full min-h-0 grid-cols-[184px_minmax(0,1fr)] gap-0"
    >
      <aside class="settings-navigation flex min-h-0 flex-col border-r bg-muted/35 p-3">
        <div class="flex h-12 items-center gap-2 px-2">
          <img
            src={appIcon}
            alt=""
            draggable="false"
            class="size-7 shrink-0 rounded-md object-cover"
          />
          <div class="settings-brand-copy min-w-0">
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
          {#if sectionAvailability.permissions}
            <Tabs.Trigger value="permissions" class="h-9 w-full justify-start px-2.5">
              <ShieldCheck data-icon="inline-start" />
              {tr('Permissions')}
            </Tabs.Trigger>
          {/if}
          {#if sectionAvailability.notifications}
            <Tabs.Trigger value="notifications" class="h-9 w-full justify-start px-2.5">
              <BellRing data-icon="inline-start" />
              {tr('Notifications')}
            </Tabs.Trigger>
          {/if}
          {#if sectionAvailability.voice}
            <Tabs.Trigger value="voice" class="h-9 w-full justify-start px-2.5">
              <Mic data-icon="inline-start" />
              {tr('Voice')}
            </Tabs.Trigger>
          {/if}
          <Tabs.Trigger value="post-processing" class="h-9 w-full justify-start px-2.5">
            <Sparkles data-icon="inline-start" />
            {tr('Post-processing')}
          </Tabs.Trigger>
          {#if sectionAvailability.shortcuts}
            <Tabs.Trigger value="shortcuts" class="h-9 w-full justify-start px-2.5">
              <Keyboard data-icon="inline-start" />
              {tr('Shortcuts')}
            </Tabs.Trigger>
          {/if}
          {#if sectionAvailability.adapters}
            <Tabs.Trigger value="adapters" class="h-9 w-full justify-start px-2.5">
              <PlugZap data-icon="inline-start" />
              <span class="flex-1 text-left">{tr('Adapters')}</span>
              {#if installedHosts.length > 0}
                <Badge variant="secondary" class="h-5 px-1.5 text-[9px]">
                  {installedHosts.length}
                </Badge>
              {/if}
            </Tabs.Trigger>
          {/if}
          <Tabs.Trigger value="about" class="h-9 w-full justify-start px-2.5">
            <Info data-icon="inline-start" />
            {tr('About')}
          </Tabs.Trigger>
        </Tabs.List>

        {#if sectionAvailability.adapters}
        <div class="settings-navigation-note mt-auto flex gap-2 border-t pt-3 text-[10px] leading-4 text-muted-foreground">
          <ShieldCheck class="mt-0.5 size-3.5 shrink-0" />
          <span>{tr('Adapter configuration is written only to your user directory and preserves other adapters.')}</span>
        </div>
        {/if}
      </aside>

      <div class="settings-content flex min-h-0 min-w-0 flex-col">
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
                      : activeSection === 'post-processing'
                        ? tr('Draft and submission transforms')
                        : activeSection === 'shortcuts'
                          ? tr('Global shortcut keys')
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
                      : activeSection === 'post-processing'
                        ? tr('Post-processing')
                        : activeSection === 'shortcuts'
                          ? tr('Shortcuts')
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

            {#if onboardingAvailable}
            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8" aria-live="polite">
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
            {/if}

            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <ArchiveRestore class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('Archived content')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('View archived sessions and Ramble requests.')}
                  </p>
                </div>
              </div>
              <Button variant="outline" onclick={onOpenArchived}>
                <ArchiveRestore data-icon="inline-start" />
                {tr('View archived content')}
              </Button>
            </section>

            {#if capabilities.webAccessAdministration.status.availability !== 'unavailable'}
            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <Globe2 class="size-4" />
                </span>
                <div>
                  <div class="flex items-center gap-2">
                    <h3 class="m-0 text-sm font-medium">{tr('Web Access')}</h3>
                    <Badge
                      variant={webAccessDisplayState === 'failed'
                        ? 'destructive'
                        : webAccessDisplayState === 'running'
                          ? 'default'
                          : 'secondary'}
                    >
                      {webAccessDisplayState === 'loading'
                        ? tr('Loading…')
                        : webAccessDisplayState === 'starting'
                          ? tr('Starting…')
                          : webAccessDisplayState === 'stopping'
                            ? tr('Stopping…')
                            : webAccessDisplayState === 'running'
                              ? tr('Running')
                              : webAccessDisplayState === 'stopped'
                                ? tr('Stopped')
                                : webAccessDisplayState === 'failed'
                                  ? tr('Needs attention')
                                  : tr('Status unavailable')}
                    </Badge>
                  </div>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {webAccessStatus?.state === 'running'
                      ? tr('Available only in a browser on this computer at {url}.', {
                          url: webAccessStatus.url,
                        })
                      : webAccessStatus?.state === 'failed'
                        ? tr('Web Access is unavailable until the reported problem is resolved.')
                        : webAccessStatus?.state === 'stopped'
                          ? tr('Start a local browser Workbench. It stays off until you start it.')
                          : tr('The current Web Access status could not be verified.')}
                  </p>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('Stopping Web Access only closes browser access. Backend Runtime and Local Integration keep running.')}
                  </p>
                  {#if webAccessError}
                    <p class="m-0 mt-1 text-xs text-destructive" role="alert">{webAccessError}</p>
                  {/if}
                </div>
              </div>
              <div class="flex flex-wrap items-center justify-end gap-2">
                <Button
                  variant="ghost"
                  disabled={webAccessPhase !== null}
                  onclick={() => void refreshWebAccessStatus()}
                >
                  <RefreshCw
                    data-icon="inline-start"
                    class={webAccessPhase === 'loading' ? 'animate-spin' : ''}
                  />
                  {tr('Refresh')}
                </Button>
                {#if webAccessRunningActions}
                  <Button variant="outline" onclick={() => void copyWebAccessToken()}>
                    <Clipboard data-icon="inline-start" />
                    {tr('Copy token')}
                  </Button>
                  <Button variant="outline" onclick={() => void openWebAccess()}>
                    <Globe2 data-icon="inline-start" />
                    {tr('Open')}
                  </Button>
                {/if}
                <Button
                  variant={webAccessStatus?.state === 'running' ? 'destructive' : 'outline'}
                  disabled={webAccessPhase !== null || webAccessStatus === null}
                  onclick={() => void toggleWebAccess()}
                >
                  {#if webAccessPhase !== null}
                    <LoaderCircle data-icon="inline-start" class="animate-spin" />
                  {:else if webAccessStatus?.state === 'running'}
                    <X data-icon="inline-start" />
                  {:else}
                    <Play data-icon="inline-start" />
                  {/if}
                  {webAccessPhase === 'loading'
                    ? tr('Loading…')
                    : webAccessPhase === 'starting'
                      ? tr('Starting…')
                      : webAccessPhase === 'stopping'
                        ? tr('Stopping…')
                        : webAccessStatus?.state === 'running'
                          ? tr('Stop')
                          : tr('Start')}
                </Button>
              </div>
            </section>
            {/if}

            {#if dataStorageSettingsAvailable}
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
                <Button variant="outline" disabled={storageMigrating} onclick={() => void chooseDataStorage()}>
                  <FolderCog data-icon="inline-start" />
                  {tr('Change location…')}
                </Button>
              </div>
              <div class="ml-11 rounded-md border bg-muted/20 px-3 py-2 font-mono text-[10px] text-muted-foreground">
                {dataStorage?.selected_path ?? tr('Loading data storage location…')}
              </div>
            </section>
            {/if}
          </Tabs.Content>


          {#if sectionAvailability.permissions}
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
                <MacPermissions
                  systemPermissions={capabilities.systemPermissions}
                  notifications={capabilities.notifications}
                  windowControls={capabilities.windowControls}
                />
              </div>
            </section>
          </Tabs.Content>
          {/if}
          {#if sectionAvailability.notifications}
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
                            capabilities.notifications.implementation.readCustomSound,
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
                          disabled={capabilities.serverPaths.status.availability === 'unavailable' || customSoundBusy}
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
                    {#if capabilities.notifications.status.availability === 'unavailable'}
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
          {/if}

          {#if sectionAvailability.voice}
          <Tabs.Content value="voice" class="m-0 space-y-8 p-6 outline-none">
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
          </Tabs.Content>
          {/if}

          <Tabs.Content value="post-processing" class="m-0 p-6 outline-none">
            <PostProcessingSettings />
          </Tabs.Content>

          {#if sectionAvailability.shortcuts}
            <Tabs.Content value="shortcuts" class="m-0 space-y-8 p-6 outline-none">
              <ShortcutSettings globalShortcuts={capabilities.globalShortcuts} />
            </Tabs.Content>
          {/if}
          {#if sectionAvailability.adapters}
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
                    {#if piStatusLoading}
                      <Badge variant="outline">{tr('Checking…')}</Badge>
                    {:else if piStatus?.installed}
                      <Badge variant="secondary">{tr('Installed')}</Badge>
                    {:else}
                      <Badge variant="outline">{tr('Not installed')}</Badge>
                    {/if}
                    {#if piStatus && !piStatus.cliAvailable}
                      <Badge variant="outline">{tr('Pi CLI not detected')}</Badge>
                    {/if}
                  </div>
                  <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                    {tr('The Pi package uses the local JSON API to request, get, wait, and cancel; waiting stays inside the Pi tool call.')}
                  </p>
                  {#if piStatus && piStatus.sourceCount > 1}
                    <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                      {tr('{count} RambleDesk Pi package registrations detected. Uninstall removes all of them.', { count: piStatus.sourceCount })}
                    </p>
                  {/if}
                </div>
                <Button
                  variant={piStatus?.installed ? 'outline' : 'default'}
                  disabled={piAction !== null || piStatusLoading || capabilities.hostIntegrationAdministration.status.availability === 'unavailable' || piStatus?.cliAvailable === false}
                  onclick={() => void (piStatus?.installed ? uninstallPiPackage() : installPiPackage())}
                >
                  {#if piAction === 'install'}
                    <LoaderCircle class="animate-spin" data-icon="inline-start" />
                    {tr('Installing…')}
                  {:else if piAction === 'uninstall'}
                    <LoaderCircle class="animate-spin" data-icon="inline-start" />
                    {tr('Uninstalling…')}
                  {:else if piStatus?.installed}
                    <Trash2 data-icon="inline-start" />
                    {tr('Uninstall')}
                  {:else}
                    <Download data-icon="inline-start" />
                    {tr('Install')}
                  {/if}
                </Button>
              </div>
              {#if piInstallMessage}
                <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                  <CheckCircle2 />
                  <Alert.Title>
                    {piLastAction === 'uninstall' ? tr('Uninstallation complete') : tr('Installation complete')}
                  </Alert.Title>
                  <Alert.Description class="whitespace-pre-wrap">{piInstallMessage}</Alert.Description>
                </Alert.Root>
              {/if}
              {#if piInstallError}
                <Alert.Root variant="destructive" class="mt-4">
                  <Alert.Title>
                    {piLastAction === 'uninstall'
                      ? tr('Uninstallation failed')
                      : piLastAction === 'status'
                        ? tr('Status check failed')
                        : tr('Installation failed')}
                  </Alert.Title>
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
                <Button disabled={installingDsh || capabilities.hostIntegrationAdministration.status.availability === 'unavailable'} onclick={installDshPackage}>
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
                      disabled={loadingHosts || installing || capabilities.hostIntegrationAdministration.status.availability === 'unavailable'}
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
                      {capabilities.hostIntegrationAdministration.status.availability !== 'unavailable' ? tr('No supported hosts detected') : tr('Manage adapters in the desktop app')}
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
                      disabled={selectedCount === 0 || installing || capabilities.hostIntegrationAdministration.status.availability === 'unavailable'}
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
          {/if}

          <Tabs.Content value="about" class="m-0 p-6 outline-none">
            <AboutSettings
              installBlocked={updateInstallBlocked}
              softwareUpdates={capabilities.softwareUpdates}
              diagnostics={capabilities.diagnostics}
              serverPaths={capabilities.serverPaths}
              externalLinks={capabilities.externalLinks}
              windowControls={capabilities.windowControls}
              {onOpenRambelleProfile}
            />
          </Tabs.Content>
        </ScrollArea>
      </div>
    </Tabs.Root>
    {/key}
</div>

<style>
  .settings-workspace-root {
    container: settings-workspace / inline-size;
  }

  @container settings-workspace (max-width: 639px) {
    :global(.settings-layout) {
      grid-template-columns: 52px minmax(0, 1fr);
    }

    .settings-navigation {
      padding: 0.375rem;
    }

    .settings-brand-copy,
    .settings-navigation-note {
      display: none;
    }

    .settings-navigation :global([role='tab']) {
      justify-content: center;
      gap: 0;
      padding-inline: 0;
      font-size: 0;
    }

    .settings-navigation :global([role='tab'] [data-slot='badge']) {
      display: none;
    }

    .settings-content :global(header) {
      padding-inline: 1rem;
    }

    .settings-content :global([role='tabpanel']) {
      padding: 1rem;
    }

    .settings-content :global([class*='grid-cols-']) {
      grid-template-columns: minmax(0, 1fr) !important;
      align-items: stretch;
      gap: 1rem;
    }
  }
</style>

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
