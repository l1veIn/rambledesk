<script lang="ts">
  import { LoaderCircle, Plus } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import type { AgentConfig } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { managedSessionDraftInput, redactAgentMessage, type CreateManagedSessionDraftInput } from './agentConfigForm'
  import { agentText } from './agentI18n'

  export let configs: AgentConfig[] = []
  export let busy = false
  export let error = ''
  export let showHeading = true
  export let initialConfigId = ''
  export let initialCwd = ''
  export let onCreate: (input: CreateManagedSessionDraftInput) => Promise<void> | void
  export let onConfigure: (() => void) | undefined = undefined

  let configId = initialConfigId
  let cwd = initialCwd
  let title = ''
  let pending = false
  let localError = ''
  $: enabledConfigs = configs.filter((config) => config.enabled)
  $: if (!configId && enabledConfigs.length) configId = enabledConfigs[0].id
  $: locked = busy || pending
  $: envText = Object.entries(configs.find((config) => config.id === configId)?.env ?? {}).map(([key, value]) => `${key}=${value}`).join('\n')
  $: safeError = redactAgentMessage(localError || error, envText)
  function tr(source: string) { return agentText($locale, source) }

  async function create() {
    if (locked) return
    localError = ''
    const operationEnv = envText
    try {
      const input = managedSessionDraftInput(configId, cwd, title, configs)
      pending = true
      await onCreate(input)
    } catch (cause) {
      const message = cause instanceof Error ? cause.message
        : typeof cause === 'object' && cause !== null && 'message' in cause ? String(cause.message)
          : tr('Something went wrong')
      localError = redactAgentMessage(message, operationEnv)
    } finally { pending = false }
  }
</script>

<form class="w-full max-w-xl space-y-5 rounded-xl border bg-background p-5" onsubmit={(event) => { event.preventDefault(); void create() }}>
  {#if showHeading}<div><h2 class="m-0 text-base font-medium">{tr('New agent session')}</h2></div>{/if}
  {#if enabledConfigs.length === 0}
    <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Configure an agent before creating a session.')}</p>
    {#if onConfigure}<Button type="button" variant="outline" size="sm" onclick={onConfigure}>{tr('Agent configurations')}</Button>{/if}
  {:else}
    <fieldset disabled={locked} class="m-0 min-w-0 space-y-4 border-0 p-0 disabled:opacity-60">
      <label class="block space-y-1.5 text-xs"><span class="font-medium">{tr('Agent configuration')}</span><select required bind:value={configId} class="h-9 w-full rounded-md border bg-background px-2"><option value="" disabled>{tr('Choose a configuration')}</option>{#each enabledConfigs as config (config.id)}<option value={config.id}>{config.name}</option>{/each}</select></label>
      <label class="block space-y-1.5 text-xs"><span class="font-medium">{tr('Project directory')}</span><input required bind:value={cwd} class="h-9 w-full rounded-md border bg-background px-3 font-mono outline-none focus:ring-2 focus:ring-ring" autocomplete="off" spellcheck="false" /><span class="block text-[11px] leading-5 text-muted-foreground">{tr('Use an absolute directory on the computer running RambleDesk.')}</span></label>
      <label class="block space-y-1.5 text-xs"><span class="font-medium">{tr('Session title (optional)')}</span><input bind:value={title} class="h-9 w-full rounded-md border bg-background px-3 outline-none focus:ring-2 focus:ring-ring" autocomplete="off" /><span class="block text-[11px] leading-5 text-muted-foreground">{tr('This names the session. Send your first task after the agent connects.')}</span></label>
    </fieldset>
    <Button type="submit" size="sm" disabled={locked}>{#if pending}<LoaderCircle class="size-3.5 animate-spin" />{:else}<Plus class="size-3.5" />{/if}{tr(pending ? 'Creating…' : 'Create session')}</Button>
  {/if}
  {#if safeError}<p role="alert" class="m-0 break-words text-xs text-destructive">{tr(safeError)}</p>{/if}
</form>
