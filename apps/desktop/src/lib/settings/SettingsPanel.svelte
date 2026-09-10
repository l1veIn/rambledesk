<script lang="ts">
  import {
    ArchiveRestore,
    BellRing,
    FolderCog,
    Globe2,
    Languages,
    ListChecks,
    LoaderCircle,
    Mic,
    MonitorCog,
    Info,
    Keyboard,
    Palette,
    PlugZap,
    Rocket,
    ShieldCheck,
    Sparkles,
    TerminalSquare,
    } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import * as Alert from '$lib/components/ui/alert'
  import { Button } from '$lib/components/ui/button'
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import AgentSettingsSection from '$lib/agents/AgentSettingsSection.svelte'
  import AdaptersSettings from '$lib/settings/AdaptersSettings.svelte'
  import NotificationsSettings from '$lib/settings/NotificationsSettings.svelte'
  import VoiceSettings from '$lib/settings/VoiceSettings.svelte'
  import AppearanceSettingsPanel from '$lib/appearance/AppearanceSettingsPanel.svelte'
  import { agentText } from '$lib/agents/agentI18n'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import { toast } from '$lib/components/ui/sonner'
  import AboutSettings from '$lib/settings/AboutSettings.svelte'
  import MacPermissions from '$lib/settings/MacPermissions.svelte'
  import PostProcessingSettings from '$lib/settings/PostProcessingSettings.svelte'
  import ShortcutSettings from '$lib/settings/ShortcutSettings.svelte'
  import WebAccessSettings from '$lib/settings/WebAccessSettings.svelte'
  import appIcon from '../../assets/rambledesk-app-icon.webp'
  import * as Select from '$lib/components/ui/select'
  import * as Tabs from '$lib/components/ui/tabs'
  import { t } from '$lib/i18n'
  import {
    DEFAULT_SPEECH_MODEL_ID,
    autoOpenTaskBrief,
    locale,
    setAutoOpenTaskBrief,
    setLocale,
    type SpeechModelId,
  } from '$lib/preferences'
  import {
    resolveSettingsSection,
    settingsSectionAvailability,
  } from '$lib/workspace/settingsCapabilitySections'
  import { applySettingsSectionCommand } from '$lib/workspace/settingsSectionCommand'
  import type { DshHostStatus } from '$lib/capabilities/workbenchCapabilities'
  import type { SettingsSection } from '../domain/settingsSection'

  const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

  type Section = SettingsSection

  export let mcpConfiguration = ''
  export let initialSection: Section = 'general'
  export let sectionSelectionEpoch = 0
  export let initialAgentConfigId: string | undefined = undefined
  export let initialAgentAdvanced = false
  export let onRestartOnboarding: () => void = () => {}
  export let onOpenArchived: () => void = () => {}
  export let onOpenRambelleProfile: () => void = () => {}
  export let updateInstallBlocked = false
  export let capabilities: WorkbenchCapabilities = unavailableCapabilities
  export let transport: ApplicationTransport

  // The navigation label and page heading describe the same section.
  const sections = {
    'general': { title: 'General', description: 'Preferences', icon: MonitorCog },
    'web-access': { title: 'Web Access', description: 'Local browser server', icon: Globe2 },
    'agents': { title: 'Agents', description: 'Agent configurations', icon: TerminalSquare },
    'appearance': { title: 'Appearance', description: 'Preferences', icon: Palette },
    'adapters': { title: 'External adapters', description: 'Feedback from external agents', icon: PlugZap },
    'permissions': { title: 'Permissions', description: 'System permissions', icon: ShieldCheck },
    'notifications': { title: 'Notifications', description: 'Alert methods', icon: BellRing },
    'voice': { title: 'Voice', description: 'Voice input', icon: Mic },
    'post-processing': { title: 'Post-processing', description: 'Draft and submission transforms', icon: Sparkles },
    'shortcuts': { title: 'Shortcuts', description: 'Global shortcut keys', icon: Keyboard },
    'about': { title: 'About', description: 'Project information', icon: Info },
  } satisfies Record<Section, { title: string; description: string; icon: typeof MonitorCog }>
  const sectionOrder = Object.keys(sections) as Section[]
  $: sectionDetails = sections[activeSection]

  function sectionText(section: Section, text: string) {
    return section === 'agents' ? agentText($locale, text) : tr(text)
  }

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

  const platform = capabilities.windowControls.implementation.platform()
  const sectionAvailability = settingsSectionAvailability(capabilities.manifest, platform)
  const initialSectionResolution = resolveSettingsSection(initialSection, sectionAvailability)
  let activeSection: Section = initialSectionResolution.activeSection
  let sectionCommandState = {
    activeSection: initialSection,
    appliedEpoch: sectionSelectionEpoch,
  }
  let initialDesktopOnlyNoticePending = initialSectionResolution.showDesktopOnlyNotice
  let dataStorage: DataStorageView | null = null
  let storageMessage = ''
  let storageError = ''
  let storageMigration: StorageMigrationProgress | null = null
  let storageMigrating = false
  let unlistenStorageProgress: (() => void) | null = null
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
    if (dataStorageSettingsAvailable) {
      void refreshDataStorage()
      unlistenStorageProgress = capabilities.dataStorageAdministration.implementation.onProgress(
        (progress) => (storageMigration = { ...progress }),
        () => undefined,
      )
    }
    return () => {
      unlistenStorageProgress?.()
    }
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
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
    <p>{tr('Manage interface preferences and agent connections.')}</p>
  </div>

    {#key $locale}
    <Tabs.Root
      bind:value={activeSection}
      orientation="vertical"
      class="settings-layout grid h-full min-h-0 grid-cols-[184px_minmax(0,1fr)] gap-0"
    >
      <aside class="settings-navigation flex min-h-0 flex-col border-r bg-muted/35 p-3">
        <div class="flex h-12 shrink-0 items-center gap-2 px-2">
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
          class="mt-3 flex min-h-0 w-full flex-1 flex-col items-stretch justify-start gap-1 overflow-x-hidden overflow-y-auto bg-transparent p-0"
        >
          {#each sectionOrder.filter((section) => sectionAvailability[section]) as section (section)}
            {@const Icon = sections[section].icon}
            <Tabs.Trigger value={section} title={sectionText(section, sections[section].title)} class="h-9 w-full flex-none justify-start px-2.5 group-data-[orientation=vertical]/tabs:after:right-0">
              <Icon data-icon="inline-start" />
              {sectionText(section, sections[section].title)}
            </Tabs.Trigger>
          {/each}
        </Tabs.List>

      </aside>

      <div class="settings-content flex min-h-0 min-w-0 flex-col">
        <header class="flex h-16 shrink-0 items-center border-b px-6">
          <div>
            <p class="m-0 text-[10px] font-medium uppercase text-muted-foreground">
              {sectionText(activeSection, sectionDetails.description)}
            </p>
            <h2 class="m-0 mt-0.5 text-base font-semibold">
              {sectionText(activeSection, sectionDetails.title)}
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

            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <ListChecks class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium" id="auto-open-task-brief-label">{tr('Automatically preview waiting requests')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground" id="auto-open-task-brief-description">
                    {tr('When opening a waiting request, switch to its Task brief tab automatically. You can still open the preview manually when this is off.')}
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={$autoOpenTaskBrief}
                aria-labelledby="auto-open-task-brief-label"
                aria-describedby="auto-open-task-brief-description"
                class={`relative h-6 w-11 shrink-0 rounded-full transition-colors focus-visible:outline-2 focus-visible:outline-ring ${$autoOpenTaskBrief ? 'bg-primary' : 'bg-muted-foreground/30'}`}
                onclick={() => setAutoOpenTaskBrief(!$autoOpenTaskBrief)}
              >
                <span class={`absolute top-0.5 left-0.5 size-5 rounded-full bg-background shadow-sm transition-transform ${$autoOpenTaskBrief ? 'translate-x-5' : ''}`}></span>
              </button>
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
                    {tr('Review storage, voice, permissions, agent connections, notifications, and Cooking, then start a new session.')}
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
            <NotificationsSettings {capabilities} />
          </Tabs.Content>
          {/if}

          {#if sectionAvailability.voice}
          <Tabs.Content value="voice" class="m-0 space-y-8 p-6 outline-none">
            <VoiceSettings {capabilities} />
          </Tabs.Content>
          {/if}

          {#if sectionAvailability['web-access']}
            <Tabs.Content value="web-access" class="m-0 p-6 outline-none">
              {#if activeSection === 'web-access'}<WebAccessSettings {capabilities} />{/if}
            </Tabs.Content>
          {/if}

          <Tabs.Content value="appearance" class="m-0 p-6 outline-none">
            {#if activeSection === 'appearance'}<AppearanceSettingsPanel />{/if}
          </Tabs.Content>

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
            <AdaptersSettings
              {capabilities}
              active={activeSection === 'adapters'}
              initialConfiguration={mcpConfiguration}
              onOpenAgents={() => (activeSection = 'agents')}
            />
          </Tabs.Content>
          {/if}

          <Tabs.Content value="agents" class="m-0 p-6 outline-none">
            {#if activeSection === 'agents'}
              {#key sectionSelectionEpoch}
                <AgentSettingsSection {transport} initialConfigId={initialAgentConfigId} initialAdvanced={initialAgentAdvanced} />
              {/key}
            {/if}
          </Tabs.Content>

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
