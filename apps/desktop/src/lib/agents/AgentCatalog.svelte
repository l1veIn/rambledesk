<!-- Layout and check/detail flow adapted from Codeg 3ebdfed acp-agent-settings.tsx (Apache-2.0).
     Modified for Svelte, typed application transport, owned installs and RambleDesk feedback capabilities. -->
<script lang="ts">
  import { onMount } from 'svelte'
  import { Bot, Check, CheckCircle2, ChevronRight, Download, ExternalLink, LoaderCircle, Plus, RefreshCw, Settings2, XCircle } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { AgentConfig, SaveAgentConfigInput } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { createAgentCatalogController, catalogConfiguration, configurationsForAgent, installIsActive } from './agentCatalogController'
  import AgentSettings from './AgentSettings.svelte'
  import { AGENT_SETUP, applyAgentCredentials } from './agentOnboarding'
  import { createAgentSettingsController } from './agentSettingsController'
  export let transport: ApplicationTransport
  const catalog = createAgentCatalogController(transport)
  const settings = createAgentSettingsController(transport)
  let selected = 'deepseek-acp'
  let custom = false
  let saving = false
  let localError = ''
  let notice = ''
  let apiKey = ''
  let apiUrl = ''
  let model = ''
  let checked: { ok: boolean; message: string } | undefined
  let selectedBefore = ''
  $: entry = $catalog.entries.find(item => item.id === selected) ?? $catalog.entries[0]
  $: setup = entry ? AGENT_SETUP[entry.id] : undefined
  $: inspection = entry ? $catalog.inspections[entry.id] : undefined
  $: checking = entry ? $catalog.checking.includes(entry.id) : false
  $: jobs = entry ? $catalog.jobs.filter(job => job.agent_id === entry.id) : []
  $: job = jobs.at(-1)
  $: installing = job ? installIsActive(job) : false
  $: profiles = entry ? configurationsForAgent(entry, $settings.configs, inspection) : []
  $: if (entry && entry.id !== selectedBefore) {
    selectedBefore = entry.id; localError = ''; notice = ''; checked = undefined; apiKey = ''; apiUrl = ''; model = ''
    if (!inspection && !checking) void catalog.inspect(entry.id)
  }
  onMount(() => { const a = catalog.start(); const b = settings.start(); return () => { a(); b() } })
  function tr(zh: string, en: string) { return $locale === 'zh-CN' ? zh : en }
  function status(id: string) {
    if ($catalog.checking.includes(id)) return tr('检测中', 'Checking')
    const info = $catalog.inspections[id]
    if (!info) return tr('待检测', 'Not checked')
    if (info.source === 'missing') return tr('未安装', 'Not installed')
    if (info.checks.some(check => check.status === 'fail')) return tr('需要处理', 'Needs attention')
    return tr('已安装', 'Installed')
  }
  async function useAgent(profile?: AgentConfig) {
    if (!entry || !inspection || saving) return
    saving = true; localError = ''; notice = ''
    try {
      const resolved = catalogConfiguration(entry, inspection)
      let input: SaveAgentConfigInput = profile ? { ...profile, command: resolved.command, args: resolved.args } : resolved
      // User-entered credentials become part of this explicit configuration save.
      input.env = applyAgentCredentials(input.env, setup, { key: apiKey, baseUrl: apiUrl, model })
      const saved = await settings.save(input)
      apiKey = ''; apiUrl = ''; model = ''
      notice = tr(`已保存「${saved.name}」，正在检查连接。`, `“${saved.name}” is saved. Checking the connection.`)
      checked = await settings.check(saved.id)
      notice = checked.ok ? tr(`已准备好「${saved.name}」，可以从会话列表新建会话。`, `“${saved.name}” is ready to select when creating a session.`) : tr('配置已保存，请根据检查结果完成设置。', 'Configuration saved. Complete the setup using the check result.')
    } catch (error) { localError = typeof error === 'object' && error && 'message' in error ? String(error.message) : String(error) }
    finally { saving = false }
  }
</script>

<section class="space-y-4 @container" aria-label={tr('智能体管理', 'Agent management')}>
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div><h3 class="m-0 text-sm font-semibold">{tr('智能体', 'Agents')}</h3><p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('选择智能体，准备运行环境，然后开始你的项目。', 'Choose an agent, prepare its environment, and start your project.')}</p></div>
    <Button variant="outline" size="sm" onclick={() => custom = !custom}><Plus class="size-3.5" />{tr(custom ? '返回智能体列表' : '自定义配置', custom ? 'Back to agents' : 'Custom configuration')}</Button>
  </div>
  {#if $catalog.error || localError}<p role="alert" class="rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-xs">{$catalog.error || localError}</p>{/if}
  {#if custom}
    <AgentSettings configs={$settings.configs} busy={$settings.loading} onSave={settings.save} onDelete={settings.remove} onCheck={settings.check} />
  {:else}
    <div class="grid min-w-0 gap-3 @min-[680px]:grid-cols-[210px_minmax(0,1fr)]">
      <nav class="overflow-hidden rounded-xl border bg-card" aria-label={tr('智能体列表', 'Agent list')}>
        <div class="border-b px-3 py-2 text-[11px] font-medium text-muted-foreground">{tr('可用智能体', 'Agent catalog')} <span class="float-right">{$catalog.entries.length}</span></div>
        <div class="max-h-[560px] space-y-1 overflow-y-auto p-2">
          {#each $catalog.entries as agent (agent.id)}
            <button type="button" class={`flex w-full items-center gap-2.5 rounded-lg border px-2.5 py-3 text-left transition-colors ${entry?.id === agent.id ? 'border-primary/30 bg-primary/5' : 'border-transparent hover:bg-muted/60'}`} aria-current={entry?.id === agent.id ? 'page' : undefined} onclick={() => selected = agent.id}>
              <span class="flex size-8 shrink-0 items-center justify-center rounded-lg border bg-background"><Bot class="size-4 text-muted-foreground" /></span>
              <span class="min-w-0 flex-1"><strong class="block truncate text-xs font-medium">{agent.name}</strong><span class="mt-1 block text-[10px] text-muted-foreground">{status(agent.id)}</span></span>
              {#if entry?.id === agent.id}<ChevronRight class="size-3 text-muted-foreground" />{/if}
            </button>
          {/each}
          {#if $catalog.loading}<div class="flex justify-center py-10"><LoaderCircle class="size-5 animate-spin text-muted-foreground" /></div>{/if}
        </div>
      </nav>
      {#if entry}
        <div class="min-w-0 space-y-4 rounded-xl border bg-card p-5">
          <div class="flex items-start gap-3"><div class="flex size-11 items-center justify-center rounded-xl border bg-muted/30"><Bot class="size-6" /></div><div class="min-w-0 flex-1"><h4 class="m-0 text-base font-semibold">{entry.name}</h4><p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{setup ? tr(...setup.description) : tr('连接你选择的智能体，使用已有的登录与项目环境。', 'Connect this agent using its existing login and project environment.')}</p></div></div>
          <div class="flex flex-wrap gap-2 text-[10px]"><span class="rounded-full border px-2 py-1">{entry.connection_kind === 'bridge' ? tr('通过 ACP 桥接', 'ACP bridge') : tr('原生 ACP', 'Native ACP')}</span><span class="rounded-full border px-2 py-1">{tr('推荐版本', 'Recommended')} {entry.distribution.kind === 'npm' ? entry.distribution.pinned_version : entry.distribution.version}</span>{#if inspection?.version}<span class="rounded-full border px-2 py-1">{tr('本机版本', 'Installed')} {inspection.version}</span>{/if}</div>
          <section class="space-y-3 rounded-lg border p-3">
            <div class="flex items-center justify-between"><h5 class="m-0 text-xs font-medium">{tr('运行环境', 'Environment')}</h5><Button variant="ghost" size="sm" disabled={checking || installing} onclick={() => entry && void catalog.inspect(entry.id)}><RefreshCw class={`size-3.5 ${checking ? 'animate-spin' : ''}`} />{tr('重新检测', 'Check again')}</Button></div>
            {#if inspection}
              {#each inspection.checks as check}<div class="flex items-start gap-2 text-xs leading-5">{#if check.status === 'pass'}<CheckCircle2 class="mt-0.5 size-3.5 shrink-0 text-emerald-600" />{:else}<XCircle class="mt-0.5 size-3.5 shrink-0 text-amber-600" />{/if}<span class="break-words">{check.message}</span></div>{/each}
            {:else}<p class="m-0 text-xs text-muted-foreground">{tr('正在检查本机安装…', 'Checking installed programs…')}</p>{/if}
            {#if entry.distribution.kind === 'npm'}
              <div class="flex flex-wrap items-center gap-2 border-t pt-3"><Button size="sm" variant={inspection?.command ? 'outline' : 'default'} disabled={installing || checking} onclick={() => entry && void catalog.install(entry.id)}><Download class="size-3.5" />{tr(inspection?.command ? '安装推荐版本' : '安装智能体', inspection?.command ? 'Install recommended version' : 'Install agent')}</Button><span class="text-[10px] text-muted-foreground">{tr('由 RambleDesk 管理安装位置与版本', 'Installed and versioned by RambleDesk')}</span></div>
            {:else}<p class="m-0 text-xs leading-5 text-muted-foreground">{entry.distribution.instructions}</p><a class="inline-flex items-center gap-1 text-xs underline" href={entry.distribution.docs_url} target="_blank" rel="noreferrer">{tr('打开安装说明', 'Installation guide')}<ExternalLink class="size-3" /></a>{/if}
            {#each entry.dependencies as dependency}<p class="m-0 text-[11px] leading-5 text-muted-foreground">{dependency.instructions}</p>{/each}
          </section>
          {#if job}
            <section class="space-y-2 rounded-lg border bg-muted/20 p-3" aria-live="polite"><div class="flex items-center gap-2 text-xs">{#if installing}<LoaderCircle class="size-3.5 animate-spin" />{:else if job.phase === 'complete'}<Check class="size-3.5 text-emerald-600" />{/if}<span>{tr('安装任务', 'Installation')}: {job.phase}</span>{#if installing}<Button variant="ghost" size="sm" class="ml-auto" disabled={job.cancel_requested} onclick={() => job && void catalog.cancel(job.id)}>{tr(job.cancel_requested ? '正在取消…' : '取消', job.cancel_requested ? 'Cancelling…' : 'Cancel')}</Button>{/if}</div><pre class="m-0 max-h-36 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-muted-foreground">{job.messages.join('\n')}</pre></section>
          {/if}
          <section class="space-y-3">
            <h5 class="m-0 text-xs font-medium">{tr('连接与认证', 'Connection and authentication')}</h5>
            <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('默认复用智能体已有的登录和模型设置。保存配置后检查连接，再从会话列表选择它。', 'Your agent’s existing login and model settings are reused. Save and check the configuration, then select it in a new session.')}</p>
            {#if setup?.key}<details class="rounded-lg border p-3"><summary class="cursor-pointer text-xs">{tr('配置 API 连接', 'Configure API access')}</summary><div class="mt-3 space-y-3"><label class="block space-y-1 text-xs">{tr('API 密钥 / 访问令牌', 'API key / access token')}<input type="password" bind:value={apiKey} autocomplete="off" class="h-9 w-full rounded-md border bg-background px-3" /></label>{#if setup.baseUrl}<label class="block space-y-1 text-xs">{tr('服务地址（可选）', 'Base URL (optional)')}<input bind:value={apiUrl} placeholder={setup.endpoint ?? 'https://…'} class="h-9 w-full rounded-md border bg-background px-3" /></label>{/if}{#if setup.model}<label class="block space-y-1 text-xs">{tr('默认模型（可选）', 'Default model (optional)')}<input bind:value={model} class="h-9 w-full rounded-md border bg-background px-3" /></label>{/if}<p class="m-0 text-[10px] text-muted-foreground">{tr('密钥随启动配置保存在本机。留空会保留已有配置。', 'Keys are saved locally with the launch configuration. Empty fields keep existing values.')}</p></div></details>{/if}
            {#if setup?.login && !profiles.length}<p class="m-0 text-xs leading-5 text-muted-foreground">{tr('首次使用需要完成智能体登录：', 'Sign in to the agent before first use:')} <code class="select-all rounded bg-muted px-1.5 py-0.5">{setup.login}</code></p>{/if}
            {#if setup?.guide}<a class="inline-flex items-center gap-1 text-xs underline" href={setup.guide} target="_blank" rel="noreferrer">{tr('登录与密钥说明', 'Sign-in and API key guide')}<ExternalLink class="size-3" /></a>{/if}
            {#if entry.verification.status === 'unsupported'}<p class="m-0 rounded-lg border border-amber-500/30 bg-amber-500/5 p-3 text-xs leading-5">{entry.verification.note}</p>{/if}
            <Button size="sm" disabled={saving || installing || checking || !inspection?.command || inspection.checks.some(check => check.status === 'fail') || entry.verification.status === 'unsupported'} onclick={() => void useAgent(profiles[0])}>{#if saving}<LoaderCircle class="size-3.5 animate-spin" />{:else}<Check class="size-3.5" />{/if}{tr(profiles.length ? '保存并检查配置' : '使用此智能体', profiles.length ? 'Save and check' : 'Use this agent')}</Button>
            {#if notice}<p role="status" class="m-0 text-xs leading-5">{notice}</p>{/if}
            {#if checked}<p role="status" class={`m-0 text-xs leading-5 ${checked.ok ? 'text-emerald-600' : 'text-amber-600'}`}>{checked.message}</p>{/if}
          </section>
          {#if profiles.length}<details class="border-t pt-3"><summary class="flex cursor-pointer items-center gap-2 text-xs text-muted-foreground"><Settings2 class="size-3.5" />{tr('高级启动配置', 'Advanced launch configuration')}</summary><div class="mt-4"><AgentSettings configs={profiles} onSave={settings.save} onDelete={settings.remove} onCheck={settings.check} /></div></details>{/if}
        </div>
      {/if}
    </div>
  {/if}
</section>
