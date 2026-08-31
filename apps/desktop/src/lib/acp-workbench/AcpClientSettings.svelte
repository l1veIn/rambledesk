<script lang="ts">
  import { AlertCircle, CheckCircle2, LoaderCircle, PlugZap } from '@lucide/svelte'
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { acpAdapterErrorMessage, createNativeAcpWorkbenchAdapter } from './adapter'
  import AgentLogo from './AgentLogo.svelte'
  import { createPreviewAcpWorkbenchAdapter } from './previewAdapter'
  import type { AgentSummary } from './types'

  type ConnectionState = {
    status: 'idle' | 'checking' | 'ready' | 'failed'
    detail: string
  }

  const native = '__TAURI_INTERNALS__' in window
  const adapter = native ? createNativeAcpWorkbenchAdapter() : createPreviewAcpWorkbenchAdapter()
  let agents: AgentSummary[] = []
  let loading = true
  let loadError = ''
  let connections: Record<string, ConnectionState> = {}

  onMount(() => void loadAgents())

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function connectionFor(agentId: string): ConnectionState {
    return connections[agentId] ?? { status: 'idle', detail: '' }
  }

  function setConnection(agentId: string, state: ConnectionState) {
    connections = { ...connections, [agentId]: state }
  }

  async function loadAgents() {
    loading = true
    loadError = ''
    try {
      const workbench = await adapter.readWorkbench()
      agents = workbench.agents
    } catch (cause) {
      loadError = acpAdapterErrorMessage(cause)
    } finally {
      loading = false
    }
  }

  async function connect(agent: AgentSummary) {
    if (connectionFor(agent.id).status === 'checking') return
    setConnection(agent.id, { status: 'checking', detail: '' })
    try {
      const readiness = await adapter.connectClient(agent.id)
      if (readiness.status === 'ready') {
        setConnection(agent.id, {
          status: 'ready',
          detail: agent.supportsStructuredRamble
            ? tr('Connection check succeeded. The Agent is ready for a new Ramble.')
            : tr('ACP connection succeeded. This Agent does not forward the RambleDesk Session Toolset, so it cannot launch a structured Ramble yet.'),
        })
      } else {
        setConnection(agent.id, {
          status: 'failed',
          detail: acpAdapterErrorMessage({ code: readiness.reasonCode, message: readiness.reason }),
        })
      }
    } catch (cause) {
      setConnection(agent.id, {
        status: 'failed',
        detail: acpAdapterErrorMessage(cause),
      })
    }
  }
</script>

<div class="space-y-5">
  <section class="border-b pb-5">
    <div class="flex gap-3">
      <span class="grid size-8 shrink-0 place-items-center rounded-md bg-primary/10 text-primary"><PlugZap class="size-4" /></span>
      <div>
        <h3 class="m-0 text-sm font-medium">{tr('ACP Client')}</h3>
        <p class="m-0 mt-1 max-w-xl text-xs leading-5 text-muted-foreground">{tr('Choose an Agent and connect. RambleDesk checks the local runtime, installs the pinned ACP client when needed, and explains any remaining login or compatibility issue.')}</p>
      </div>
    </div>
  </section>

  {#if loadError}
    <div role="alert" class="flex items-center justify-between gap-4 rounded-md border border-destructive/25 bg-destructive/5 px-4 py-3 text-xs text-destructive">
      <span>{loadError}</span>
      <Button variant="outline" size="sm" onclick={() => void loadAgents()}>{tr('Try again')}</Button>
    </div>
  {/if}

  {#if loading}
    <div class="flex items-center justify-center gap-2 rounded-lg border py-10 text-xs text-muted-foreground">
      <LoaderCircle class="size-4 animate-spin" />{tr('Loading Agent connection…')}
    </div>
  {:else if agents.length === 0 && !loadError}
    <div class="rounded-lg border px-4 py-8 text-center text-xs text-muted-foreground">{tr('No ACP Agents are available.')}</div>
  {:else}
    <div class="space-y-3">
      {#each agents as agent}
        {@const connection = connectionFor(agent.id)}
        <section class="rounded-lg border p-4">
          <div class="flex items-center justify-between gap-4">
            <div class="flex min-w-0 items-center gap-3">
              <AgentLogo agentId={agent.id} label={agent.label} iconSvg={agent.iconSvg} />
              <div class="min-w-0">
                <strong class="block truncate text-sm font-medium">{agent.label}</strong>
                {#if !agent.supportsStructuredRamble}
                  <span class="mt-0.5 block text-[10px] text-amber-700 dark:text-amber-300">{tr('ACP connection only')}</span>
                {/if}
                <span class="mt-1 flex items-center gap-1.5 text-[11px] text-muted-foreground">
                  {#if connection.status === 'ready'}
                    <CheckCircle2 class="size-3.5 text-success" />{tr('ACP Client ready')}
                  {:else if connection.status === 'failed'}
                    <AlertCircle class="size-3.5 text-destructive" />{tr('Connection failed')}
                  {:else if connection.status === 'checking'}
                    <LoaderCircle class="size-3.5 animate-spin" />{tr('Connecting and preparing…')}
                  {:else}
                    <span class="size-2 rounded-full bg-muted-foreground/45"></span>{tr('Not checked')}
                  {/if}
                </span>
              </div>
            </div>
            <Button
              size="sm"
              variant={connection.status === 'ready' ? 'outline' : 'default'}
              disabled={connection.status === 'checking'}
              onclick={() => void connect(agent)}
            >
              {#if connection.status === 'checking'}
                <LoaderCircle class="animate-spin" data-icon="inline-start" />
              {:else}
                <PlugZap data-icon="inline-start" />
              {/if}
              {connection.status === 'ready' ? tr('Check again') : connection.status === 'failed' ? tr('Try again') : tr('Connect')}
            </Button>
          </div>

          {#if connection.detail}
            <p
              class:text-destructive={connection.status === 'failed'}
              class:text-success={connection.status === 'ready'}
              class="m-0 mt-3 whitespace-pre-line rounded-md bg-muted/35 px-3 py-2 text-[11px] leading-5"
              role={connection.status === 'failed' ? 'alert' : undefined}
              aria-live="polite"
            >{connection.detail}</p>
          {/if}
        </section>
      {/each}
    </div>
  {/if}

  <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('Agent, model, reasoning effort, workspace, and access permission are chosen when you launch each Ramble.')}</p>
</div>
