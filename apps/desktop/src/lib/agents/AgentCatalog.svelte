<!-- Installation detail flow adapted from Codeg 3ebdfed acp-agent-settings.tsx (Apache-2.0). -->
<script lang="ts">
  import { onMount } from 'svelte'
  import { CheckCircle2, ChevronRight, Download, ExternalLink, LoaderCircle, Plus, RefreshCw, Settings2, XCircle } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { AgentConfig, AgentInspection, SaveAgentConfigInput } from '$lib/generated/feedback'
  import type { AgentDiagnosis } from './agentDiagnosis'
  import { locale } from '$lib/preferences'
  import { agentConnectionResult, agentDiagnosis, agentListItems, agentStatus, connectionPreparationAvailable, createAgentCatalogController, installIsActive, manualAgentConfiguration, type AgentStatus } from './agentCatalogController'
  import AgentSettings from './AgentSettings.svelte'
  import AgentSetupGuide from './AgentSetupGuide.svelte'
  import AgentIcon from './AgentIcon.svelte'
  import { agentSetupGuidance } from './agentOnboarding'
  import { AgentDraftCache, redactAgentMessage } from './agentConfigForm'

  export let transport: ApplicationTransport
  export let autoDetect = false
  export let onReady: ((config: AgentConfig | null) => void) | undefined = undefined
  export let initialConfigId: string | undefined = undefined
  export let initialAdvanced = false
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
  let advancedOpen = initialAdvanced
  let lastReady: string | null | undefined = undefined
  let manualPaths: Record<string, string> = {}
  $: items = agentListItems($catalog.entries, $catalog.configs)
  $: item = selected === 'new' ? undefined : items.find(row => row.key === selected) ?? items[0]
  $: entry = item?.entry
  $: profile = item?.configs.find(config => config.id === selectedProfile) ?? item?.config
  $: inspection = entry ? $catalog.inspections[entry.id] : undefined
  $: status = item ? agentStatus({ ...item, config: profile }, $catalog) : 'attention'
  $: checked = agentConnectionResult(profile, $catalog)
  $: diagnosis = item ? agentDiagnosis({ ...item, config: profile }, $catalog) : undefined
  $: nativeFound = inspection?.dependencies.some(dependency => dependency.path && dependency.command === (profile?.host_id ?? entry?.host_id)) ?? false
  $: setupGuide = agentSetupGuidance({ catalogId: entry?.id, hostId: profile?.host_id ?? entry?.host_id, config: profile, inspection }).guide
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
  $: showConnect = diagnosis?.canInstall && (['prepare', 'missing'].includes(diagnosis.connection) || (diagnosis.connection === 'failed' && (!inspection?.command || diagnosis.reason === 'install' || diagnosis.reason === 'dependency')))
  $: canUseDetected = !!inspection?.command && !inspection.checks.some(check => check.status === 'fail') && entry?.verification.status !== 'unsupported'
  $: canCheck = !!profile || (canUseDetected && diagnosis?.reason !== 'managed_setup')
  $: safeError = redactAgentMessage(localError || $catalog.error, environmentText())
  $: if ((item?.key ?? 'new') !== selectedBefore) {
    selectedBefore = item?.key ?? 'new'; selectedProfile = ''; localError = ''; notice = ''; advancedOpen = selectingInitial && initialAdvanced
  }
  onMount(() => {
    const dispose = catalog.start()
    let mounted = true
    if (selectingInitial) void catalog.refresh().then(() => {
      if (!mounted || !selectingInitial) return
      const row = agentListItems($catalog.entries, $catalog.configs).find(row => row.configs.some(config => config.id === initialSelection))
      if (row) { selected = row.key; selectedBefore = row.key; selectedProfile = initialSelection!; advancedOpen = initialAdvanced }
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
  function connectionText(value: string) {
    return ({ unchecked: tr('未检测', 'Not checked'), missing: tr('未发现程序', 'Program not found'), prepare: tr('需要连接组件', 'Connection component needed'), checking: tr('正在检测', 'Checking'), connected: tr('已连接', 'Connected'), failed: tr('连接失败', 'Connection failed') } as Record<string, string>)[value] ?? tr('未检测', 'Not checked')
  }
  function connectionExplanation(diagnosis: AgentDiagnosis | undefined, installing: boolean, nativeFound: boolean, name: string, inspection: AgentInspection | undefined, language: string) {
    const tr = (zh: string, en: string) => language === 'zh-CN' ? zh : en
    if (installing) return tr('RambleDesk 正在安装连接组件，完成后会自动检查 ACP 连接。', 'RambleDesk is installing the connection component and will check the ACP connection when it finishes.')
    if (diagnosis?.reason === 'authentication') return tr('ACP 程序要求先完成认证。请按当前连接的指引处理后重试。', 'The ACP program requires authentication. Follow the guidance for this connection, then retry.')
    if (diagnosis?.reason === 'feedback') return tr('ACP 已接通，但智能体未提供 RambleDesk 所需的反馈能力。请查看高级设置中的检测详情。', 'ACP is connected, but the agent did not provide the feedback capability RambleDesk requires. Review the detection details in advanced settings.')
    if (diagnosis?.connection === 'connected') return tr('ACP 连接检查通过。模型访问与 Ramble 反馈交接需在实际会话中验证。', 'ACP connection check passed. Model access and Ramble handoff still need verification in an actual session.')
    if (diagnosis?.connection === 'checking') return tr('正在查找本机程序并检查 ACP 连接。', 'Finding the local program and checking its ACP connection.')
    switch (diagnosis?.reason) {
      case 'managed_setup': return tr('已发现 DeepSeek 连接组件。推荐由 RambleDesk 安装固定版本，无需寻找 dsh 或启动 dsh web；也可在高级设置中使用已发现的入口。', 'A DeepSeek connection component was found. Use the RambleDesk-managed pinned version without locating dsh or starting dsh web, or choose the discovered entry in advanced settings.')
      case 'bridge_missing': return nativeFound
        ? tr('已找到 ' + name + '。RambleDesk 会安装 ACP 连接组件，完成后自动检查连接。', 'Found ' + name + '. RambleDesk will install its ACP connection component, then check the connection automatically.')
        : tr('需要 ACP 连接组件才能接入。RambleDesk 可以安装组件并检查连接。', 'An ACP connection component is needed. RambleDesk can install it and check the connection.')
      case 'runtime': {
        const names = inspection?.checks.filter(check => ['node', 'npm'].includes(check.id) && check.status === 'fail').map(check => check.id === 'node' ? 'Node.js' : 'npm').join(' / ') || 'Node.js / npm'
        return tr('连接组件需要的 ' + names + ' 暂不可用。请安装或更新后重新检测。', names + ' is unavailable for the connection component. Install or update it, then detect again.')
      }
      case 'agent_missing': return tr('未自动发现可用的 ACP 启动入口，不代表本机一定未安装。已有程序或 npx 启动方式可在高级设置中指定命令与参数。', 'No usable ACP launch entry was discovered; this does not prove the agent is uninstalled. Configure an existing program or npx command and arguments in advanced settings.')
      case 'dependency': return diagnosis?.canInstall
        ? tr('连接所需的配套程序尚未安装。RambleDesk 会一起安装并检查连接。', 'A required companion program is missing. RambleDesk will install it and check the connection.')
        : tr('缺少连接所需的配套程序。请按安装说明补齐，高级设置中可查看缺失项。', 'A required companion program is missing. Follow the installation guide; advanced settings show what is missing.')
      case 'launch': return tr('无法启动 ACP 程序。请在高级设置中检查程序位置与启动设置。', 'The ACP program could not start. Check its location and launch settings in advanced settings.')
      case 'install': return tr('连接组件安装失败。请查看高级设置中的具体原因，处理后重试连接。', 'The connection component could not be installed. Review the reason in advanced settings, then retry connecting.')
      case 'connection': return tr('ACP 连接未成功。请查看高级设置中的具体原因，检查连接组件或启动设置后重试。', 'The ACP connection failed. Review the reason in advanced settings, check the component or launch settings, then retry.')
      default: return tr('检测此智能体，确认它能否连接到 RambleDesk。', 'Detect this agent to check whether it can connect to RambleDesk.')
    }
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
    <div><h3 class="m-0 text-sm font-semibold">{tr('智能体', 'Agents')}</h3><p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('选择支持的智能体，检测或准备与 RambleDesk 的连接。', 'Choose a supported agent, then detect or prepare its connection to RambleDesk.')}</p></div>
    <Button variant="outline" size="sm" disabled={$catalog.loading || $catalog.checking.length > 0 || $catalog.connecting.length > 0 || saving} onclick={() => void catalog.detectAll()}><RefreshCw class={`size-3.5 ${$catalog.checking.length > 0 || $catalog.connecting.length > 0 ? 'animate-spin' : ''}`} />{tr('检测智能体', 'Detect agents')}</Button>
  </div>
  {#if safeError}<p role="alert" class="break-words rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-xs">{safeError}</p>{/if}
  <div class="grid min-w-0 gap-3 @min-[680px]:grid-cols-[210px_minmax(0,1fr)]">
    <nav class="overflow-hidden rounded-xl border bg-card" aria-label={tr('智能体列表', 'Agent list')}>
      <div class="border-b px-3 py-3 text-[11px] font-medium text-muted-foreground">{tr('可连接的智能体', 'Available agents')} · {items.length}</div>
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
        <div class="flex items-start gap-3"><div class="flex size-11 shrink-0 items-center justify-center rounded-xl border bg-muted/30"><AgentIcon hostId={profile?.host_id ?? entry?.host_id} class="size-6" /></div><div class="min-w-0 flex-1"><h4 class="m-0 text-base font-semibold">{item.name}</h4><p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{tr('管理与 RambleDesk 的连接。', 'Manage its connection to RambleDesk.')}</p></div></div>
        <section class="space-y-4 rounded-lg border bg-muted/15 p-4" aria-label={tr('检测状态', 'Detection status')} aria-live="polite" data-agent-detection-card>
          <h5 class="m-0 text-xs font-semibold">{tr('检测状态', 'Detection status')}</h5>
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-3 text-xs"><span class="font-medium">{tr('ACP 连接', 'ACP connection')}</span><span class="flex items-center gap-1.5" class:text-emerald-600={diagnosis?.connection === 'connected'} class:text-muted-foreground={diagnosis?.connection !== 'connected'}>{#if installing || diagnosis?.connection === 'checking'}<LoaderCircle class="size-3.5 animate-spin" />{:else if diagnosis?.connection === 'connected'}<CheckCircle2 class="size-3.5" />{/if}{installing ? tr('正在连接', 'Connecting') : connectionText(diagnosis?.connection ?? 'unchecked')}</span></div>
            <p class="m-0 text-xs leading-5 text-muted-foreground">{connectionExplanation(diagnosis, installing, nativeFound, item.name, inspection, $locale)}</p>
            {#if diagnosis?.reason === 'authentication'}<AgentSetupGuide catalogId={entry?.id} hostId={profile?.host_id ?? entry?.host_id} name={item.name} config={profile} {inspection} purpose="authentication" onConfigure={() => advancedOpen = true} compact />{/if}
          </div>
          <div class="flex flex-wrap gap-2">
            {#if showConnect}<Button size="sm" disabled={saving || installing || checking} onclick={() => entry && void catalog.connect(entry.id, profile?.id)}><Download class="size-3.5" />{tr('一键连接', 'Connect in one click')}</Button>{/if}
            <Button variant="outline" size="sm" disabled={saving || checking || installing} onclick={() => void retry()}><RefreshCw class="size-3.5" />{tr(canCheck ? '检查连接' : '重新检测', canCheck ? 'Check connection' : 'Detect again')}</Button>
            {#if diagnosis?.reason === 'runtime'}<a class="inline-flex items-center gap-1 px-1 text-xs underline underline-offset-4" href="https://nodejs.org/en/download" target="_blank" rel="noreferrer">{tr('安装 Node.js', 'Install Node.js')}<ExternalLink class="size-3" /></a>
            {:else if setupGuide && (diagnosis?.reason === 'agent_missing' || diagnosis?.reason === 'dependency') && !diagnosis.canInstall}<a class="inline-flex items-center gap-1 px-1 text-xs underline underline-offset-4" href={setupGuide} target="_blank" rel="noreferrer">{tr('安装说明', 'Installation guide')}<ExternalLink class="size-3" /></a>{/if}
            {#if diagnosis?.connection === 'failed'}<Button variant="ghost" size="sm" onclick={() => advancedOpen = true}>{tr('查看原因', 'View reason')}</Button>{/if}
            {#if installing && job}<Button variant="ghost" size="sm" disabled={job.cancel_requested} onclick={() => job && void catalog.cancel(job.id)}>{tr(job.cancel_requested ? '正在取消…' : '取消', job.cancel_requested ? 'Cancelling…' : 'Cancel')}</Button>{/if}
          </div>
          {#if notice}<p role="status" class="m-0 text-xs leading-5">{notice}</p>{/if}
        </section>
        {#key item.key}<details bind:open={advancedOpen} class="border-t pt-3" data-agent-advanced><summary class="flex cursor-pointer items-center gap-2 text-xs text-muted-foreground"><Settings2 class="size-3.5" />{tr('高级设置', 'Advanced settings')}</summary><div class="mt-4 space-y-4">
          {#if entry}
            <form class="space-y-3 rounded-lg border p-3" onsubmit={(event) => { event.preventDefault(); void usePath() }}>
              <h5 class="m-0 text-xs font-medium">{tr('指定程序位置', 'Specify program location')}</h5>
              <p class="m-0 text-xs leading-5 text-muted-foreground">{entry.connection_kind === 'bridge' ? tr('填写 ACP 连接组件的可执行文件位置；智能体的交互命令不能替代连接组件。', 'Enter the ACP connection component executable location; the agent’s interactive command cannot replace the component.') : tr('填写 CLI 可执行文件位置。RambleDesk 会添加 ACP 启动参数。', 'Enter the CLI executable location. RambleDesk adds its ACP launch arguments.')}</p>
              <label class="block space-y-1.5 text-xs"><span>{tr('可执行文件完整路径', 'Full executable path')}</span><input required bind:value={manualPaths[entry.id]} autocomplete="off" spellcheck="false" class="h-9 w-full rounded-md border bg-background px-3 font-mono" /></label>
              <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('JavaScript 脚本需要在下方启动设置中填写运行时与脚本参数。', 'For a JavaScript script, configure its runtime and script arguments in the launch settings below.')}</p>
              <Button type="submit" size="sm" disabled={saving || checking || !manualPaths[entry.id]?.trim()}>{tr('保存并检查连接', 'Save and check connection')}</Button>
            </form>
          {/if}
          {#if entry}<div class="flex flex-wrap gap-2 text-[10px]"><span class="rounded-full border px-2 py-1">{entry.connection_kind === 'bridge' ? tr('ACP 连接组件', 'ACP connection component') : tr('原生 ACP', 'Native ACP')}</span><span class="rounded-full border px-2 py-1">{tr('推荐版本', 'Recommended')} {entry.distribution.kind === 'npm' ? entry.distribution.pinned_version : entry.distribution.version}</span>{#if inspection?.version}<span class="rounded-full border px-2 py-1">{tr('发现的版本', 'Discovered version')} {inspection.version}</span>{/if}</div>{/if}
          {#if profile}<div class="space-y-1 text-xs"><p class="m-0 font-medium">{tr('实际启动入口', 'Actual launch entry')}</p><code class="block break-all rounded bg-muted p-2 text-[11px]">{redactAgentMessage([profile.command, ...profile.args].join(' '), environmentText())}</code></div>{/if}
          {#if inspection}<div class="space-y-2"><h5 class="m-0 text-xs font-medium">{tr('自动发现结果', 'Discovery details')}</h5>{#each inspection.checks as check}<div class="flex items-start gap-2 text-xs leading-5">{#if check.status === 'pass'}<CheckCircle2 class="mt-0.5 size-3.5 shrink-0 text-emerald-600" />{:else}<XCircle class="mt-0.5 size-3.5 shrink-0 text-amber-600" />{/if}<span class="break-words">{redactAgentMessage(check.message, environmentText())}</span></div>{/each}</div>{/if}
          {#if checked}<div class="space-y-2"><h5 class="m-0 text-xs font-medium">{tr('最近一次检查', 'Latest check')}</h5><p class="m-0 break-words text-xs leading-5 text-muted-foreground">{redactAgentMessage(checked.message, environmentText())}</p></div>{/if}
          {#if checked?.details.length}<div class="space-y-2"><h5 class="m-0 text-xs font-medium">{tr('连接检查详情', 'Connection check details')}</h5><ul class="m-0 list-disc space-y-1 pl-4 text-xs leading-5 text-muted-foreground">{#each checked.details as detail}<li class="break-words">{redactAgentMessage(detail, environmentText())}</li>{/each}</ul></div>{/if}
          {#if canPrepare}<div class="space-y-2"><Button variant="outline" size="sm" disabled={saving || installing || checking} onclick={() => entry && void catalog.install(entry.id)}><Download class="size-3.5" />{tr('修复连接组件', 'Repair connection component')}</Button><p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('安装推荐版本的 ACP 连接组件。已有的自定义启动配置会保留。', 'Install the recommended ACP connection component. Existing custom launch configurations are retained.')}</p></div>{/if}
          {#if job}<div class="space-y-2 rounded-lg border bg-muted/20 p-3"><p class="m-0 text-xs">{tr('连接准备日志', 'Connection setup log')}: {job.phase}</p><pre class="m-0 max-h-36 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-muted-foreground">{redactAgentMessage(job.messages.join('\n'), environmentText())}</pre></div>{/if}
          {#if item.configs.length > 1}<div class="space-y-2"><p class="m-0 text-xs font-medium">{tr('已有启动配置', 'Saved launch configurations')}</p><div class="flex flex-wrap gap-2">{#each item.configs as config}<Button size="sm" variant={config.id === profile?.id ? 'secondary' : 'outline'} disabled={saving} onclick={() => selectedProfile = config.id}>{config.name}</Button>{/each}</div></div>{/if}
          {#if profile}{#key profile.id}<AgentSettings {cache} {baselines} configs={[profile]} busy={saving} onSave={saveProfile} onDelete={removeProfile} onCheck={catalog.check} />{/key}
          {:else if canUseDetected}<Button variant="outline" size="sm" disabled={saving} onclick={() => void prepareAdvanced()}>{tr('使用已发现的入口并编辑', 'Use and edit discovered entry')}</Button>
          {:else}<Button variant="outline" size="sm" disabled={saving} onclick={() => selected = 'new'}>{tr('自定义 ACP 启动设置', 'Custom ACP launch settings')}</Button>{/if}
        </div></details>{/key}
      {/if}
    </div>
  </div>
</section>
