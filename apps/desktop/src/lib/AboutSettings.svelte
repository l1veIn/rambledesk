<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { getVersion } from '@tauri-apps/api/app'
  import { save } from '@tauri-apps/plugin-dialog'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { Download, ExternalLink, FileArchive, FolderOpen, GitBranch, LoaderCircle, RefreshCw, RotateCw, ShieldCheck, Sparkles } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import rambelleSticker from '../assets/rambelle-states/idle.webp'
  import RambelleProfileDialog from './RambelleProfileDialog.svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { toast } from '$lib/components/ui/sonner'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { diagnosticExportView } from './nativePath'
  import {
    checkForUpdates,
    downloadAndInstallUpdate,
    restartAfterUpdate,
    updateState,
  } from '$lib/updater'

  export let installBlocked = false

  let version = '0.0.1'
  let profileOpen = false
  let exporting: 'last_7_days' | 'all' | null = null
  let lastExportPath = ''
  const isTauri = '__TAURI_INTERNALS__' in window
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent)
  const projectUrl = 'https://github.com/l1veIn/rambledesk'
  const releasesUrl = `${projectUrl}/releases`

  onMount(async () => {
    if (isTauri) version = await getVersion().catch(() => version)
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function openProject() {
    if (isTauri) {
      await openUrl(projectUrl)
      return
    }
    window.open(projectUrl, '_blank', 'noopener,noreferrer')
  }

  async function openReleases() {
    if (isTauri) {
      await openUrl(releasesUrl)
      return
    }
    window.open(releasesUrl, '_blank', 'noopener,noreferrer')
  }

  async function exportDiagnostics(scope: 'last_7_days' | 'all') {
    if (!isTauri || exporting) return
    exporting = scope
    try {
      const stamp = new Date().toISOString().slice(0, 10)
      const path = await save({
        defaultPath: `RambleDesk-diagnostics-${scope === 'all' ? 'all' : '7d'}-${stamp}.zip`,
        filters: [{ name: 'Zip', extensions: ['zip'] }],
      })
      if (!path) return
      const exported = diagnosticExportView(await invoke('export_diagnostics', { scope, path }))
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
      await invoke('reveal_path_in_folder', { path })
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
          <Badge variant="secondary">v{version}</Badge>
          <Badge variant="outline">{isMac ? 'macOS' : /Win/.test(navigator.platform || navigator.userAgent) ? 'Windows' : 'Linux'}</Badge>
        </div>
        <p class="m-0 mt-3 max-w-xl text-sm leading-6 text-muted-foreground">
          {tr('Let agents pause at key moments and request structured human feedback that can be resumed and archived.')}
        </p>
        <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">
          {tr('Feedback drafts, attachments, and packages stay on your device. The agent only receives results you explicitly submit or cancel.')}
        </p>
        <Button variant="link" class="mt-3 h-auto gap-1.5 p-0 text-xs" onclick={() => void openProject()}>
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
      <Button variant="outline" class="shrink-0" onclick={() => (profileOpen = true)}>
        <Sparkles data-icon="inline-start" />
        {tr('View character profile')}
      </Button>
    </div>
  </section>

  <section class="rounded-xl border p-5">
    <div class="flex items-start justify-between gap-6">
      <div>
        <h3 class="m-0 text-sm font-medium">{tr('Software updates')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {#if isMac}
            {tr('macOS builds are unsigned. Download a new DMG from GitHub Releases when you want to update.')}
          {:else}
            {tr('RambleDesk checks for updates quietly after launch, and you can check manually at any time.')}
          {/if}
        </p>
      </div>
      {#if isMac}
        <Button variant="outline" onclick={() => void openReleases()}>
          <ExternalLink data-icon="inline-start" />
          {tr('Open GitHub Releases')}
        </Button>
      {:else}
      <Button
        variant="outline"
        disabled={!isTauri || $updateState.status === 'checking' || $updateState.status === 'downloading'}
        onclick={() => void checkForUpdates()}
      >
        {#if $updateState.status === 'checking'}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
          {tr('Checking…')}
        {:else}
          <RefreshCw data-icon="inline-start" />
          {tr('Check for updates')}
        {/if}
      </Button>
      {/if}
    </div>

    {#if !isMac}
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
              <p class="m-0 mt-1 line-clamp-3 text-[10px] leading-4 text-muted-foreground">{$updateState.message}</p>
            {/if}
          </div>
          <Button
            disabled={installBlocked}
            title={installBlocked ? tr('Finish or cancel the current feedback before installing the update.') : ''}
            onclick={() => void downloadAndInstallUpdate()}
          >
            <Download data-icon="inline-start" />
            {tr('Download and install')}
          </Button>
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
          <Button
            disabled={installBlocked}
            title={installBlocked ? tr('Finish or cancel the current feedback before restarting.') : ''}
            onclick={() => void restartAfterUpdate()}
          >
            <RotateCw data-icon="inline-start" />
            {tr('Restart now')}
          </Button>
        </div>
      {:else if $updateState.status === 'error'}
        <div>
          <strong class="block text-xs text-destructive">{tr('Update check or installation failed')}</strong>
          <p class="m-0 mt-1 break-all text-[10px] leading-4 text-muted-foreground">{$updateState.message}</p>
        </div>
      {/if}
    </div>

    {/if}
    {#if !isMac && installBlocked && ($updateState.status === 'available' || $updateState.status === 'ready')}
      <p class="m-0 mt-3 text-[10px] leading-4 text-warning-foreground dark:text-warning">
        {tr('Feedback is in progress or has unsaved content. Update restart is disabled to prevent data loss.')}
      </p>
    {/if}
  </section>

  <section class="rounded-xl border p-5">
    <div>
      <h3 class="m-0 text-sm font-medium">{tr('Diagnostic package')}</h3>
      <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
        {tr('Export logs, environment, adapter status, and usage metadata as a zip. Drafts, feedback text, attachments, and API keys are never included.')}
      </p>
    </div>
    <div class="mt-4 flex flex-wrap gap-2">
      <Button
        variant="outline"
        disabled={!isTauri || exporting !== null}
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
        disabled={!isTauri || exporting !== null}
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
  </section>

  <p class="m-0 text-center text-[10px] text-muted-foreground">
    © 2026 RambleDesk · MIT · {tr('See THIRD_PARTY_NOTICES.md for third-party component notices')}
  </p>
</div>

<RambelleProfileDialog bind:open={profileOpen} />
