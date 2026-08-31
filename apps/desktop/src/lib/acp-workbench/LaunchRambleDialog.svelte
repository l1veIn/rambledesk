<script lang="ts">
  import { open as choosePath } from '@tauri-apps/plugin-dialog'
  import { FolderOpen, LoaderCircle, Play, Rocket } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  import AgentLogo from './AgentLogo.svelte'
  import LaunchConfigField from './LaunchConfigField.svelte'
  import { launchBootstrapDocumentJson, launchBootstrapMarkdown } from './launchBootstrap'
  import {
    isCurrentPreflightContext,
    isUsablePreflight,
    launchConfigIsComplete,
    resolvePreflightSelection,
  } from './state'
  import type {
    AgentSummary,
    LaunchConfigSelection,
    LaunchConfigValue,
    LaunchDraft,
    LaunchPreflight,
    LaunchPreflightInput,
  } from './types'

  export let open = false
  export let agents: AgentSummary[] = []
  export let busy = false
  export let error = ''
  export let onPreflight: (input: LaunchPreflightInput) => Promise<LaunchPreflight> = async () => ({
    agentId: '', schemaDigest: '', configOptions: [], warning: null,
  })
  export let onLaunch: (draft: LaunchDraft) => void = () => {}

  let workspace = ''
  let agentId = ''
  let submissionId = crypto.randomUUID()
  let preflight: LaunchPreflight | null = null
  let configValues: LaunchConfigSelection[] = []
  let preflightBusy = false
  let preflightGeneration = 0
  let preflightTimer: ReturnType<typeof setTimeout> | null = null
  let wasOpen = false

  $: selectedAgent = agents.find((agent) => agent.id === agentId) ?? null
  $: preflightReady = isUsablePreflight(preflight)
    && launchConfigIsComplete(preflight, configValues)
  $: if (open && !wasOpen) {
    wasOpen = true
    workspace = ''
    agentId = ''
    submissionId = crypto.randomUUID()
    preflight = null
    configValues = []
    preflightGeneration += 1
    preflightBusy = false
  }
  $: if (!open) {
    wasOpen = false
    if (preflightTimer) clearTimeout(preflightTimer)
    preflightTimer = null
  }

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function draft(): LaunchDraft {
    return {
      submissionId,
      workspace: workspace.trim(),
      agentId,
      schemaDigest: preflight?.schemaDigest ?? '',
      configValues,
      documentJson: launchBootstrapDocumentJson,
      bodyMarkdown: launchBootstrapMarkdown,
    }
  }

  function resetAgentSelection() {
    if (preflightTimer) clearTimeout(preflightTimer)
    preflightTimer = null
    preflightGeneration += 1
    preflightBusy = false
    agentId = ''
    preflight = null
    configValues = []
  }

  function changeWorkspace(next: string) {
    if (next === workspace) return
    workspace = next
    resetAgentSelection()
  }

  async function selectWorkspace() {
    try {
      const selected = await choosePath({ directory: true, multiple: false })
      if (typeof selected === 'string') changeWorkspace(selected)
    } catch {
      // Browser preview uses the editable path field instead of a native picker.
    }
  }

  async function runPreflight() {
    if (!workspace.trim() || !agentId || preflightBusy) return
    const context = {
      generation: ++preflightGeneration,
      workspace: workspace.trim(),
      agentId,
    }
    preflight = null
    configValues = []
    preflightBusy = true
    try {
      const result = await onPreflight({
        workspace: context.workspace,
        agentId: context.agentId,
      })
      if (!isCurrentPreflightContext(context, {
        generation: preflightGeneration,
        workspace: workspace.trim(),
        agentId,
      })) return
      preflight = result
      configValues = resolvePreflightSelection(result)
    } finally {
      if (context.generation === preflightGeneration) preflightBusy = false
    }
  }

  function changeAgent(nextAgentId: string) {
    if (preflightTimer) clearTimeout(preflightTimer)
    preflightTimer = null
    preflightGeneration += 1
    preflightBusy = false
    preflight = null
    configValues = []
    agentId = nextAgentId
    if (workspace.trim() && agentId) {
      preflightTimer = setTimeout(() => void runPreflight(), 250)
    }
  }

  function configValue(id: string): LaunchConfigValue | undefined {
    return configValues.find((selection) => selection.id === id)?.value
  }

  function setConfigValue(id: string, value: LaunchConfigValue) {
    const existing = configValues.findIndex((selection) => selection.id === id)
    configValues = existing < 0
      ? [...configValues, { id, value }]
      : configValues.map((selection, index) => index === existing ? { id, value } : selection)
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="flex max-h-[calc(100dvh-2rem)] w-[min(680px,calc(100vw-3rem))] max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none">
    <Dialog.Header class="shrink-0 border-b px-6 py-4">
      <Dialog.Title class="flex items-center gap-2"><Rocket class="size-4 text-primary" />{tr('Launch new Ramble')}</Dialog.Title>
      <Dialog.Description>{tr('Choose a workspace, then let the selected Agent describe its own launch options.')}</Dialog.Description>
    </Dialog.Header>

    <form class="flex min-h-0 flex-1 flex-col overflow-hidden" onsubmit={(event) => { event.preventDefault(); onLaunch(draft()) }}>
      <div class="min-h-0 flex-1 overflow-y-auto bg-muted/15 px-6 py-5">
        <div class="grid gap-4">
          <label class="grid gap-1.5 text-[11px] font-medium">
            <span>{tr('Workspace')}</span>
            <span class="flex gap-2">
              <input
                value={workspace}
                class="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-xs outline-none focus:ring-2 focus:ring-ring/35"
                placeholder="/path/to/project"
                oninput={(event) => changeWorkspace(event.currentTarget.value)}
              />
              <Button type="button" variant="outline" onclick={() => void selectWorkspace()}>
                <FolderOpen data-icon="inline-start" />{tr('Choose…')}
              </Button>
            </span>
          </label>

          {#if workspace.trim()}
            <label class="grid gap-1.5 text-[11px] font-medium">
              <span>{tr('Agent')}</span>
              <span class="flex items-center gap-2">
                {#if selectedAgent}
                  <AgentLogo agentId={selectedAgent.id} label={selectedAgent.label} iconSvg={selectedAgent.iconSvg} size="sm" />
                {/if}
                <select
                  value={agentId}
                  class="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-xs"
                  onchange={(event) => changeAgent(event.currentTarget.value)}
                >
                  <option value="">{tr('Choose an Agent')}</option>
                  {#each agents as agent}
                    <option value={agent.id} disabled={!agent.supportsStructuredRamble}>
                      {agent.label}{agent.supportsStructuredRamble ? '' : ` · ${tr('ACP connection only')}`}
                    </option>
                  {/each}
                </select>
              </span>
            </label>
          {/if}

          {#if preflightBusy}
            <div class="flex min-h-20 items-center justify-center gap-2 rounded-md border bg-background/70 text-[11px] text-muted-foreground" aria-live="polite">
              <LoaderCircle class="size-4 animate-spin text-primary" />
              {tr('Checking this Agent in the selected workspace…')}
            </div>
          {:else if preflight}
            {#if preflight.configOptions.length > 0}
              <div class="grid grid-cols-1 gap-x-4 gap-y-3 sm:grid-cols-2">
                {#each preflight.configOptions as option, optionIndex (option.id)}
                  <div
                    class={preflight.configOptions.length % 2 === 1 && optionIndex === preflight.configOptions.length - 1
                      ? 'min-w-0 sm:col-span-2'
                      : 'min-w-0'}
                  >
                    <LaunchConfigField
                      {option}
                      value={configValue(option.id)}
                      disabled={busy}
                      onChange={(value) => setConfigValue(option.id, value)}
                    />
                  </div>
                {/each}
              </div>
            {:else}
              <p class="m-0 rounded-md border bg-background px-3 py-3 text-[11px] text-muted-foreground">
                {tr('This Agent did not report any launch options. RambleDesk will use its Agent defaults.')}
              </p>
            {/if}

            <p class="m-0 text-[10px] text-muted-foreground" aria-live="polite">
              {preflightReady
                ? tr('Ready. The Agent will ask what you want to do after launch.')
                : tr('The Agent options are incomplete. Refresh the Agent check and try again.')}
            </p>
            {#if preflight.warning}
              <p class="m-0 rounded-md border border-warning/30 bg-warning/5 px-3 py-2 text-[11px] text-warning-foreground dark:text-warning">
                {preflight.warning}
              </p>
            {/if}
          {:else}
            <p class="m-0 text-[10px] text-muted-foreground">
              {workspace.trim()
                ? tr('Choose an Agent to load the options it provides for this workspace.')
                : tr('Choose a workspace first. Agent selection and options come next.')}
            </p>
          {/if}
        </div>
      </div>

      {#if error}<p class="mx-6 mb-3 rounded-md border border-destructive/25 bg-destructive/5 px-3 py-2 text-[11px] text-destructive">{error}</p>{/if}
      <Dialog.Footer class="mx-0 mb-0 shrink-0 rounded-none border-t px-6 py-3">
        <Button type="button" variant="ghost" disabled={busy} onclick={() => (open = false)}>{tr('Cancel')}</Button>
        <Button type="submit" disabled={busy || !preflightReady || !workspace.trim() || !agentId}>
          {#if busy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<Play data-icon="inline-start" />{/if}
          {tr('Launch Ramble')}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
