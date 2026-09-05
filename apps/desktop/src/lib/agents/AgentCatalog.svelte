<!-- Installation detail flow adapted from Codeg 3ebdfed acp-agent-settings.tsx (Apache-2.0). -->
<script lang="ts">
  import { onMount } from 'svelte'
  import { Bot, CheckCircle2, ChevronRight, Download, ExternalLink, LoaderCircle, Plus, RefreshCw, Settings2, XCircle } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { SaveAgentConfigInput } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { agentListItems, catalogConfiguration, createAgentCatalogController, installIsActive, type AgentListItem } from './agentCatalogController'
  import AgentSettings from './AgentSettings.svelte'
  import { AGENT_SETUP, applyAgentCredentials } from './agentOnboarding'
  import { AgentDraftCache, redactAgentMessage } from './agentConfigForm'
  import { createAgentSettingsController } from './agentSettingsController'
  export let transport: ApplicationTransport
  const catalog = createAgentCatalogController(transport)
  const settings = createAgentSettingsController(transport)
  const cache = new AgentDraftCache()
  const baselines = new Map<string, string>()
  let selected = ''
  let saving = false
  let localError = ''
  let notice = ''
  let apiKey = ''
  let apiUrl = ''
  let model = ''
  let checked: { ok: boolean; message: string } | undefined
  let selectedBefore = ''
  let inspectedCatalog = ''
  $: items = agentListItems($catalog.entries, $settings.configs)
  $: item = selected === 'new' ? undefined : items.find(item => item.key === selected) ?? items[0]
  $: entry = item?.entry
  $: profile = item?.config
  $: setup = entry ? AGENT_SETUP[entry.id] : undefined
  $: inspection = entry ? $catalog.inspections[entry.id] : undefined
  $: checking = entry ? $catalog.checking.includes(entry.id) : false
  $: job = entry ? $catalog.jobs.filter(job => job.agent_id === entry.id).at(-1) : undefined
  $: installing = job ? installIsActive(job) : false
  $: ready = !!inspection?.command && !inspection.checks.some(check => check.status === 'fail') && entry?.verification.status !== 'unsupported'
  $: locked = saving || $settings.loading
  $: safeError = redactAgentMessage(localError || $settings.error || $catalog.error, environmentText())
  $: if ((item?.key ?? 'new') !== selectedBefore) {
    selectedBefore = item?.key ?? 'new'; localError = ''; notice = ''; checked = undefined; apiKey = ''; apiUrl = ''; model = ''
  }
  $: if ($catalog.entries.map(entry => entry.id).join('|') !== inspectedCatalog) {
    inspectedCatalog = $catalog.entries.map(entry => entry.id).join('|')
    void catalog.inspectAll()
  }
  onMount(() => { const a = catalog.start(); const b = settings.start(); return () => { a(); b() } })
  function tr(zh: string, en: string) { return $locale === 'zh-CN' ? zh : en }
  function environmentText() { return [...Object.entries(profile?.env ?? {}).map(([key, value]) => `${key}=${value}`), `API_KEY=${apiKey}`].join('\n') }
  function status(row: AgentListItem) {
    if (row.config) return tr('已配置 · 可在新会话中选择', 'Configured · available in new sessions')
    const id = row.entry!.id
    if ($catalog.checking.includes(id)) return tr('检测中', 'Checking')
    const info = $catalog.inspections[id]
    if (!info) return tr('待检测', 'Not checked')
    if (info.source === 'missing') return tr('未安装', 'Not installed')
    if (info.checks.some(check => check.status === 'fail')) return tr('需要处理', 'Needs attention')
    return tr('已安装 · 可用于新会话', 'Installed · available for sessions')
  }
  function failure(error: unknown, environment = environmentText()) {
    const message = typeof error === 'object' && error && 'message' in error ? String(error.message) : String(error)
    localError = redactAgentMessage(message, environment)
  }
  async function saveProfile(input: SaveAgentConfigInput) {
    const wasSaving = saving
    saving = true
    try {
      const saved = await settings.save(input)
      selected = `config:${saved.id}`
      return saved
    } finally { saving = wasSaving }
  }
  async function removeProfile(id: string) {
    const catalogId = entry?.id
    saving = true
    try {
      await settings.remove(id)
      cache.remove(id); baselines.delete(id)
      selected = catalogId ? `catalog:${catalogId}` : ''
    } finally { saving = false }
  }
  async function resolveProfile() {
    if (profile) return profile
    if (!entry) throw new Error(tr('请选择智能体。', 'Choose an agent.'))
    const saved = await settings.resolve({ agent_id: entry.id, enable: false })
    selected = `config:${saved.id}`
    return saved
  }
  async function addInstance() {
    if (locked || !entry || !inspection || !ready) return
    saving = true; localError = ''
    try {
      const input = catalogConfiguration(entry, inspection)
      const number = $settings.configs.filter(config => config.catalog_id === entry!.id).length + 1
      await saveProfile({ ...input, name: `${entry.name} (${number})` })
    } catch (error) { failure(error) }
    finally { saving = false }
  }
  async function action(kind: 'credentials' | 'check' | 'edit') {
    if (saving) return
    saving = true; localError = ''; notice = ''; checked = undefined
    // Capture entered values before resolving changes the selected list row.
    const credentials = { key: apiKey, baseUrl: apiUrl, model }
    const operationEnvironment = environmentText()
    const selectedSetup = setup
    try {
      const saved = await resolveProfile()
      if (kind === 'credentials') {
        await saveProfile({ ...saved, env: applyAgentCredentials(saved.env, selectedSetup, credentials) })
        apiKey = ''; apiUrl = ''; model = ''
        notice = tr('连接设置已保存，下次启动时生效。', 'Connection settings saved for the next launch.')
      } else if (kind === 'check') {
        const result = await settings.check(saved.id)
        checked = { ...result, message: redactAgentMessage(result.message, Object.entries(saved.env).map(([key, value]) => `${key}=${value}`).join('\n')) }
      }
    } catch (error) { failure(error, operationEnvironment) }
    finally { saving = false }
  }
</script>

<section class="space-y-4 @container" aria-label={tr('智能体管理', 'Agent management')}>
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div><h3 class="m-0 text-sm font-semibold">{tr('智能体', 'Agents')}</h3><p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('查看和管理设备上的智能体。已安装的智能体可直接在新会话中选择。', 'Manage agents on this device. Installed agents are available directly when creating a session.')}</p></div>
    <Button variant="outline" size="sm" disabled={locked} onclick={() => selected = 'new'}><Plus class="size-3.5" />{tr('添加智能体', 'Add agent')}</Button>
  </div>
  {#if safeError}<p role="alert" class="rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-xs">{safeError}</p>{/if}
  <div class="grid min-w-0 gap-3 @min-[680px]:grid-cols-[230px_minmax(0,1fr)]">
    <nav class="overflow-hidden rounded-xl border bg-card" aria-label={tr('智能体列表', 'Agent list')}>
      <div class="flex items-center justify-between border-b px-3 py-2 text-[11px] font-medium text-muted-foreground"><span>{tr('智能体', 'Agents')} · {items.length}</span><Button variant="ghost" size="icon-sm" disabled={$catalog.checking.length > 0} aria-label={tr('重新检测所有智能体', 'Check all agents')} onclick={() => void catalog.inspectAll()}><RefreshCw class="size-3" /></Button></div>
      <div class="max-h-[640px] space-y-1 overflow-y-auto p-2">
        {#each items as row (row.key)}
          <button type="button" disabled={locked} class={`flex w-full items-center gap-2.5 rounded-lg border px-2.5 py-3 text-left transition-colors disabled:opacity-50 ${item?.key === row.key ? 'border-primary/30 bg-primary/5' : 'border-transparent hover:bg-muted/60'}`} aria-current={item?.key === row.key ? 'page' : undefined} onclick={() => selected = row.key}>
            <span class="flex size-8 shrink-0 items-center justify-center rounded-lg border bg-background"><Bot class="size-4 text-muted-foreground" /></span>
            <span class="min-w-0 flex-1"><strong class="block truncate text-xs font-medium">{row.name}</strong><span class="mt-1 block text-[10px] text-muted-foreground">{status(row)}</span>{#if row.config}<span class="mt-1 block truncate text-[10px] text-muted-foreground">{row.entry?.name ?? tr('自定义 ACP 智能体', 'Custom ACP agent')}</span>{/if}</span>
            {#if item?.key === row.key}<ChevronRight class="size-3 text-muted-foreground" />{/if}
          </button>
        {/each}
        {#if selected === 'new'}<div aria-current="page" class="rounded-lg border border-dashed px-3 py-3 text-xs">{tr('新智能体', 'New agent')}</div>{/if}
        {#if $catalog.loading || $settings.loading}<div class="flex justify-center py-5"><LoaderCircle class="size-5 animate-spin text-muted-foreground" /></div>{/if}
      </div>
    </nav>
    <div class="min-w-0 space-y-4 rounded-xl border bg-card p-5">
      {#if selected === 'new'}
        <h4 class="m-0 text-base font-semibold">{tr('添加智能体', 'Add agent')}</h4>
        <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('连接已安装的 ACP 程序，保存后会出现在同一个智能体列表中。', 'Connect an installed ACP program. It will appear in this agent list after saving.')}</p>
        {#key selected}<AgentSettings {cache} {baselines} busy={locked} onSave={saveProfile} onDelete={removeProfile} onCheck={settings.check} />{/key}
      {:else if item}
        <div class="flex items-start gap-3"><div class="flex size-11 items-center justify-center rounded-xl border bg-muted/30"><Bot class="size-6" /></div><div class="min-w-0 flex-1"><h4 class="m-0 text-base font-semibold">{item.name}</h4><p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{setup ? tr(...setup.description) : tr('使用已有的登录与项目环境连接此智能体。', 'Connect this agent using its existing login and project environment.')}</p></div></div>
        {#if entry}
          <div class="flex flex-wrap gap-2 text-[10px]"><span class="rounded-full border px-2 py-1">{entry.connection_kind === 'bridge' ? tr('通过 ACP 桥接', 'ACP bridge') : tr('原生 ACP', 'Native ACP')}</span><span class="rounded-full border px-2 py-1">{tr('推荐版本', 'Recommended')} {entry.distribution.kind === 'npm' ? entry.distribution.pinned_version : entry.distribution.version}</span>{#if inspection?.version}<span class="rounded-full border px-2 py-1">{tr('本机版本', 'Installed')} {inspection.version}</span>{/if}</div>
          <section class="space-y-3 rounded-lg border p-3">
            <div class="flex items-center justify-between"><h5 class="m-0 text-xs font-medium">{tr('安装与运行环境', 'Installation and environment')}</h5><Button variant="ghost" size="sm" disabled={checking || installing} onclick={() => entry && void catalog.inspect(entry.id)}><RefreshCw class={`size-3.5 ${checking ? 'animate-spin' : ''}`} />{tr('重新检测', 'Check again')}</Button></div>
            {#if inspection}{#each inspection.checks as check}<div class="flex items-start gap-2 text-xs leading-5">{#if check.status === 'pass'}<CheckCircle2 class="mt-0.5 size-3.5 shrink-0 text-emerald-600" />{:else}<XCircle class="mt-0.5 size-3.5 shrink-0 text-amber-600" />{/if}<span class="break-words">{check.message}</span></div>{/each}{:else}<p class="m-0 text-xs text-muted-foreground">{tr('正在检查本机安装…', 'Checking installed programs…')}</p>{/if}
            {#if entry.distribution.kind === 'npm'}<div class="flex flex-wrap items-center gap-2 border-t pt-3"><Button size="sm" variant={inspection?.command ? 'outline' : 'default'} disabled={installing || checking} onclick={() => entry && void catalog.install(entry.id)}><Download class="size-3.5" />{tr(inspection?.command ? '安装推荐版本' : '安装智能体', inspection?.command ? 'Install recommended version' : 'Install agent')}</Button><span class="text-[10px] text-muted-foreground">{tr('由 RambleDesk 管理安装位置与版本', 'Installed and versioned by RambleDesk')}</span></div>
            {:else}<p class="m-0 text-xs leading-5 text-muted-foreground">{entry.distribution.instructions}</p><a class="inline-flex items-center gap-1 text-xs underline" href={entry.distribution.docs_url} target="_blank" rel="noreferrer">{tr('打开安装说明', 'Installation guide')}<ExternalLink class="size-3" /></a>{/if}
            {#each entry.dependencies as dependency}<p class="m-0 text-[11px] leading-5 text-muted-foreground">{dependency.instructions}</p>{/each}
          </section>
          {#if job}<section class="space-y-2 rounded-lg border bg-muted/20 p-3" aria-live="polite"><div class="flex items-center gap-2 text-xs">{#if installing}<LoaderCircle class="size-3.5 animate-spin" />{/if}<span>{tr('安装任务', 'Installation')}: {job.phase}</span>{#if installing}<Button variant="ghost" size="sm" class="ml-auto" disabled={job.cancel_requested} onclick={() => job && void catalog.cancel(job.id)}>{tr(job.cancel_requested ? '正在取消…' : '取消', job.cancel_requested ? 'Cancelling…' : 'Cancel')}</Button>{/if}</div><pre class="m-0 max-h-36 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-muted-foreground">{job.messages.join('\n')}</pre></section>{/if}
          <section class="space-y-3">
            <h5 class="m-0 text-xs font-medium">{tr('登录与连接', 'Sign-in and connection')}</h5>
            <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('默认使用智能体已有的登录和模型设置。已安装即可在新会话中选择；连接检查可帮助排查问题。', 'Your agent’s existing login and model settings are reused. Select an installed agent in a new session; a connection check can help diagnose issues.')}</p>
            {#if setup?.login}<p class="m-0 text-xs leading-5 text-muted-foreground">{tr('智能体登录命令：', 'Agent sign-in command:')} <code class="select-all rounded bg-muted px-1.5 py-0.5">{setup.login}</code></p>{/if}
            {#if setup?.guide}<a class="inline-flex items-center gap-1 text-xs underline" href={setup.guide} target="_blank" rel="noreferrer">{tr('登录与密钥说明', 'Sign-in and API key guide')}<ExternalLink class="size-3" /></a>{/if}
            {#if setup?.key}<details class="rounded-lg border p-3"><summary class="cursor-pointer text-xs">{tr('API 连接设置', 'API connection settings')}</summary><div class="mt-3 space-y-3"><label class="block space-y-1 text-xs">{tr('API 密钥 / 访问令牌', 'API key / access token')}<input type="password" bind:value={apiKey} autocomplete="off" class="h-9 w-full rounded-md border bg-background px-3" /></label>{#if setup.baseUrl}<label class="block space-y-1 text-xs">{tr('服务地址（可选）', 'Base URL (optional)')}<input bind:value={apiUrl} placeholder={setup.endpoint ?? 'https://…'} class="h-9 w-full rounded-md border bg-background px-3" /></label>{/if}{#if setup.model}<label class="block space-y-1 text-xs">{tr('默认模型（可选）', 'Default model (optional)')}<input bind:value={model} class="h-9 w-full rounded-md border bg-background px-3" /></label>{/if}<p class="m-0 text-[10px] text-muted-foreground">{tr('留空保留已有值。保存仅更新填写的连接设置。', 'Empty fields keep existing values. Saving updates only the connection settings you enter.')}</p><Button size="sm" disabled={locked || (!profile && !ready) || !(apiKey.trim() || apiUrl.trim() || model.trim())} onclick={() => void action('credentials')}>{tr('保存连接设置', 'Save connection settings')}</Button></div></details>{/if}
            <Button variant="outline" size="sm" disabled={locked || installing || (!profile && !ready)} onclick={() => void action('check')}>{#if saving}<LoaderCircle class="size-3.5 animate-spin" />{/if}{tr('检查连接', 'Check connection')}</Button>
          </section>
        {/if}
        {#if notice}<p role="status" class="m-0 text-xs leading-5">{notice}</p>{/if}
        {#if checked}<p role="status" class={`m-0 text-xs leading-5 ${checked.ok ? 'text-emerald-600' : 'text-amber-600'}`}>{checked.message}</p>{/if}
        {#if profile}
          {#key profile.id}<details open={!entry} class="border-t pt-3"><summary class="flex cursor-pointer items-center gap-2 text-xs text-muted-foreground"><Settings2 class="size-3.5" />{tr('启动设置', 'Launch settings')}</summary><div class="mt-4"><AgentSettings {cache} {baselines} configs={[profile]} busy={locked} onSave={saveProfile} onDelete={removeProfile} onCheck={settings.check} /></div></details>{/key}
          {#if entry && ready}<div class="space-y-1 border-t pt-3"><Button variant="ghost" size="sm" disabled={locked} onclick={() => void addInstance()}><Plus class="size-3.5" />{tr('添加另一个实例', 'Add another instance')}</Button><p class="m-0 text-[10px] leading-5 text-muted-foreground">{tr('使用当前安装版本创建独立启动设置，适合其他账号或不同环境。', 'Create separate launch settings from the current installation for another account or environment.')}</p></div>{/if}
        {:else if ready}<div class="border-t pt-3"><Button variant="ghost" size="sm" disabled={locked} onclick={() => void action('edit')}><Settings2 class="size-3.5" />{tr('编辑启动设置', 'Edit launch settings')}</Button></div>{/if}
      {/if}
    </div>
  </div>
</section>
