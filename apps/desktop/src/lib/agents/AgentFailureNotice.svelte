<script lang="ts">
  import type { AgentConfig, AgentFailure, AgentInspection } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { Button } from '$lib/components/ui/button'
  import { agentText } from './agentI18n'
  import { agentFailureAdvice, agentFailureTitle } from './agentFailure'
  import { redactAgentMessage } from './agentConfigForm'
  import AgentSetupGuide from './AgentSetupGuide.svelte'

  export let failure: AgentFailure
  export let config: AgentConfig | null | undefined = undefined
  export let inspection: AgentInspection | undefined = undefined
  export let catalogId: string | undefined = undefined
  export let hostId: string | undefined = undefined
  export let name: string | undefined = undefined
  export let onConfigure: (() => void) | undefined = undefined
  export let onRetry: (() => Promise<void> | void) | undefined = undefined
  export let retryLabel = 'Retry preparing this session'
  export let busy = false
  $: envText = Object.entries(config?.env ?? {}).map(([key, value]) => key + '=' + value).join('\n')
  function tr(text: string) { return agentText($locale, text) }
</script>

<section class="space-y-2 rounded-lg border border-amber-500/25 bg-amber-500/5 px-3 py-3 text-xs leading-5" aria-label={tr('Agent action needs attention')} data-agent-failure={failure.reason}>
  <p role="alert" class="m-0 font-medium">{tr(agentFailureTitle(failure))}</p>
  {#if name || config?.name}<p class="m-0 text-[11px] text-muted-foreground">{tr('Current connection')}: {name ?? config?.name}</p>{/if}
  {#if failure.reason !== 'authentication'}<p class="m-0 text-muted-foreground">{tr(agentFailureAdvice(failure))}</p>{/if}
  {#if failure.reason === 'authentication'}
    <AgentSetupGuide {catalogId} {hostId} {name} {config} {inspection} {onConfigure} purpose="authentication" compact />
  {:else if onConfigure && (failure.stage === 'launch' || failure.stage === 'initialize' || failure.reason === 'configuration' || failure.reason === 'model')}
    <button type="button" class="text-left underline underline-offset-4" onclick={onConfigure}>{tr('Open this agent’s settings')}</button>
  {/if}
  {#if failure.message}<details><summary class="cursor-pointer text-[11px] text-muted-foreground">{tr('Error details')}</summary><p class="mb-0 mt-1 whitespace-pre-wrap break-words text-[11px] text-muted-foreground">{redactAgentMessage(failure.message, envText)}</p></details>{/if}
  {#if onRetry}<Button variant="outline" size="sm" aria-label={tr(retryLabel)} disabled={busy} onclick={() => void onRetry?.()}>{tr(retryLabel)}</Button>{/if}
</section>
