<script lang="ts">
  import { flushClientDiagnostics, setClientDiagnosticsEnabled } from '$lib/diagnostics/clientDiagnostics'
  import { createDiagnosticSettingsController } from '$lib/diagnostics/diagnosticSettingsController'
  import { Download, ExternalLink, FileArchive, FolderOpen, GitBranch, LoaderCircle, RefreshCw, RotateCw, ShieldCheck, Sparkles, Trash2 } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import rambelleSticker from '../assets/rambelle-states/idle.webp'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import type {
    CapabilitySlot,
    DiagnosticsCapability,
    ExternalLinkCapability,
    ServerPathCapability,
    UpdaterCapability,
    WindowCapability,
  } from '$lib/capabilities/workbenchCapabilities'
  import { toast } from '$lib/components/ui/sonner'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { diagnosticExportView } from './nativePath'
  import {
    openUpdateDialog,
    updateState,
  } from '$lib/updater'

  const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

  export let installBlocked = false
  export let onOpenRambelleProfile: () => void = () => {}
  export let softwareUpdates: CapabilitySlot<UpdaterCapability> = unavailableCapabilities.softwareUpdates
  export let diagnostics: CapabilitySlot<DiagnosticsCapability> = unavailableCapabilities.diagnostics
  export let serverPaths: CapabilitySlot<ServerPathCapability> = unavailableCapabilities.serverPaths
  export let externalLinks: CapabilitySlot<ExternalLinkCapability> = unavailableCapabilities.externalLinks
  export let windowControls: CapabilitySlot<WindowCapability> = unavailableCapabilities.windowControls

  let version = '0.0.1'
  type DiagnosticScope = 'last_24_hours' | 'last_7_days' | 'all'

  let exporting: DiagnosticScope | null = null
  let lastExportPath = ''
  const diagnosticSettings = createDiagnosticSettingsController({
    readSettings: () => diagnostics.implementation.readSettings(),
    setEnabled: (enabled) => diagnostics.implementation.setEnabled(enabled),
    clear: () => diagnostics.implementation.clear(),
  }, { setClientEnabled: setClientDiagnosticsEnabled, flush: flushClientDiagnostics })
  $: updatesAvailable = softwareUpdates.status.availability !== 'unavailable'
  $: diagnosticsAvailable = diagnostics.status.availability !== 'unavailable'
  $: serverPathsAvailable = serverPaths.status.availability !== 'unavailable'
  $: externalLinksAvailable = externalLinks.status.availability !== 'unavailable'
  $: windowControlsAvailable = windowControls.status.availability !== 'unavailable'
  $: desktopPlatform = windowControls.implementation.platform()
  $: isMac = desktopPlatform === 'macOS'
  const projectUrl = 'https://github.com/l1veIn/rambledesk'
  const releasesUrl = `${projectUrl}/releases`

  onMount(async () => {
    if (diagnosticsAvailable) void diagnosticSettings.load()
    if (updatesAvailable) version = await softwareUpdates.implementation.version().catch(() => version)
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function openSafeUrl(url: string) {
    if (!externalLinksAvailable) {
      toast.info(tr('Opening external links is available only in the desktop app.'))
      return
    }
    try {
      await externalLinks.implementation.open(url)
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause)
      toast.error(tr('Could not open link'), { description: message })
    }
  }

  function openExternalLink(event: MouseEvent, url: string) {
    event.preventDefault()
    void openSafeUrl(url)
  }

  async function exportDiagnostics(scope: DiagnosticScope) {
    if (!diagnosticsAvailable || !serverPathsAvailable || exporting || $diagnosticSettings.busy) return
    exporting = scope
    try {
      const stamp = new Date().toISOString().slice(0, 10)
      const path = await serverPaths.implementation.chooseSaveFile({
        defaultName: `RambleDesk-diagnostics-${scope === 'all' ? 'all' : scope === 'last_24_hours' ? '24h' : '7d'}-${stamp}.zip`,
        extensions: ['zip'],
      })
      if (!path) return
      await flushClientDiagnostics()
      const exported = diagnosticExportView(await diagnostics.implementation.export(scope, path))
      lastExportPath = exported.path
      toast.success(tr('Diagnostic package exported'), {
        duration: 12_000,
        description: tr('{events} events · {requests} requests · {logs} log files', {
          events: exported.events,
          requests: exported.requests,
          logs: exported.logs,
        }),
        action: {
          label: tr('Show in folder'),
          onClick: (event) => {
            event.preventDefault()
            void revealExportedPath(exported.path)
          },
        },
      })
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause)
      toast.error(tr('Could not export the diagnostic package'), { description: message })
    } finally {
      exporting = null
    }
  }

  async function revealExportedPath(path: string) {
    if (!path) return
    try {
      await serverPaths.implementation.reveal(path)
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause)
      toast.error(tr('Could not show the file in the folder'), { description: message })
    }
  }

  $: progress =
    $updateState.total > 0
      ? Math.min(100, Math.round(($updateState.downloaded / $updateState.total) * 100))
      : 0
</script>

<div class="space-y-6">
  <section class="relative overflow-hidden rounded-xl border bg-gradient-to-br from-primary/8 via-background to-info/8 p-6">
    <div class="relative z-10 grid grid-cols-[minmax(0,1fr)_150px] items-center gap-6">
      <div>
        <div class="flex flex-wrap items-center gap-2">
          <h3 class="m-0 text-xl font-semibold tracking-tight">RambleDesk</h3>
          {#if updatesAvailable}<Badge variant="secondary">v{version}</Badge>{/if}
          {#if windowControlsAvailable}<Badge variant="outline">{desktopPlatform}</Badge>{/if}
        </div>
        <p class="m-0 mt-3 max-w-xl text-sm leading-6 text-muted-foreground">
          {tr('Let agents pause at key moments and request structured human feedback that can be resumed and archived.')}
        </p>
        <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">
          {tr('Feedback drafts, attachments, and packages stay on your device. The agent only receives results you explicitly submit or cancel.')}
        </p>
        <Button
          href={externalLinksAvailable ? projectUrl : undefined}
          target="_blank"
          rel="noreferrer"
          disabled={!externalLinksAvailable}
          title={!externalLinksAvailable ? tr('Opening external links is available only in the desktop app.') : ''}
          variant="link"
          class="mt-3 h-auto gap-1.5 p-0 text-xs"
          onclick={(event) => openExternalLink(event, projectUrl)}
        >
          <GitBranch data-icon="inline-start" />
          {tr('View GitHub repository')}
          <ExternalLink class="size-3" />
        </Button>
      </div>
      <img
        src={rambelleSticker}
        alt={tr('Rambelle waving sticker')}
        class="mx-auto h-36 w-36 object-contain drop-shadow-[0_16px_30px_rgba(59,130,246,0.2)]"
      />
    </div>
  </section>

  <section class="rounded-xl border p-5">
    <div class="flex items-center justify-between gap-6">
      <div class="flex min-w-0 items-center gap-3">
        <img
          src={rambelleSticker}
          alt=""
          draggable="false"
          class="size-12 shrink-0 rounded-xl object-contain"
        />
        <div class="min-w-0">
          <h3 class="m-0 text-sm font-medium">Rambelle</h3>
          <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
            {tr('View Rambelle’s story and character profile.')}
          </p>
        </div>
      </div>
      <Button
        variant="outline"
        class="shrink-0"
        onclick={onOpenRambelleProfile}
      >
        <Sparkles data-icon="inline-start" />
        {tr('View character profile')}
      </Button>
    </div>
  </section>

  {#if updatesAvailable}
  <section class="rounded-xl border p-5">
    <div class="flex items-start justify-between gap-6">
      <div>
        <h3 class="m-0 text-sm font-medium">{tr('Software updates')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('RambleDesk checks for updates after launch and shows what’s new when a version is available.')}
          {#if isMac}
            {' '}{tr('macOS builds are unsigned. Download a new DMG from GitHub Releases when you want to update.')}
          {/if}
        </p>
      </div>
      <Button
        variant="outline"
        disabled={!updatesAvailable || $updateState.status === 'checking' || $updateState.status === 'downloading'}
        onclick={() => void softwareUpdates.implementation.check({ prompt: true, forcePrompt: true })}
      >
        {#if $updateState.status === 'checking'}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
          {tr('Checking…')}
        {:else}
          <RefreshCw data-icon="inline-start" />
          {tr('Check for updates')}
        {/if}
      </Button>
    </div>

    <div class="mt-4 rounded-lg border bg-muted/25 p-4" aria-live="polite">
      {#if $updateState.status === 'idle'}
        <p class="m-0 text-xs text-muted-foreground">{tr('Updates have not been checked yet.')}</p>
      {:else if $updateState.status === 'checking'}
        <p class="m-0 text-xs text-muted-foreground">{tr('Connecting to the update server…')}</p>
      {:else if $updateState.status === 'up-to-date'}
        <div class="flex items-center gap-2 text-xs text-success">
          <ShieldCheck class="size-4" />
          {tr('You are up to date.')}
        </div>
      {:else if $updateState.status === 'available'}
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div>
            <strong class="block text-xs">{tr('Version v{version} is available', { version: $updateState.version })}</strong>
            {#if $updateState.message}
              <p class="m-0 mt-1 line-clamp-4 text-[10px] leading-4 text-muted-foreground">{$updateState.message}</p>
            {/if}
          </div>
          <div class="flex flex-wrap gap-2">
            <Button variant="outline" onclick={() => openUpdateDialog()}>
              {tr("What's new")}
            </Button>
            {#if desktopPlatform === 'Windows'}
              <Button
                disabled={installBlocked}
                title={installBlocked ? tr('Finish or cancel the current feedback before installing the update.') : ''}
                onclick={() => void softwareUpdates.implementation.install()}
              >
                <Download data-icon="inline-start" />
                {tr('Download and install')}
              </Button>
            {:else}
              <Button
                href={externalLinksAvailable ? releasesUrl : undefined}
                target="_blank"
                rel="noreferrer"
                disabled={!externalLinksAvailable}
                onclick={(event) => openExternalLink(event, releasesUrl)}
              >
                <ExternalLink data-icon="inline-start" />
                {tr('Open GitHub Releases')}
              </Button>
            {/if}
          </div>
        </div>
      {:else if $updateState.status === 'downloading'}
        <div>
          <div class="flex items-center justify-between gap-3 text-xs">
            <span>{tr('Downloading v{version}…', { version: $updateState.version })}</span>
            {#if $updateState.total > 0}<span>{progress}%</span>{/if}
          </div>
          <div class="mt-3 h-2 overflow-hidden rounded-full bg-muted">
            <div
              class={['h-full bg-primary transition-[width]', $updateState.total <= 0 ? 'animate-pulse' : '']}
              style={`width: ${$updateState.total > 0 ? progress : 35}%`}
            ></div>
          </div>
        </div>
      {:else if $updateState.status === 'ready'}
        <div class="flex flex-wrap items-center justify-between gap-3">
          <strong class="text-xs">{tr('v{version} is installed and will take effect after restart.', { version: $updateState.version })}</strong>
          {#if windowControlsAvailable}
            <Button
              disabled={installBlocked}
              title={installBlocked ? tr('Finish or cancel the current feedback before restarting.') : ''}
              onclick={() => void windowControls.implementation.restart()}
            >
              <RotateCw data-icon="inline-start" />
              {tr('Restart now')}
            </Button>
          {/if}
        </div>
      {:else if $updateState.status === 'error'}
        <div>
          <strong class="block text-xs text-destructive">{tr('Update check or installation failed')}</strong>
          <p class="m-0 mt-1 break-all text-[10px] leading-4 text-muted-foreground">{$updateState.message}</p>
        </div>
      {/if}
    </div>

    {#if desktopPlatform === 'Windows' && installBlocked && ($updateState.status === 'available' || $updateState.status === 'ready')}
      <p class="m-0 mt-3 text-[10px] leading-4 text-warning-foreground dark:text-warning">
        {tr('Feedback is in progress or has unsaved content. Update restart is disabled to prevent data loss.')}
      </p>
    {/if}
  </section>
  {/if}

  {#if diagnosticsAvailable}
  <section class="rounded-xl border p-5">
    <div>
      <h3 class="m-0 text-sm font-medium">{tr('Diagnostics')}</h3>
    </div>
    <div class="mt-4 flex items-start justify-between gap-4">
      <div>
        <h4 id="diagnostic-recording-label" class="m-0 text-xs font-medium">{tr('Record diagnostic information')}</h4>
        <p id="diagnostic-recording-description" class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('Keep local diagnostic events and logs to help investigate problems. Turning this off keeps existing diagnostics.')}
        </p>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={$diagnosticSettings.enabled === true}
        aria-labelledby="diagnostic-recording-label"
        aria-describedby="diagnostic-recording-description"
        disabled={$diagnosticSettings.enabled === null || $diagnosticSettings.busy !== null || exporting !== null}
        class={[
          'relative h-[22px] w-10 shrink-0 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:opacity-50',
          $diagnosticSettings.enabled === true ? 'bg-primary' : 'bg-input',
        ]}
        onclick={() => void diagnosticSettings.setEnabled(!$diagnosticSettings.enabled)}
      >
        <span class={[
          'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform',
          $diagnosticSettings.enabled === true ? 'translate-x-5' : 'translate-x-0',
        ]}></span>
      </button>
    </div>
    <div class="mt-4 flex flex-wrap items-center gap-3">
      <Button
        variant="outline"
        disabled={$diagnosticSettings.busy !== null || exporting !== null}
        onclick={() => void diagnosticSettings.clear()}
      >
        {#if $diagnosticSettings.busy === 'clearing'}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
        {:else}
          <Trash2 data-icon="inline-start" />
        {/if}
        {tr('Clear diagnostics')}
      </Button>
      {#if $diagnosticSettings.busy === 'loading' || $diagnosticSettings.busy === 'saving'}
        <span class="flex items-center gap-1.5 text-xs text-muted-foreground" role="status">
          <LoaderCircle class="size-3.5 animate-spin" />
          {tr($diagnosticSettings.busy === 'loading' ? 'Loading diagnostic settings…' : 'Saving diagnostic settings…')}
        </span>
      {/if}
    </div>
    <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">
      {tr('Clearing diagnostics keeps your sessions, feedback, attachments, and exported ZIP files.')}
      {#if $diagnosticSettings.enabled === true}
        {tr('New entries may appear while recording is enabled.')}
      {/if}
    </p>
    {#if $diagnosticSettings.error}
      <div class="mt-3 rounded-md border border-destructive/30 bg-destructive/5 p-3 text-xs" role="alert">
        <p class="m-0 font-medium text-destructive">
          {tr($diagnosticSettings.error.action === 'loading'
            ? 'Could not load diagnostic settings'
            : $diagnosticSettings.error.action === 'saving'
              ? 'Could not save diagnostic settings'
              : 'Could not clear diagnostics')}
        </p>
        <p class="m-0 mt-1 break-words text-muted-foreground">{$diagnosticSettings.error.message}</p>
      </div>
    {:else if $diagnosticSettings.cleared}
      <p class="m-0 mt-3 text-xs text-success" role="status">{tr('Diagnostics cleared')}</p>
    {/if}
    {#if $diagnosticSettings.enabled === null && $diagnosticSettings.busy !== 'loading'}
      <Button
        class="mt-3"
        variant="outline"
        size="sm"
        disabled={$diagnosticSettings.busy !== null || exporting !== null}
        onclick={() => void diagnosticSettings.setEnabled(false)}
      >{tr('Turn off recording')}</Button>
      <Button
        class="mt-3"
        variant="outline"
        size="sm"
        disabled={$diagnosticSettings.busy !== null || exporting !== null}
        onclick={() => void diagnosticSettings.load()}
      >{tr('Reload diagnostic settings')}</Button>
    {/if}
    {#if serverPathsAvailable}
    <div class="mt-5 border-t pt-4">
      <h4 class="m-0 text-xs font-medium">{tr('Diagnostic package')}</h4>
      <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
        {tr('Export logs, environment, adapter status, and usage metadata as a zip. Drafts, feedback text, attachments, and API keys are never included.')}
      </p>
    </div>
    <div class="mt-4 flex flex-wrap gap-2">
      <Button
        variant="outline"
        disabled={exporting !== null || $diagnosticSettings.busy !== null}
        onclick={() => void exportDiagnostics('last_24_hours')}
      >
        {#if exporting === 'last_24_hours'}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
        {:else}
          <FileArchive data-icon="inline-start" />
        {/if}
        {tr('Export last 24 hours')}
      </Button>
      <Button
        variant="outline"
        disabled={exporting !== null || $diagnosticSettings.busy !== null}
        onclick={() => void exportDiagnostics('last_7_days')}
      >
        {#if exporting === 'last_7_days'}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
        {:else}
          <FileArchive data-icon="inline-start" />
        {/if}
        {tr('Export last 7 days')}
      </Button>
      <Button
        variant="outline"
        disabled={exporting !== null || $diagnosticSettings.busy !== null}
        onclick={() => void exportDiagnostics('all')}
      >
        {#if exporting === 'all'}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
        {:else}
          <FileArchive data-icon="inline-start" />
        {/if}
        {tr('Export all diagnostics')}
      </Button>
      {#if lastExportPath}
        <Button variant="ghost" onclick={() => void revealExportedPath(lastExportPath)}>
          <FolderOpen data-icon="inline-start" />
          {tr('Show in folder')}
        </Button>
      {/if}
    </div>
    {#if lastExportPath}
      <p class="m-0 mt-3 truncate font-mono text-[10px] leading-4 text-muted-foreground" title={lastExportPath}>
        {lastExportPath}
      </p>
    {/if}
    {/if}
  </section>
  {/if}

  <p class="m-0 text-center text-[10px] text-muted-foreground">
    © 2026 RambleDesk · MIT · {tr('See THIRD_PARTY_NOTICES.md for third-party component notices')}
  </p>
</div>
