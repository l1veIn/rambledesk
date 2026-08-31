<script lang="ts">
  import { open as choosePath } from '@tauri-apps/plugin-dialog'
  import { FolderOpen, LoaderCircle, Play, Rocket } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { acpClientDefaults } from './preferences'
  import AgentLogo from './AgentLogo.svelte'
  import { launchBootstrapDocumentJson, launchBootstrapMarkdown } from './launchBootstrap'
  import { isCurrentPreflightContext, isUsablePreflight, resolvePreflightSelection } from './state'
  import type { AccessMode, AgentSummary, LaunchDraft, LaunchPreflight } from './types'

  export let open = false
  export let agents: AgentSummary[] = []
  export let busy = false
  export let error = ''
  export let onPreflight: (draft: LaunchDraft) => Promise<LaunchPreflight> = async () => ({
    agentId: '', models: [], reasoningEfforts: [], accessModes: [], warning: null,
  })
  export let onLaunch: (draft: LaunchDraft) => void = () => {}

  let workspace = ''
  let agentId = ''
  let model = ''
  let reasoningEffort = 'high'
  let accessMode: AccessMode = 'workspace_write'
  let submissionId = crypto.randomUUID()
  let preflight: LaunchPreflight | null = null
  let preflightBusy = false
  let preflightGeneration = 0
  let preflightTimer: ReturnType<typeof setTimeout> | null = null
  let wasOpen = false

  $: selectedAgent = agents.find((agent) => agent.id === agentId) ?? agents[0] ?? null
  $: preflightReady = isUsablePreflight(preflight)
  $: if (open && !wasOpen) {
    wasOpen = true
    submissionId = crypto.randomUUID()
    const defaultAgent = agents.find(
      (agent) => agent.id === $acpClientDefaults.agentId && agent.supportsStructuredRamble,
    ) ?? agents.find((agent) => agent.supportsStructuredRamble)
    agentId = defaultAgent?.id ?? ''
    model = ''
    reasoningEffort = ''
    accessMode = $acpClientDefaults.accessMode
    preflight = null
    preflightGeneration += 1
    preflightBusy = false
    if (workspace.trim() && agentId) preflightTimer = setTimeout(() => void runPreflight(), 0)
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
      model,
      reasoningEffort,
      accessMode,
      documentJson: launchBootstrapDocumentJson,
      bodyMarkdown: launchBootstrapMarkdown,
    }
  }

  async function selectWorkspace() {
    try {
      const selected = await choosePath({ directory: true, multiple: false })
      if (typeof selected === 'string') {
        workspace = selected
        schedulePreflight()
      }
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
    model = ''
    reasoningEffort = ''
    preflightBusy = true
    try {
      const result = await onPreflight({
        ...draft(),
        workspace: context.workspace,
        agentId: context.agentId,
      })
      if (!isCurrentPreflightContext(context, {
        generation: preflightGeneration,
        workspace: workspace.trim(),
        agentId,
      })) return
      preflight = result
      const selection = resolvePreflightSelection(result, {
        model: $acpClientDefaults.model,
        reasoningEffort: $acpClientDefaults.reasoningEffort,
        accessMode: $acpClientDefaults.accessMode,
      })
      model = selection.model
      reasoningEffort = selection.reasoningEffort
      if (selection.accessMode) accessMode = selection.accessMode
    } finally {
      if (context.generation === preflightGeneration) preflightBusy = false
    }
  }

  function changeAgent(nextAgentId: string) {
    agentId = nextAgentId
    schedulePreflight()
  }

  function invalidatePreflight() {
    if (preflightTimer) clearTimeout(preflightTimer)
    preflightTimer = null
    preflightGeneration += 1
    preflightBusy = false
    preflight = null
    model = ''
    reasoningEffort = ''
  }

  function schedulePreflight() {
    invalidatePreflight()
    if (!workspace.trim() || !agentId) return
    preflightTimer = setTimeout(() => void runPreflight(), 250)
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="flex w-[min(680px,calc(100vw-3rem))] max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none">
    <Dialog.Header class="shrink-0 border-b px-6 py-4">
      <Dialog.Title class="flex items-center gap-2"><Rocket class="size-4 text-primary" />{tr('Launch new Ramble')}</Dialog.Title>
      <Dialog.Description>{tr('Choose how the Agent should work. It will open the Session by asking what you want to do.')}</Dialog.Description>
    </Dialog.Header>

    <form class="flex flex-col" onsubmit={(event) => { event.preventDefault(); onLaunch(draft()) }}>
      <div class="grid grid-cols-2 gap-x-4 gap-y-3 bg-muted/15 px-6 py-5">
        <label class="grid gap-1.5 text-[11px] font-medium">
          {tr('Agent')}
          <span class="relative flex items-center">
            {#if selectedAgent}<span class="absolute left-2 z-10"><AgentLogo agentId={selectedAgent.id} label={selectedAgent.label} iconSvg={selectedAgent.iconSvg} size="sm" /></span>{/if}
            <select value={agentId} class="h-9 w-full rounded-md border bg-background pl-10 pr-3 text-xs" onchange={(event) => changeAgent(event.currentTarget.value)}>
              {#each agents as agent}<option value={agent.id} disabled={!agent.supportsStructuredRamble}>{agent.label}{agent.supportsStructuredRamble ? '' : ` · ${tr('ACP connection only')}`}</option>{/each}
            </select>
          </span>
        </label>

        {#if !preflight || preflight.models.length > 0}
          <label class="grid gap-1.5 text-[11px] font-medium">
            {tr('Model')}
            <select bind:value={model} disabled={!preflight || preflightBusy} class="h-9 rounded-md border bg-background px-3 text-xs">
              {#if !preflight}<option value="">{preflightBusy ? tr('Checking Agent capabilities…') : tr('Choose a workspace first')}</option>{/if}
              {#each preflight?.models ?? [] as candidate}<option value={candidate}>{candidate}</option>{/each}
            </select>
          </label>
        {/if}

        {#if !preflight || preflight.reasoningEfforts.length > 0}
          <label class="grid gap-1.5 text-[11px] font-medium">
            {tr('Reasoning effort')}
            <select bind:value={reasoningEffort} disabled={!preflight || preflightBusy} class="h-9 rounded-md border bg-background px-3 text-xs">
              {#if !preflight}<option value="">{preflightBusy ? tr('Checking Agent capabilities…') : tr('Choose a workspace first')}</option>{/if}
              {#each preflight?.reasoningEfforts ?? [] as effort}<option value={effort}>{effort}</option>{/each}
            </select>
          </label>
        {/if}

        <label class="grid gap-1.5 text-[11px] font-medium">
          {tr('Access mode')}
          <select bind:value={accessMode} disabled={!preflight || preflightBusy} class="h-9 rounded-md border bg-background px-3 text-xs">
            {#if !preflight}<option value={$acpClientDefaults.accessMode}>{preflightBusy ? tr('Checking Agent capabilities…') : tr('Choose a workspace first')}</option>{/if}
            {#each preflight?.accessModes ?? [] as mode}
              <option value={mode}>{mode === 'read_only' ? tr('Read Only') : mode === 'workspace_write' ? tr('Workspace Write') : 'YOLO'}</option>
            {/each}
          </select>
        </label>

        <label class="col-span-2 grid gap-1.5 text-[11px] font-medium">
          {tr('Workspace')}
          <span class="flex gap-2">
            <input bind:value={workspace} class="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-xs outline-none focus:ring-2 focus:ring-ring/35" placeholder="/path/to/project" oninput={schedulePreflight} />
            <Button type="button" variant="outline" onclick={() => void selectWorkspace()}><FolderOpen data-icon="inline-start" />{tr('Choose…')}</Button>
          </span>
        </label>

        <p class="col-span-2 m-0 text-[10px] text-muted-foreground" aria-live="polite">
          {preflightBusy
            ? tr('Checking Agent capabilities…')
            : preflightReady
              ? tr('Ready. The Agent will ask what you want to do after launch.')
              : preflight?.warning
                ? tr('Agent options could not be loaded. Check the reason below and try again.')
              : tr('Choose a workspace to load the available Agent options automatically.')}
        </p>
        {#if preflight?.warning}<p class="col-span-2 m-0 rounded-md border border-warning/30 bg-warning/5 px-3 py-2 text-[11px] text-warning-foreground dark:text-warning">{preflight.warning}</p>{/if}
      </div>

      {#if error}<p class="mx-6 mb-3 rounded-md border border-destructive/25 bg-destructive/5 px-3 py-2 text-[11px] text-destructive">{error}</p>{/if}
      <Dialog.Footer class="shrink-0 border-t px-6 py-3">
        <Button type="button" variant="ghost" disabled={busy} onclick={() => (open = false)}>{tr('Cancel')}</Button>
        <Button type="submit" disabled={busy || !preflightReady || !preflight?.accessModes.includes(accessMode) || !workspace.trim() || !agentId || ((preflight?.models.length ?? 0) > 0 && !model) || ((preflight?.reasoningEfforts.length ?? 0) > 0 && !reasoningEffort)}>
          {#if busy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{:else}<Play data-icon="inline-start" />{/if}
          {tr('Launch Ramble')}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
