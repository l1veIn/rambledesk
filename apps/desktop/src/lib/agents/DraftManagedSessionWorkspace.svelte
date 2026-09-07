<script lang="ts">
  import { Check, ChevronDown, Folder, FolderOpen, GitBranch, LoaderCircle, MessageSquare, RefreshCw, Settings } from '@lucide/svelte'
  import { Popover } from 'bits-ui'
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import { locale } from '$lib/preferences'
  import AgentComposer from './composer/AgentComposer.svelte'
  import AgentIcon from './AgentIcon.svelte'
  import AgentSetupGuide from './AgentSetupGuide.svelte'
  import SessionConfigurationControls from './configuration/SessionConfigurationControls.svelte'
  import SessionContextUsage from './SessionContextUsage.svelte'
  import { isAbsoluteAgentDirectory, redactAgentMessage } from './agentConfigForm'
  import { agentText } from './agentI18n'
  import { agentNeedsPreparation, canPrepareAgentConnection, type DraftManagedSessionController } from './draftManagedSessionController'
  import { createManagedWorkspaceInfoController } from './managedWorkspaceInfoController'

  export let transport: ApplicationTransport
  export let controller: DraftManagedSessionController
  export let draftId: string
  export let onConfigure: () => void
  export let onChooseDirectory: (() => Promise<string | null>) | undefined = undefined
  let localError = ''
  let choosingDirectory = false
  let directoryPickerOpen = false
  let agentPickerOpen = false
  let mounted = false
  const workspaceInfo = createManagedWorkspaceInfoController(transport)
  $: workspaceInfo.setSessionId($controller.snapshot?.session.session_id ?? null)
  $: locked = $controller.awaitingAcknowledgement || $controller.preparingConnection || $controller.phase === 'closing' || $controller.phase === 'promoted'
  $: selectedAgent = $controller.choices.find(choice => choice.key === $controller.choice)
  $: needsPreparation = agentNeedsPreparation(selectedAgent)
  $: canPrepare = canPrepareAgentConnection(selectedAgent)
  $: installationJob = $controller.installationJob?.agent_id === selectedAgent?.catalogId ? $controller.installationJob : null
  $: installationDetails = redactAgentMessage(installationJob?.messages.join('\n') ?? '', Object.entries(selectedAgent?.config?.env ?? {}).map(([key, value]) => `${key}=${value}`).join('\n'))
  $: directoryValid = isAbsoluteAgentDirectory($controller.cwd.trim())
  $: projectName = $controller.cwd.trim().replace(/[\\/]+$/u, '').split(/[\\/]/u).pop() || $controller.cwd.trim()

  const zh: Record<string, string> = {
    'New session': '新建会话', 'What would you like to work on?': '准备开始什么任务？',
    'Choose an agent and a project, then describe your task.': '选择智能体和项目目录，然后描述你的任务。',
    'Manage agents': '管理智能体', 'Ready to send': '可以发送', 'Connecting…': '正在连接…',
    'Sending your first message…': '正在发送第一条消息…', 'Closing the draft…': '正在关闭草稿…',
    'Choose an agent and directory to connect.': '选择智能体和目录后即可连接。',
    'Connecting will load the agent’s session options.': '连接后会显示智能体提供的会话选项。',
    'Your session appears in the sidebar after the first message.': '发送第一条消息后，会话会出现在侧栏。',
    'No saved or previously detected agents. Detect agents or open Agents for setup guidance.': '暂无已保存或已检测到的智能体。可手动检测，或前往智能体页面查看设置指引。',
    'Loading agents…': '正在读取智能体…', 'Selected agent is unavailable': '所选智能体不可用',
    'Refresh agents': '刷新智能体', 'Retry connection': '重试连接',
    'Enter an absolute project directory.': '请输入项目目录的绝对路径。',
    'Could not confirm whether the first message was accepted. Retry to check the session.': '暂时无法确认第一条消息是否已接纳。请重试以检查会话。',
    'Other launch profiles': '其他启动配置', 'Needs connection preparation': '需要准备连接', 'Not found': '未发现',
    'Prepare connection': '准备连接', 'Preparing connection…': '正在准备连接…',
    'RambleDesk will install the connection components for this agent and check the connection.': 'RambleDesk 将安装此智能体所需的连接组件并检查连接。',
    'Complete the installation or setup below, then check again.': '请按照下方指引完成安装或设置，然后重新检测。',
    'Installed elsewhere? Specify its program location in Agents.': '安装在其他位置？可在智能体页面指定程序位置。',
    'Check again': '重新检测', 'Installation details': '安装详情', 'Cancel preparation': '取消准备', 'Cancelling…': '正在取消…',
    'Connection preparation was cancelled.': '已取消连接准备。',
    'Could not prepare the connection. See the installation details and retry.': '无法准备连接，请查看安装详情后重试。',
    'Connection preparation status is unavailable. Check Agents and retry.': '暂时无法读取连接准备状态，请前往智能体页面检查后重试。',
    'Install the required runtime': '安装所需运行环境', 'Needs attention': '需要处理',
    'Choose a project': '选择项目', 'Choose a project folder to start.': '请先选择项目文件夹。',
    'Choose a project directory before connecting.': '请先选择项目目录，再连接智能体。',
    'Project directory is required.': '项目目录为必填项。', 'Use this directory': '使用此目录',
    'Choose an agent': '选择智能体', 'Agent': '智能体', 'Project directory': '项目目录',
    'Browse…': '浏览…', 'Could not choose the project directory.': '无法选择项目目录。',
  }
  function tr(text: string) { return $locale === 'zh-CN' ? zh[text] ?? agentText($locale, text) : text }

  onMount(() => {
    mounted = true
    controller.start()
    const stopWorkspaceInfo = workspaceInfo.start()
    return () => { mounted = false; stopWorkspaceInfo() }
  })
  function refreshOnReturn() {
    if (mounted && document.visibilityState === 'visible' && !$controller.preparingConnection) void controller.refreshChoices(false)
  }
  async function chooseDirectory() {
    if (!onChooseDirectory || locked || choosingDirectory) return
    choosingDirectory = true
    const choice = $controller.choice
    const cwd = $controller.cwd
    try {
      const directory = await onChooseDirectory()
      if (mounted && directory && $controller.choice === choice && $controller.cwd === cwd) {
        controller.select(choice, directory)
        directoryPickerOpen = false
        localError = ''
      }
    } catch { localError = tr('Could not choose the project directory.') }
    finally { choosingDirectory = false }
  }
</script>

<svelte:window onfocus={refreshOnReturn} />

<section class="flex h-full min-h-0 min-w-0 flex-col bg-background" aria-label={tr('New session')}>
  <div class="flex min-h-0 flex-1 flex-col overflow-y-auto px-5 py-8 sm:px-8">
    <div class="mx-auto my-auto w-full max-w-3xl space-y-4 py-8">
      <div class="pb-8 text-center sm:pb-12">
        <MessageSquare class="mx-auto mb-5 size-9 text-muted-foreground/40" />
        <h2 class="m-0 text-xl font-medium tracking-tight sm:text-2xl">{tr('What would you like to work on?')}</h2>
      </div>
      <div class="space-y-2" data-agent-composer>
        <div class="flex min-w-0 items-center gap-3 px-2" data-workspace-metadata>
          <Popover.Root bind:open={directoryPickerOpen}>
            <Popover.Trigger>
              {#snippet child({ props })}
                <Button {...props} variant="ghost" size="sm" class="min-w-0 max-w-[70%] justify-start gap-2 px-2 text-xs" disabled={locked}
                  title={$controller.cwd || tr('Project directory is required.')} aria-label={`${tr('Project directory')}: ${$controller.cwd || tr('Choose a project')}`}>
                  <Folder class="size-4 shrink-0" /><span class="truncate">{projectName || tr('Choose a project')}</span><ChevronDown class="size-3 shrink-0 text-muted-foreground" />
                </Button>
              {/snippet}
            </Popover.Trigger>
            <Popover.Portal>
              <Popover.Content side="top" align="start" sideOffset={8} aria-label={tr('Project directory')}
                class="z-[130] w-96 max-w-[calc(100vw-2rem)] space-y-3 rounded-xl border bg-popover p-4 text-popover-foreground shadow-lg outline-none">
                <label for={`draft-cwd-${draftId}`} class="text-xs font-medium">{tr('Project directory')}</label>
                <input id={`draft-cwd-${draftId}`} value={$controller.cwd} disabled={locked} aria-required="true" aria-invalid={Boolean($controller.cwd) && !directoryValid} autocomplete="off" spellcheck="false"
                  placeholder="/path/to/project" class="h-9 w-full min-w-0 rounded-md border bg-background px-3 font-mono text-xs outline-none focus:ring-2 focus:ring-ring disabled:opacity-50"
                  oninput={(event) => controller.select($controller.choice, event.currentTarget.value, 500)}
                  onkeydown={(event) => { if (event.key === 'Enter' && directoryValid) { event.preventDefault(); directoryPickerOpen = false } }} />
                <p class="m-0 text-xs text-muted-foreground">{tr($controller.cwd && !directoryValid ? 'Enter an absolute project directory.' : 'Project directory is required.')}</p>
                <div class="flex items-center justify-between gap-2">
                  {#if onChooseDirectory}<Button variant="outline" size="sm" disabled={locked || choosingDirectory} onclick={() => void chooseDirectory()}><FolderOpen class="size-3.5" />{tr('Browse…')}</Button>{/if}
                  <Button size="sm" class="ml-auto" disabled={locked || !directoryValid} onclick={() => { directoryPickerOpen = false }}>{tr('Use this directory')}</Button>
                </div>
              </Popover.Content>
            </Popover.Portal>
          </Popover.Root>
          {#if $workspaceInfo?.branch}<span class="flex min-w-0 max-w-[35%] items-center gap-1.5 text-xs text-muted-foreground" title={$workspaceInfo.branch}><GitBranch class="size-3.5 shrink-0" /><span class="truncate">{$workspaceInfo.branch}</span></span>{/if}
        </div>
        <div class="draft-composer">
          <AgentComposer value={$controller.text} draftKey={draftId} onchange={(text) => controller.edit(text)} onsubmit={(text) => controller.send(text)}
            disabled={$controller.phase === 'closing' || $controller.phase === 'promoted'} busy={$controller.phase === 'sending'} sendDisabled={!directoryValid || $controller.phase !== 'ready'}>
            <svelte:fragment slot="footer">
              {#if $controller.snapshot}<SessionConfigurationControls configuration={$controller.snapshot.runtime.configuration} disabled={$controller.phase !== 'ready'} onChange={controller.configure} />{/if}
            </svelte:fragment>
            <svelte:fragment slot="footer-end">
              <Popover.Root bind:open={agentPickerOpen}>
                <Popover.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} variant="ghost" size="sm" class="h-8 max-w-48 shrink gap-1.5 px-2 text-xs" disabled={locked}
                      aria-label={`${tr('Choose an agent')}: ${selectedAgent?.name || tr($controller.choice ? 'Selected agent is unavailable' : 'Choose an agent')}`}>
                      {#if $controller.loadingChoices}<LoaderCircle class="size-3.5 shrink-0 animate-spin" />{:else}<AgentIcon hostId={selectedAgent?.hostId} class="size-3.5 shrink-0" />{/if}
                      <span class="truncate">{selectedAgent?.name || tr($controller.loadingChoices ? 'Loading agents…' : 'Choose an agent')}</span><ChevronDown class="size-3 shrink-0 text-muted-foreground" />
                    </Button>
                  {/snippet}
                </Popover.Trigger>
                <Popover.Portal>
                  <Popover.Content side="top" align="end" sideOffset={8} aria-label={tr('Choose an agent')}
                    class="z-[130] w-72 max-w-[calc(100vw-2rem)] rounded-xl border bg-popover p-1.5 text-popover-foreground shadow-lg outline-none">
                    <p class="m-0 px-2 py-2 text-xs font-medium text-muted-foreground">{tr('Choose an agent')}</p>
                    <div class="max-h-72 space-y-0.5 overflow-y-auto">
                      {#each [false, true] as advanced}
                        {#if advanced && $controller.choices.some(choice => choice.advanced)}<p class="mb-1 mt-2 border-t px-2 pt-2 text-[11px] text-muted-foreground">{tr('Other launch profiles')}</p>{/if}
                        {#each $controller.choices.filter(choice => Boolean(choice.advanced) === advanced) as choice (choice.key)}
                          <button type="button" class="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left text-xs hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50" disabled={locked}
                            aria-pressed={choice.key === $controller.choice} onclick={() => { controller.select(choice.key, $controller.cwd); agentPickerOpen = false }}>
                            <AgentIcon hostId={choice.hostId} class="size-4 shrink-0" /><span class="min-w-0 flex-1"><span class="block truncate">{choice.name}</span>
                            {#if agentNeedsPreparation(choice)}<span class="mt-0.5 block text-[10px] text-muted-foreground">{tr(canPrepareAgentConnection(choice) ? 'Needs connection preparation' : choice.inspection?.source === 'missing' ? 'Not found' : 'Needs attention')}</span>{/if}</span>
                            {#if choice.key === $controller.choice}<Check class="size-3.5 shrink-0" />{/if}
                          </button>
                        {/each}
                      {/each}
                      {#if $controller.choice && !selectedAgent}<p class="m-0 px-2 py-2 text-xs text-destructive">{tr('Selected agent is unavailable')}</p>{/if}
                      {#if !$controller.loadingChoices && $controller.choices.length === 0}<p class="m-0 px-2 py-2 text-xs leading-5 text-muted-foreground">{tr('No saved or previously detected agents. Detect agents or open Agents for setup guidance.')}</p>{/if}
                    </div>
                    <div class="mt-1.5 flex items-center justify-between border-t pt-1.5">
                      <Button size="sm" variant="ghost" disabled={locked || $controller.loadingChoices} onclick={() => void controller.refreshChoices(true)}><RefreshCw class="size-3.5" />{tr('Refresh agents')}</Button>
                      <Button size="sm" variant="ghost" onclick={() => { agentPickerOpen = false; onConfigure() }}><Settings class="size-3.5" />{tr('Manage agents')}</Button>
                    </div>
                  </Popover.Content>
                </Popover.Portal>
              </Popover.Root>
            </svelte:fragment>
          </AgentComposer>
        </div>
        <div class="flex min-h-5 flex-wrap items-center gap-2 px-2 text-[11px] text-muted-foreground" role="status">
          {#if !directoryValid}<span class:text-destructive={Boolean($controller.cwd)}>{tr($controller.cwd ? 'Enter an absolute project directory.' : 'Choose a project folder to start.')}</span>
          {:else if $controller.phase === 'preparing' || $controller.phase === 'sending' || $controller.phase === 'closing'}
            <LoaderCircle class="size-3.5 animate-spin" /><span>{tr($controller.phase === 'sending' ? 'Sending your first message…' : $controller.phase === 'closing' ? 'Closing the draft…' : 'Connecting…')}</span>
          {:else if $controller.phase === 'ready'}<Check class="size-3.5 text-primary" /><span>{tr('Ready to send')}</span>
          {:else if $controller.phase === 'failed'}<Button size="sm" variant="outline" onclick={() => void controller.retry()}><RefreshCw class="size-3" />{tr('Retry connection')}</Button>
          {:else if !needsPreparation}<span>{tr('Choose an agent and directory to connect.')}</span>{/if}
          <span class="flex-1"></span><SessionContextUsage usage={$controller.snapshot?.runtime.context_usage} />
        </div>
      </div>
      {#if selectedAgent && (needsPreparation || $controller.preparingConnection)}
        <section class="space-y-3 rounded-lg border bg-muted/15 p-4" aria-label={tr('Prepare connection')}>
          <p class="m-0 text-xs leading-5">{tr(canPrepare ? 'RambleDesk will install the connection components for this agent and check the connection.' : 'Complete the installation or setup below, then check again.')}</p>
          {#if selectedAgent.inspection?.checks.some(check => ['node', 'npm'].includes(check.id) && check.status === 'fail')}
            <div class="space-y-1 text-xs leading-5">{#each selectedAgent.inspection.checks.filter(check => ['node', 'npm'].includes(check.id) && check.status === 'fail') as check}<p class="m-0 break-words">{check.message}</p>{/each}<a class="inline-block underline underline-offset-2" href="https://nodejs.org/en/download" target="_blank" rel="noreferrer">{tr('Install the required runtime')}</a></div>
          {/if}
          {#if canPrepare || $controller.preparingConnection}<Button size="sm" disabled={locked} onclick={() => void controller.prepareConnection()}>{#if $controller.preparingConnection}<LoaderCircle class="size-3.5 animate-spin" />{/if}{tr($controller.preparingConnection ? 'Preparing connection…' : 'Prepare connection')}</Button>{/if}
          {#if $controller.preparingConnection && installationJob}<Button size="sm" variant="ghost" disabled={installationJob.cancel_requested} onclick={() => void controller.cancelPreparation()}>{tr(installationJob.cancel_requested ? 'Cancelling…' : 'Cancel preparation')}</Button>{/if}
          {#if !$controller.preparingConnection}
            {#if !canPrepare}<AgentSetupGuide catalogId={selectedAgent.catalogId} hostId={selectedAgent.hostId} name={selectedAgent.name} config={selectedAgent.config} inspection={selectedAgent.inspection} compact />{/if}
            <div class="flex flex-wrap items-center gap-2"><Button size="sm" variant="outline" disabled={$controller.loadingChoices} onclick={() => void controller.refreshChoices(true)}><RefreshCw class="size-3" />{tr('Check again')}</Button><Button size="sm" variant="ghost" onclick={onConfigure}>{tr('Manage agents')}</Button></div>
            <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('Installed elsewhere? Specify its program location in Agents.')}</p>
          {/if}
          {#if installationDetails}<details><summary class="cursor-pointer text-xs text-muted-foreground">{tr('Installation details')}</summary><pre class="mt-2 max-h-36 overflow-auto whitespace-pre-wrap break-words text-[11px]">{installationDetails}</pre></details>{/if}
        </section>
      {/if}
      {#if $controller.error || $controller.choicesError || localError}<p role="alert" class="m-0 break-words text-xs text-destructive">{tr(localError || $controller.error || $controller.choicesError)}</p>{/if}
      {#if $controller.phase === 'failed' && selectedAgent && !needsPreparation && !$controller.awaitingAcknowledgement}<AgentSetupGuide catalogId={selectedAgent.catalogId} hostId={selectedAgent.hostId} name={selectedAgent.name} config={selectedAgent.config} inspection={selectedAgent.inspection} compact />{/if}
    </div>
  </div>
</section>

<style>
  .draft-composer :global(.ramble-composer-editor) { min-height: 6rem; }
</style>
