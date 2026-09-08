<script lang="ts">
  import { onMount } from 'svelte'
  import {
    Check,
    CheckCircle2,
    ChevronDown,
    Clipboard,
    Download,
    LoaderCircle,
    PlugZap,
    RefreshCw,
    ShieldCheck,
    TerminalSquare,
    Trash2,
  } from '@lucide/svelte'

  import * as Alert from '$lib/components/ui/alert'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { DshHostStatus } from '$lib/capabilities/workbenchCapabilities'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { createExternalAdapterSettingsAccess } from '$lib/workspace/externalAdapterSettingsAccess'
  import dshLogoSvg from '../../assets/dsh-logo.svg?raw'
  import piLogoSvg from '../../assets/pi-logo.svg?raw'

  type McpHostView = {
    id: string
    name: string
    iconSvg: string
    installed: boolean
    configured: boolean
    configPath: string
    restartRequired: boolean
  }

  type PiPackageStatus = {
    cliAvailable: boolean
    installed: boolean
    sourceCount: number
    restartRequired: boolean
  }

  export let capabilities: WorkbenchCapabilities
  /** True while the settings navigation shows this page. */
  export let active = false
  export let initialConfiguration = ''
  export let onOpenAgents: () => void = () => {}

  let hosts: McpHostView[] = []
  let selectedIds = new Set<string>()
  let loadingHosts = false
  let installing = false
  let installMessage = ''
  let installError = ''
  let piStatus: PiPackageStatus | null = null
  let piStatusLoading = false
  let piAction: 'install' | 'uninstall' | null = null
  let piLastAction: 'status' | 'install' | 'uninstall' = 'status'
  let piInstallMessage = ''
  let piInstallError = ''
  let installingDsh = false
  let dshInstallMessage = ''
  let dshInstallError = ''
  let dshStatus: DshHostStatus | null = null
  let dshStatusLoading = false
  let mounted = false
  let mcpConfiguration = initialConfiguration
  let copyState: 'idle' | 'copied' | 'error' = 'idle'
  let starterPromptCopyState: 'idle' | 'copied' | 'error' = 'idle'
  let genericAdapterOpen = true
  let configurationOpen = false
  const adapterStarterPrompt = 'Please request feedback through RambleDesk so I can first describe the goal of this task.'
  const adapterSettingsAccess = createExternalAdapterSettingsAccess([
    refreshHosts, refreshPiStatus, refreshDshStatus, refreshMcpConfiguration,
  ])
  const isWindows = capabilities.windowControls.implementation.platform() === 'Windows'

  $: selectedCount = selectedIds.size
  // Adapter reads belong to an explicit visit to this page.
  $: if (mounted) void adapterSettingsAccess.setSection('adapters', active)

  onMount(() => {
    mounted = true
    return () => {
      mounted = false
      void adapterSettingsAccess.setSection('general', false)
    }
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

  async function refreshHosts() {
    if (!adapterSettingsAccess.isActive()) return
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
    if (!adapterSettingsAccess.isActive() || selectedIds.size === 0 || installing) return
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
    if (!adapterSettingsAccess.isActive()) return
    try {
      await navigator.clipboard.writeText(mcpConfiguration)
      copyState = 'copied'
    } catch {
      copyState = 'error'
    }
  }

  async function copyAdapterStarterPrompt() {
    if (!adapterSettingsAccess.isActive()) return
    try {
      await navigator.clipboard.writeText(tr(adapterStarterPrompt))
      starterPromptCopyState = 'copied'
    } catch {
      starterPromptCopyState = 'error'
    }
  }

  async function refreshPiStatus(reportError = true) {
    if (!adapterSettingsAccess.isActive()) return
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
    if (!adapterSettingsAccess.isActive() || piAction) return
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
    if (!adapterSettingsAccess.isActive() || piAction || !piStatus?.installed || !confirm(tr('Uninstall the Pi native adapter?'))) return
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
    if (!adapterSettingsAccess.isActive() || installingDsh) return
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
      await refreshDshStatus(false)
      installingDsh = false
    }
  }

  async function refreshDshStatus(reportError = true) {
    if (!adapterSettingsAccess.isActive()) return
    dshStatusLoading = true
    try {
      dshStatus = await capabilities.hostIntegrationAdministration.implementation.dshStatus()
    } catch (cause) {
      dshStatus = null
      if (reportError) dshInstallError = messageFrom(cause)
    } finally { dshStatusLoading = false }
  }

  async function refreshMcpConfiguration() {
    if (!adapterSettingsAccess.isActive()) return
    try {
      mcpConfiguration = await capabilities.hostIntegrationAdministration.implementation.genericMcpConfiguration()
    } catch (cause) { installError = messageFrom(cause) }
  }
</script>

            <section class="space-y-4 rounded-lg border bg-muted/20 p-4">
              <div class="space-y-2">
                <h3 class="m-0 text-sm font-medium">{tr('A lightweight connection to your existing workflow')}</h3>
                <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Keep working in your agent’s app or terminal. It manages the session and conversation, while RambleDesk handles feedback requests and your replies.')}</p>
                <ol class="m-0 list-decimal space-y-1 pl-4 text-xs leading-5 text-muted-foreground">
                  <li>{tr('Install the adapter for the external agent below, then restart that agent.')}</li>
                  <li>{tr('Continue your work there and ask the agent to request RambleDesk feedback when needed.')}</li>
                  <li>{tr('Review and submit the feedback in RambleDesk. Follow a Resume Prompt back to the external agent when one is provided.')}</li>
                </ol>
                <div class="space-y-2 pt-2">
                  <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Paste this example into your external agent:')}</p>
                  <p class="m-0 select-text rounded-md border bg-background p-3 text-xs leading-5">{tr(adapterStarterPrompt)}</p>
                  <div class="flex flex-wrap items-center gap-2">
                    <Button variant="outline" size="sm" onclick={copyAdapterStarterPrompt}>
                      {#if starterPromptCopyState === 'copied'}
                        <Check data-icon="inline-start" />
                        {tr('Copied')}
                      {:else}
                        <Clipboard data-icon="inline-start" />
                        {tr('Copy starter prompt')}
                      {/if}
                    </Button>
                    {#if starterPromptCopyState === 'error'}
                      <span class="text-xs text-destructive" role="status">{tr('Could not copy the prompt. Select the text and copy it manually.')}</span>
                    {/if}
                  </div>
                  <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('If your agent supports /ramble, you can also use it to start a feedback request.')}</p>
                </div>
              </div>
              <div class="space-y-2 border-t pt-4">
                <h3 class="m-0 text-sm font-medium">{tr('Work in RambleDesk (recommended)')}</h3>
                <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Connect through ACP to create sessions, chat with agents, and handle feedback in RambleDesk.')}</p>
                <Button variant="outline" size="sm" onclick={onOpenAgents}>{tr('Go to Agents')}</Button>
              </div>
            </section>
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
                    {tr('Adds feedback requests to your external Pi session. Pi can wait for your reply while you review the request in RambleDesk.')}
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
                    {#if dshStatusLoading}<Badge variant="outline">{tr('Checking…')}</Badge>
                    {:else if dshStatus?.profiles.some(profile => profile.configured)}<Badge variant="secondary">{tr('Installed')}</Badge>
                    {:else if dshStatus}<Badge variant="outline">{tr('Not installed')}</Badge>{/if}
                    {#if dshStatus && !dshStatus.installed}<Badge variant="outline">{tr('DSH not detected')}</Badge>{/if}
                  </div>
                  <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                    {tr('Adds feedback requests to your external DSH profiles and installs the Ramble guide. DSH can wait for your reply in its own session.')}
                  </p>
                </div>
                <Button disabled={installingDsh || dshStatusLoading || dshStatus?.profiles.length === 0 || capabilities.hostIntegrationAdministration.status.availability === 'unavailable'} onclick={installDshPackage}>
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
