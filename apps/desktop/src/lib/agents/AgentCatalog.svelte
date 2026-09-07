<!-- Installation detail flow adapted from Codeg 3ebdfed acp-agent-settings.tsx (Apache-2.0). -->
<script lang="ts">
  import { onMount } from 'svelte'
  import { CheckCircle2, ChevronRight, Download, LoaderCircle, Plus, RefreshCw, Settings2, XCircle } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { AgentConfig, SaveAgentConfigInput } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { agentConnectionResult, agentListItems, agentStatus, connectionPreparationAvailable, createAgentCatalogController, installIsActive, manualAgentConfiguration, type AgentStatus } from './agentCatalogController'
  import AgentSettings from './AgentSettings.svelte'
  import AgentSetupGuide from './AgentSetupGuide.svelte'
  import AgentIcon from './AgentIcon.svelte'
  import { AgentDraftCache, redactAgentMessage } from './agentConfigForm'

  export let transport: ApplicationTransport
  export let autoDetect = false
  export let onReady: ((config: AgentConfig | null) => void) | undefined = undefined
  export let initialConfigId: string | undefined = undefined
  // Capture the initial value once. Parent onReady updates must not steer later selection.
  const initialSelection = initialConfigId
  let selectingInitial = Boolean(initialSelection)
  const catalog = createAgentCatalogController(transport)
  const cache = new AgentDraftCache()
  const baselines = new Map<string, string>()
  let selected = ''
  let selectedProfile = ''
  let selectedBefore = ''
  let saving = false
  let localError = ''
  let notice = ''
  let lastReady: string | null | undefined = undefined
  let manualPaths: Record<string, string> = {}
  $: items = agentListItems($catalog.entries, $catalog.configs)
  $: item = selected === 'new' ? undefined : items.find(row => row.key === selected) ?? items[0]
  $: entry = item?.entry
  $: profile = item?.configs.find(config => config.id === selectedProfile) ?? item?.config
  $: inspection = entry ? $catalog.inspections[entry.id] : undefined
  $: status = item ? agentStatus({ ...item, config: profile }, $catalog) : 'attention'
  $: checked = agentConnectionResult(profile, $catalog)
  $: if (!selectingInitial) {
    if (profile?.enabled && checked?.ok) {
      const ready = `${profile.id}:${profile.updated_at}`
      if (ready !== lastReady) { lastReady = ready; onReady?.(profile) }
    } else if (lastReady !== null) { lastReady = null; onReady?.(null) }
  }
  $: checking = status === 'checking'
  $: job = entry ? $catalog.jobs.filter(job => job.agent_id === entry.id).at(-1) : undefined
  $: installing = job ? installIsActive(job) : false
  $: canPrepare = connectionPreparationAvailable(entry, inspection)
  $: canCheck = !!profile || (!!inspection?.command && !inspection.checks.some(check => check.status === 'fail') && entry?.verification.status !== 'unsupported')
  $: safeError = redactAgentMessage(localError || $catalog.error, environmentText())
  $: if ((item?.key ?? 'new') !== selectedBefore) {
    selectedBefore = item?.key ?? 'new'; selectedProfile = ''; localError = ''; notice = ''
  }
  onMount(() => {
    const dispose = catalog.start()
    let mounted = true
    if (selectingInitial) void catalog.refresh().then(() => {
      if (!mounted || !selectingInitial) return
      const row = agentListItems($catalog.entries, $catalog.configs).find(row => row.configs.some(config => config.id === initialSelection))
      if (row) { selected = row.key; selectedBefore = row.key; selectedProfile = initialSelection! }
      selectingInitial = false
    })
    if (autoDetect) void catalog.detectAll('onboarding')
    const refresh = () => { if (document.visibilityState === 'visible') void catalog.refresh() }
    window.addEventListener('focus', refresh)
    return () => { mounted = false; window.removeEventListener('focus', refresh); dispose() }
  })
  function selectRow(key: string) { selectingInitial = false; selected = key }
  function tr(zh: string, en: string) { return $locale === 'zh-CN' ? zh : en }
  function environmentText() { return $catalog.configs.flatMap(config => Object.entries(config.env).map(([key, value]) => `${key}=${value}`)).join('\n') }
  function statusText(status: AgentStatus) {
    return { unchecked: tr('未检测', 'Not checked'), missing: tr('未发现', 'Not found'), prepare: tr('需要准备连接', 'Connection setup needed'), checking: tr('正在检测', 'Checking'), connected: tr('连接成功', 'Connected'), attention: tr('需要处理', 'Needs attention') }[status]
  }
  function failure(error: unknown) {
    const message = typeof error === 'object' && error && 'message' in error ? String(error.message) : String(error)
    localError = redactAgentMessage(message, environmentText())
  }
  async function saveProfile(input: SaveAgentConfigInput) {
    saving = true
    try {
      const saved = await catalog.save(input)
      selected = $catalog.entries.some(entry => entry.id === saved.catalog_id) ? `catalog:${saved.catalog_id}` : `config:${saved.id}`
      selectedProfile = saved.id
      return saved
    } finally { saving = false }
  }
  async function removeProfile(id: string) {
    const catalogId = entry?.id
    saving = true
    try {
      await catalog.remove(id)
      cache.remove(id); baselines.delete(id)
      selected = catalogId ? `catalog:${catalogId}` : ''
      selectedProfile = ''
    } finally { saving = false }
  }
  async function retry() {
    if (saving || checking || installing) return
    localError = ''; notice = ''
    try {
      if (entry) await catalog.inspect(entry.id)
      if (profile) await catalog.check(profile.id)
      else if (entry) await catalog.checkAgent(entry.id)
    } catch (error) { failure(error) }
  }
  async function usePath() {
    if (!entry || saving) return
    localError = ''; notice = ''
    try {
      const saved = await saveProfile(manualAgentConfiguration(entry, manualPaths[entry.id] ?? '', profile))
      notice = tr('程序位置已保存。', 'Program location saved.')
      await catalog.check(saved.id)
    } catch (error) {
      const text = error instanceof Error ? error.message : ''
      if (text.startsWith('Enter the full path')) localError = tr('请输入 CLI 或 ACP 可执行文件的完整路径。', text)
      else if (text.startsWith('JavaScript entry')) localError = tr('JavaScript 入口需要运行时，请在高级启动设置中填写运行时命令和脚本参数。', text)
      else failure(error)
    }
  }
  async function prepareAdvanced() {
    if (!entry || saving) return
    saving = true; localError = ''
    try { const saved = await catalog.resolve(entry.id); selectedProfile = saved.id }
    catch (error) { failure(error) }
    finally { saving = false }
  }
</script>

<section class="space-y-4 @container" aria-label={tr('智能体管理', 'Agent management')}>
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div><h3 class="m-0 text-sm font-semibold">{tr('智能体', 'Agents')}</h3><p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('管理设备上的智能体及其连接。登录和模型配置在智能体中完成。', 'Manage agents and their connections on this device. Complete sign-in and model setup in the agent.')}</p></div>
    <Button variant="outline" size="sm" disabled={$catalog.loading || $catalog.checking.length > 0 || $catalog.connecting.length > 0 || saving} onclick={() => void catalog.detectAll()}><RefreshCw class={`size-3.5 ${$catalog.checking.length > 0 || $catalog.connecting.length > 0 ? 'animate-spin' : ''}`} />{tr('检测智能体', 'Detect agents')}</Button>
  </div>
  {#if safeError}<p role="alert" class="break-words rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-xs">{safeError}</p>{/if}
  <div class="grid min-w-0 gap-3 @min-[680px]:grid-cols-[210px_minmax(0,1fr)]">
    <nav class="overflow-hidden rounded-xl border bg-card" aria-label={tr('智能体列表', 'Agent list')}>
      <div class="border-b px-3 py-3 text-[11px] font-medium text-muted-foreground">{tr('此设备上的智能体', 'Agents on this device')} · {items.length}</div>
      <div class="max-h-[560px] space-y-1 overflow-y-auto p-2">
        {#each items as row (row.key)}
          <button type="button" disabled={saving} class={`flex w-full items-center gap-2.5 rounded-lg border px-2.5 py-3 text-left transition-colors disabled:opacity-50 ${item?.key === row.key ? 'border-primary/30 bg-primary/5' : 'border-transparent hover:bg-muted/60'}`} aria-current={item?.key === row.key ? 'page' : undefined} onclick={() => selectRow(row.key)}>
            <span class="flex size-8 shrink-0 items-center justify-center rounded-lg border bg-background"><AgentIcon hostId={row.config?.host_id ?? row.entry?.host_id} class="size-4" /></span>
            <span class="min-w-0 flex-1"><strong class="block truncate text-xs font-medium">{row.name}</strong><span class={`mt-1 block text-[10px] ${agentStatus(row, $catalog) === 'connected' ? 'text-emerald-600' : 'text-muted-foreground'}`}>{statusText(agentStatus(row, $catalog))}</span></span>
            {#if item?.key === row.key}<ChevronRight class="size-3 shrink-0 text-muted-foreground" />{/if}
          </button>
        {/each}
        {#if selected === 'new'}<div aria-current="page" class="rounded-lg border border-dashed px-3 py-3 text-xs">{tr('自定义 ACP', 'Custom ACP')}</div>{/if}
        {#if $catalog.loading && !items.length}<div class="flex justify-center py-5"><LoaderCircle class="size-5 animate-spin text-muted-foreground" /></div>{/if}
      </div>
      <details class="border-t px-3 py-3"><summary class="cursor-pointer text-[11px] text-muted-foreground">{tr('高级', 'Advanced')}</summary><Button class="mt-2" variant="ghost" size="sm" disabled={saving} onclick={() => selected = 'new'}><Plus class="size-3.5" />{tr('自定义 ACP 智能体', 'Custom ACP agent')}</Button></details>
    </nav>
    <div class="min-w-0 space-y-4 rounded-xl border bg-card p-5">
      {#if selected === 'new'}
        <h4 class="m-0 text-base font-semibold">{tr('自定义 ACP 智能体', 'Custom ACP agent')}</h4>
        <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('连接已经安装的 ACP 程序。启动参数和环境变量可在这里手动设置。', 'Connect an installed ACP program with custom launch arguments and environment variables.')}</p>
        {#key selected}<AgentSettings {cache} {baselines} busy={saving} onSave={saveProfile} onDelete={removeProfile} onCheck={catalog.check} />{/key}
      {:else if item}
        <div class="flex items-start gap-3"><div class="flex size-11 shrink-0 items-center justify-center rounded-xl border bg-muted/30"><AgentIcon hostId={profile?.host_id ?? entry?.host_id} class="size-6" /></div><div class="min-w-0 flex-1"><h4 class="m-0 text-base font-semibold">{item.name}</h4><p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('使用智能体自身的登录和配置环境。', 'Uses the agent’s own sign-in and configuration environment.')}</p></div></div>
        <section class="space-y-3 rounded-lg border bg-muted/15 p-4" aria-live="polite">
          <div class="flex items-center gap-2 text-sm font-medium">{#if checking || installing}<LoaderCircle class="size-4 animate-spin" />{:else if status === 'connected'}<CheckCircle2 class="size-4 text-emerald-600" />{:else if status === 'attention'}<XCircle class="size-4 text-amber-600" />{/if}<span>{installing ? tr('正在准备连接', 'Preparing connection') : statusText(status)}</span></div>
          <p class="m-0 text-xs leading-5 text-muted-foreground">
            {#if installing}{tr('连接组件准备好后会自动检查。', 'The connection will be checked when setup finishes.')}
            {:else if status === 'connected'}{tr('可以在新会话中选择此智能体。', 'Select this agent in a new session.')}
            {:else if status === 'checking'}{tr('正在检查本机程序和 ACP 连接…', 'Checking the local program and ACP connection…')}
            {:else if status === 'unchecked'}{tr('尚未检查此智能体。需要时点击检测，已有配置也可直接用于新会话。', 'This agent has not been checked. Detect it when needed, or use an existing configuration in a new session.')}
            {:else if status === 'prepare'}{tr('需要连接组件才能通过 RambleDesk 使用此智能体。', 'A connection component is needed to use this agent in RambleDesk.')}
            {:else if status === 'missing'}{tr('暂未找到可连接的程序。安装后重新检测，或指定已有程序的位置。', 'No connectable program was found. Check again after installing, or specify an existing program location.')}
            {:else if profile && !profile.enabled}{tr('此启动配置尚未启用。检查连接后即可继续使用。', 'This launch configuration is disabled. Check the connection to enable it.')}
            {:else}{tr('连接尚未确认。按照下方指引处理后，回来重试。', 'The connection is not confirmed. Follow the guidance below, then retry.')}{/if}
          </p>
          {#if checked && !checked.ok}<p class="m-0 break-words text-xs leading-5 text-amber-700 dark:text-amber-400">{checked.message}</p>{/if}
          {#if job?.phase === 'failed'}<p role="alert" class="m-0 text-xs leading-5 text-amber-700 dark:text-amber-400">{tr('连接组件准备失败。请查看高级诊断中的日志，处理后重新准备连接。', 'Connection setup failed. Review the log in advanced diagnostics, address the issue, then prepare the connection again.')}</p>{/if}
          <div class="flex flex-wrap gap-2">
            {#if canPrepare && (status === 'prepare' || (status === 'attention' && (!inspection?.command || inspection.dependencies.some(dependency => dependency.required && !dependency.path))))}<Button size="sm" disabled={saving || installing || checking} onclick={() => entry && void catalog.install(entry.id)}><Download class="size-3.5" />{tr('准备连接', 'Prepare connection')}</Button>{/if}
            <Button variant="outline" size="sm" disabled={saving || checking || installing} onclick={() => void retry()}><RefreshCw class="size-3.5" />{tr(canCheck ? '检查连接' : '重新检测', canCheck ? 'Check connection' : 'Check again')}</Button>
            {#if installing && job}<Button variant="ghost" size="sm" disabled={job.cancel_requested} onclick={() => job && void catalog.cancel(job.id)}>{tr(job.cancel_requested ? '正在取消…' : '取消', job.cancel_requested ? 'Cancelling…' : 'Cancel')}</Button>{/if}
          </div>
        </section>
        {#if !installing && (status === 'missing' || status === 'attention')}<AgentSetupGuide catalogId={entry?.id} hostId={profile?.host_id ?? entry?.host_id} name={item.name} config={profile} {inspection} />{/if}
        {#if entry && status !== 'connected' && status !== 'checking'}
          <details class="rounded-lg border p-3"><summary class="cursor-pointer text-xs">{tr('已安装？指定程序位置', 'Already installed? Specify its location')}</summary><form class="mt-3 space-y-3" onsubmit={(event) => { event.preventDefault(); void usePath() }}>
            <p class="m-0 text-xs leading-5 text-muted-foreground">{tr(`选择 ${entry.distribution.command} 的 CLI 或 ACP 可执行文件，而不是桌面应用或安装目录。RambleDesk 会补上此智能体的 ACP 启动参数。`, `Choose the ${entry.distribution.command} CLI or ACP executable, rather than a desktop app or installation folder. RambleDesk adds this agent’s ACP launch arguments.`)}</p>
            {#if entry.connection_kind === 'bridge'}<p class="m-0 text-xs leading-5 text-muted-foreground">{tr('这里需要 ACP 连接组件的入口；智能体的交互命令不能替代它。', 'This requires the ACP connection component entry point; the agent’s interactive command cannot replace it.')}</p>{/if}
            <label class="block space-y-1.5 text-xs"><span>{tr('可执行文件完整路径', 'Full executable path')}</span><input required bind:value={manualPaths[entry.id]} placeholder={tr('例如 D:\\agents\\程序.exe 或 /opt/agents/程序', 'For example D:\\agents\\program.exe or /opt/agents/program')} autocomplete="off" spellcheck="false" class="h-9 w-full rounded-md border bg-background px-3 font-mono" /></label>
            <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('JavaScript 脚本需要在高级启动设置中填写运行时与脚本参数。', 'For a JavaScript script, configure the runtime and script arguments in advanced launch settings.')}</p>
            <Button type="submit" size="sm" disabled={saving || checking || !manualPaths[entry.id]?.trim()}>{tr('保存并检查连接', 'Save and check connection')}</Button>
          </form></details>
        {/if}
        {#if notice}<p role="status" class="m-0 text-xs leading-5">{notice}</p>{/if}
        {#key item.key}<details open={!entry} class="border-t pt-3"><summary class="flex cursor-pointer items-center gap-2 text-xs text-muted-foreground"><Settings2 class="size-3.5" />{tr('高级与诊断', 'Advanced and diagnostics')}</summary><div class="mt-4 space-y-4">
          {#if entry}<div class="flex flex-wrap gap-2 text-[10px]"><span class="rounded-full border px-2 py-1">{entry.connection_kind === 'bridge' ? tr('ACP 连接组件', 'ACP connection component') : tr('原生 ACP', 'Native ACP')}</span><span class="rounded-full border px-2 py-1">{tr('推荐版本', 'Recommended')} {entry.distribution.kind === 'npm' ? entry.distribution.pinned_version : entry.distribution.version}</span>{#if inspection?.version}<span class="rounded-full border px-2 py-1">{tr('发现的版本', 'Discovered version')} {inspection.version}</span>{/if}</div>{/if}
          {#if profile}<div class="space-y-1 text-xs"><p class="m-0 font-medium">{tr('实际启动入口', 'Actual launch entry')}</p><code class="block break-all rounded bg-muted p-2 text-[11px]">{redactAgentMessage([profile.command, ...profile.args].join(' '), environmentText())}</code></div>{/if}
          {#if inspection}<div class="space-y-2"><h5 class="m-0 text-xs font-medium">{tr('自动发现结果', 'Discovery details')}</h5>{#each inspection.checks as check}<div class="flex items-start gap-2 text-xs leading-5">{#if check.status === 'pass'}<CheckCircle2 class="mt-0.5 size-3.5 shrink-0 text-emerald-600" />{:else}<XCircle class="mt-0.5 size-3.5 shrink-0 text-amber-600" />{/if}<span class="break-words">{redactAgentMessage(check.message, environmentText())}</span></div>{/each}</div>{/if}
          {#if checked?.details.length}<div class="space-y-2"><h5 class="m-0 text-xs font-medium">{tr('连接检查详情', 'Connection check details')}</h5><ul class="m-0 list-disc space-y-1 pl-4 text-xs leading-5 text-muted-foreground">{#each checked.details as detail}<li class="break-words">{detail}</li>{/each}</ul></div>{/if}
          {#if canPrepare}<div class="space-y-2"><Button variant="outline" size="sm" disabled={saving || installing || checking} onclick={() => entry && void catalog.install(entry.id)}><Download class="size-3.5" />{tr('修复连接组件', 'Repair connection component')}</Button><p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('安装推荐版本的 ACP 连接组件。已有的自定义启动配置会保留。', 'Install the recommended ACP connection component. Existing custom launch configurations are retained.')}</p></div>{/if}
          {#if job}<div class="space-y-2 rounded-lg border bg-muted/20 p-3"><p class="m-0 text-xs">{tr('连接准备日志', 'Connection setup log')}: {job.phase}</p><pre class="m-0 max-h-36 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-muted-foreground">{redactAgentMessage(job.messages.join('\n'), environmentText())}</pre></div>{/if}
          <AgentSetupGuide catalogId={entry?.id} hostId={profile?.host_id ?? entry?.host_id} name={item.name} config={profile} {inspection} compact />
          {#if item.configs.length > 1}<div class="space-y-2"><p class="m-0 text-xs font-medium">{tr('已有启动配置', 'Saved launch configurations')}</p><div class="flex flex-wrap gap-2">{#each item.configs as config}<Button size="sm" variant={config.id === profile?.id ? 'secondary' : 'outline'} disabled={saving} onclick={() => selectedProfile = config.id}>{config.name}</Button>{/each}</div></div>{/if}
          {#if profile}{#key profile.id}<AgentSettings {cache} {baselines} configs={[profile]} busy={saving} onSave={saveProfile} onDelete={removeProfile} onCheck={catalog.check} />{/key}
          {:else if canCheck}<Button variant="outline" size="sm" disabled={saving} onclick={() => void prepareAdvanced()}>{tr('编辑启动设置', 'Edit launch settings')}</Button>
          {:else}<Button variant="outline" size="sm" disabled={saving} onclick={() => selected = 'new'}>{tr('自定义 ACP 启动设置', 'Custom ACP launch settings')}</Button>{/if}
        </div></details>{/key}
      {/if}
    </div>
  </div>
</section>
