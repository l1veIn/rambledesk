<script lang="ts">
  import { CheckCircle2, Eye, EyeOff, LoaderCircle, Plus, Save, Trash2 } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import { Badge } from '$lib/components/ui/badge'
  import type { AgentConfig, AgentConnectionCheck, SaveAgentConfigInput } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import {
    AGENT_PRESETS,
    AgentDraftCache,
    agentConfigDraft,
    agentDraftInput,
    newAgentDraft,
    redactAgentMessage,
  } from './agentConfigForm'
  import { agentText } from './agentI18n'

  export let configs: AgentConfig[] = []
  export let busy = false
  export let error = ''
  export let onSave: (input: SaveAgentConfigInput) => Promise<AgentConfig | void> | AgentConfig | void
  export let onDelete: (id: string) => Promise<void> | void
  export let onCheck: (id: string) => Promise<AgentConnectionCheck | void> | AgentConnectionCheck | void

  const cache = new AgentDraftCache()
  const baselines = new Map<string, string>()
  let draft = newAgentDraft()
  let initialized = false
  let revealEnvironment = false
  let pending: 'save' | 'delete' | 'check' | null = null
  let localError = ''
  let notice = ''
  let checkResult: { ok: boolean; message: string; details: readonly string[] } | null = null

  $: if (!initialized && !locked && configs.length > 0) selectConfig(configs[0].id)
  $: cache.remember(draft)
  $: locked = busy || pending !== null
  $: signature = JSON.stringify(draft)
  $: dirty = signature !== (baselines.get(draft.id ?? 'new') ?? '')
  $: safeError = redactAgentMessage(localError || error, draft.envText)

  function tr(source: string) { return agentText($locale, source) }

  function selectConfig(id: string | null) {
    // Read the source state directly: deletion can select another config before the next reactive flush.
    if (busy || pending !== null) return
    cache.remember(draft)
    initialized = true
    draft = cache.select(id, configs)
    const config = configs.find((item) => item.id === id)
    if (config && !baselines.has(id!)) baselines.set(id!, JSON.stringify(agentConfigDraft(config)))
    revealEnvironment = false
    localError = ''
    notice = ''
    checkResult = null
  }

  function choosePreset(id: string) {
    const preset = AGENT_PRESETS.find((item) => item.id === id)
    if (!preset || locked) return
    initialized = true
    draft = { ...newAgentDraft(preset), envText: draft.id === null ? draft.envText : '' }
    cache.remember(draft)
    revealEnvironment = false
    localError = ''
    notice = ''
    checkResult = null
  }

  function failure(cause: unknown) {
    const message = cause instanceof Error ? cause.message
      : typeof cause === 'object' && cause !== null && 'message' in cause ? String(cause.message)
        : tr('Something went wrong')
    // Redact at receipt too: a later edit may remove the value from the draft.
    localError = redactAgentMessage(message, draft.envText)
  }

  async function save() {
    if (locked) return
    localError = ''
    notice = ''
    let input: SaveAgentConfigInput
    try { input = agentDraftInput(draft) } catch (cause) { failure(cause); return }
    const submitted = { ...draft }
    pending = 'save'
    try {
      const saved = await onSave(input)
      if (saved) {
        if (submitted.id === null) cache.remove(null)
        draft = agentConfigDraft(saved)
      }
      cache.remember(draft)
      baselines.set(draft.id ?? 'new', JSON.stringify(draft))
      // Assignment also invalidates the baseline-derived dirty flag in Svelte.
      draft = { ...draft }
      notice = 'Configuration saved.'
      checkResult = null
    } catch (cause) {
      failure(cause)
    } finally {
      pending = null
    }
  }

  async function remove() {
    if (locked || !draft.id) return
    const id = draft.id
    pending = 'delete'
    localError = ''
    notice = ''
    try {
      await onDelete(id)
      cache.remove(id)
      baselines.delete(id)
      draft = newAgentDraft()
      pending = null
      selectConfig(configs.find((config) => config.id !== id)?.id ?? null)
    } catch (cause) {
      failure(cause)
    } finally {
      pending = null
    }
  }

  async function check() {
    if (locked || !draft.id || dirty) return
    pending = 'check'
    localError = ''
    notice = ''
    checkResult = null
    try {
      const result = await onCheck(draft.id)
      checkResult = {
        ok: result?.ok ?? true,
        message: redactAgentMessage(result?.message ?? 'Check completed.', draft.envText),
        details: (result?.details ?? []).map((detail) => redactAgentMessage(detail, draft.envText)),
      }
    } catch (cause) {
      failure(cause)
    } finally {
      pending = null
    }
  }
</script>

<section class="space-y-5 @container" aria-label={tr('Agent configurations')}>
  <div class="flex items-start justify-between gap-4">
    <div>
      <h3 class="m-0 text-sm font-medium">{tr('Agent configurations')}</h3>
      <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
        {tr('Presets only fill the command. Install the required agent separately, then check its connection.')}
      </p>
    </div>
    <Button variant="outline" size="sm" disabled={locked} onclick={() => selectConfig(null)}>
      <Plus class="size-3.5" />{tr('Add configuration')}
    </Button>
  </div>

  <div class="grid min-w-0 gap-4 @min-[720px]:grid-cols-[200px_minmax(0,1fr)]">
    <nav class="space-y-1" aria-label={tr('Agent configurations')}>
      {#each configs as config (config.id)}
        <button type="button" disabled={locked} aria-current={draft.id === config.id ? 'page' : undefined}
          class={`flex w-full items-center gap-2 rounded-md border px-3 py-2.5 text-left text-xs transition-colors disabled:opacity-50 ${draft.id === config.id ? 'border-primary/40 bg-primary/5' : 'border-transparent hover:bg-muted'}`}
          onclick={() => selectConfig(config.id)}>
          <span class={`size-1.5 shrink-0 rounded-full ${config.enabled ? 'bg-primary' : 'bg-muted-foreground/40'}`}></span>
          <span class="min-w-0 flex-1"><strong class="block truncate font-medium">{config.name}</strong><span class="mt-0.5 block text-[10px] text-muted-foreground">{config.host_id}</span></span>
        </button>
      {/each}
      {#if configs.length === 0}<p class="m-0 px-3 py-3 text-xs text-muted-foreground">{tr('No agent configurations yet.')}</p>{/if}
      {#if draft.id === null}<div class="rounded-md border border-dashed px-3 py-2.5 text-xs text-muted-foreground">{tr('New configuration')}</div>{/if}
    </nav>

    <form class="min-w-0 space-y-4 rounded-lg border bg-background p-4" onsubmit={(event) => { event.preventDefault(); void save() }} onfocusin={() => initialized = true}>
      <fieldset disabled={locked} class="m-0 min-w-0 space-y-4 border-0 p-0 disabled:opacity-70">
        {#if draft.id === null}
          <label class="block space-y-1.5 text-xs">
            <span class="font-medium">{tr('Start from a preset')}</span>
            <select class="h-9 w-full rounded-md border bg-background px-2 text-xs" value="" onchange={(event) => choosePreset(event.currentTarget.value)}>
              <option value="">{tr('Custom command')}</option>
              {#each AGENT_PRESETS as preset}<option value={preset.id}>{preset.name} · {tr('Needs checking')}</option>{/each}
            </select>
          </label>
          {#each AGENT_PRESETS.filter((preset) => preset.command === draft.command) as preset}
            <p class="m-0 text-[11px] text-muted-foreground">{tr(preset.note)}</p>
          {/each}
        {/if}
        <div class="grid gap-4 sm:grid-cols-2">
          <label class="block space-y-1.5 text-xs"><span class="font-medium">{tr('Configuration name')}</span><input required bind:value={draft.name} class="h-9 w-full rounded-md border bg-background px-3 outline-none focus:ring-2 focus:ring-ring" autocomplete="off" /></label>
          <label class="block space-y-1.5 text-xs"><span class="font-medium">{tr('Agent backend identifier')}</span><input required bind:value={draft.hostId} class="h-9 w-full rounded-md border bg-background px-3 outline-none focus:ring-2 focus:ring-ring" autocomplete="off" /></label>
        </div>
        <label class="block space-y-1.5 text-xs"><span class="font-medium">{tr('Executable command')}</span><input required bind:value={draft.command} class="h-9 w-full rounded-md border bg-background px-3 font-mono outline-none focus:ring-2 focus:ring-ring" autocomplete="off" spellcheck="false" /></label>
        <label class="block space-y-1.5 text-xs"><span class="font-medium">{tr('One argument per line')}</span><textarea bind:value={draft.argsText} rows="3" class="w-full resize-y rounded-md border bg-background px-3 py-2 font-mono outline-none focus:ring-2 focus:ring-ring" spellcheck="false"></textarea><span class="block text-[11px] text-muted-foreground">{tr('Arguments are passed directly, without shell expansion.')}</span></label>
        <div class="space-y-1.5 text-xs">
          <div class="flex items-center justify-between gap-2"><label for="agent-config-env" class="font-medium">{tr('Environment variables')}</label><Button type="button" variant="ghost" size="icon-sm" aria-label={tr(revealEnvironment ? 'Hide environment values' : 'Show environment values')} onclick={() => revealEnvironment = !revealEnvironment}>{#if revealEnvironment}<EyeOff class="size-3.5" />{:else}<Eye class="size-3.5" />{/if}</Button></div>
          {#if revealEnvironment}<textarea id="agent-config-env" bind:value={draft.envText} rows="4" class="w-full resize-y rounded-md border bg-background px-3 py-2 font-mono outline-none focus:ring-2 focus:ring-ring" autocomplete="off" spellcheck="false"></textarea>
          {:else}<button id="agent-config-env" type="button" class="flex min-h-24 w-full items-center justify-center rounded-md border bg-muted/30 px-3 text-muted-foreground" onclick={() => revealEnvironment = true}>{tr('Environment values are hidden.')}</button>{/if}
          <p class="m-0 text-[11px] text-muted-foreground">{tr('One KEY=VALUE per line. Values stay in this configuration.')}</p>
        </div>
        <label class="flex items-center gap-2 text-xs"><input type="checkbox" bind:checked={draft.enabled} class="size-3.5 accent-primary" />{tr('Enabled for new sessions')}</label>
      </fieldset>

      {#if safeError}<p role="alert" class="m-0 break-words rounded-md border border-destructive/25 bg-destructive/5 px-3 py-2 text-xs text-destructive">{tr(safeError)}</p>{/if}
      {#if notice}<p role="status" class="m-0 text-xs text-muted-foreground">{tr(notice)}</p>{/if}
      {#if checkResult}
        <div role="status" class={`space-y-2 rounded-md border p-3 text-xs ${checkResult.ok ? 'border-border' : 'border-destructive/25 text-destructive'}`}>
          <div class="flex items-start gap-2">{#if checkResult.ok}<CheckCircle2 class="mt-0.5 size-3.5 shrink-0" />{/if}<span class="break-words">{tr(checkResult.message)}</span></div>
          {#if checkResult.details.length}<ul class="m-0 list-disc space-y-1 pl-5">{#each checkResult.details as detail}<li class="break-words">{detail}</li>{/each}</ul>{/if}
        </div>
      {/if}
      <div class="flex flex-wrap items-center gap-2 border-t pt-4">
        <Button type="submit" size="sm" disabled={locked || !dirty}>{#if pending === 'save'}<LoaderCircle class="size-3.5 animate-spin" />{:else}<Save class="size-3.5" />{/if}{tr('Save configuration')}</Button>
        {#if draft.id}<Button type="button" variant="outline" size="sm" disabled={locked || dirty} title={dirty ? tr('Save changes before checking this configuration.') : undefined} onclick={() => void check()}>{#if pending === 'check'}<LoaderCircle class="size-3.5 animate-spin" />{/if}{tr(pending === 'check' ? 'Checking…' : 'Check connection')}</Button><Button type="button" variant="ghost" size="icon-sm" class="ml-auto text-muted-foreground hover:text-destructive" disabled={locked} aria-label={tr('Delete configuration')} onclick={() => void remove()}><Trash2 class="size-3.5" /></Button>{:else}<Badge variant="outline">{tr('Needs checking')}</Badge>{/if}
      </div>
      <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('Saved changes apply when an agent instance next starts.')}</p>
      <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('Connection checks verify the ACP handshake. Session and feedback support depend on the installed agent.')}</p>
    </form>
  </div>
</section>
