<script lang="ts">
  import { Check, ChevronDown, Folder, FolderOpen, GitBranch, LoaderCircle, MessageSquare, RefreshCw, Settings } from '@lucide/svelte'
  import { Popover } from 'bits-ui'
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import { locale } from '$lib/preferences'
  import AgentComposer from './composer/AgentComposer.svelte'
  import AgentIcon from './AgentIcon.svelte'
  import AgentFailureNotice from './AgentFailureNotice.svelte'
  import { agentFailureFrom } from './agentFailure'
  import SessionConfigurationControls from './configuration/SessionConfigurationControls.svelte'
  import SessionContextUsage from './SessionContextUsage.svelte'
  import { isAbsoluteAgentDirectory, redactAgentMessage } from './agentConfigForm'
  import { agentText } from './agentI18n'
  import type { DraftManagedSessionController } from './draftManagedSessionController'
  import { createManagedWorkspaceInfoController } from './managedWorkspaceInfoController'

  export let transport: ApplicationTransport
  export let controller: DraftManagedSessionController
  export let draftId: string
  export let onConfigure: () => void
  export let onConfigureAgent: ((configId: string | undefined, advanced?: boolean) => void) | undefined = undefined
  export let onChooseDirectory: (() => Promise<string | null>) | undefined = undefined
  let localError = ''
  let choosingDirectory = false
  let directoryPickerOpen = false
  let agentPickerOpen = false
  let mounted = false
  const workspaceInfo = createManagedWorkspaceInfoController(transport)
  $: workspaceInfo.setSessionId($controller.snapshot?.session.session_id ?? null)
  $: locked = $controller.awaitingAcknowledgement || $controller.phase === 'closing' || $controller.phase === 'promoted'
  $: selectedAgent = $controller.choices.find(choice => choice.key === $controller.choice)
  $: actionFailure = $controller.failure ?? ($controller.error && !$controller.awaitingAcknowledgement ? agentFailureFrom(new Error($controller.error), 'session') : null)
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
    'No agents have passed the connection check. Detect agents or open Agents to set one up.': '暂无通过连接检查的智能体。请检测智能体，或前往智能体页面完成设置。',
    'Loading agents…': '正在读取智能体…', 'Selected agent is unavailable': '所选智能体不可用',
    'Refresh agents': '刷新智能体', 'Retry connection': '重试连接',
    'Launch configuration': '启动配置',
    'Enter an absolute project directory.': '请输入项目目录的绝对路径。',
    'Could not confirm whether the first message was accepted. Retry to check the session.': '暂时无法确认第一条消息是否已接纳。请重试以检查会话。',
    'Choose a project': '选择项目', 'Choose a project folder to start.': '请先选择项目文件夹。',
    'Choose a project directory before connecting.': '请先选择项目目录，再连接智能体。',
    'Project directory is required.': '项目目录为必填项。', 'Use this directory': '使用此目录',
    'Choose an agent': '选择智能体', 'Agent': '智能体', 'Project directory': '项目目录',
    'Browse…': '浏览…', 'Could not choose the project directory.': '无法选择项目目录。',
  }
  function configureCurrentAgent() { if (onConfigureAgent) onConfigureAgent(selectedAgent?.config?.id, true); else onConfigure() }
  function tr(text: string) { return $locale === 'zh-CN' ? zh[text] ?? agentText($locale, text) : text }

  onMount(() => {
    mounted = true
    controller.start()
    const stopWorkspaceInfo = workspaceInfo.start()
    return () => { mounted = false; stopWorkspaceInfo() }
  })
  function refreshOnReturn() {
    if (mounted && document.visibilityState === 'visible') void controller.refreshChoices(false)
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

<section class="appearance-surface flex h-full min-h-0 min-w-0 flex-col bg-background" aria-label={tr('New session')}>
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
                  <Folder class="size-4 shrink-0" />
                  {#if !directoryValid}<span class="size-1.5 shrink-0 rounded-full bg-destructive" aria-hidden="true"></span>{/if}
                  <span class="truncate">{projectName || tr('Choose a project')}</span><ChevronDown class="size-3 shrink-0 text-muted-foreground" />
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
                      {#each $controller.choices as choice (choice.key)}
                          <button type="button" class="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left text-xs hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50" disabled={locked}
                            aria-pressed={choice.key === $controller.choice} onclick={() => { controller.select(choice.key, $controller.cwd); agentPickerOpen = false }}>
                            <AgentIcon hostId={choice.hostId} class="size-4 shrink-0" /><span class="min-w-0 flex-1"><span class="block truncate">{choice.name}</span>
                            </span>
                            {#if choice.key === $controller.choice}<Check class="size-3.5 shrink-0" />{/if}
                          </button>
                      {/each}
                      {#if selectedAgent && selectedAgent.profiles.length > 1}
                        <details class="border-t px-2 py-2">
                          <summary class="cursor-pointer text-[11px] text-muted-foreground">{selectedAgent.name} · {tr('Launch configuration')}</summary>
                          {#each selectedAgent.profiles as profile (profile.id)}
                            <button type="button" class="mt-1 flex w-full items-center justify-between rounded-md px-2 py-2 text-left text-xs hover:bg-muted disabled:opacity-50" disabled={locked}
                              aria-pressed={`config:${profile.id}` === $controller.choice} onclick={() => { controller.select(`config:${profile.id}`, $controller.cwd); agentPickerOpen = false }}>
                              <span class="truncate">{profile.name}</span>{#if `config:${profile.id}` === $controller.choice}<Check class="size-3.5 shrink-0" />{/if}
                            </button>
                          {/each}
                        </details>
                      {/if}
                      {#if $controller.choice && !selectedAgent}<p class="m-0 px-2 py-2 text-xs text-destructive">{tr('Selected agent is unavailable')}</p>{/if}
                      {#if !$controller.loadingChoices && $controller.choices.length === 0}<p class="m-0 px-2 py-2 text-xs leading-5 text-muted-foreground">{tr('No agents have passed the connection check. Detect agents or open Agents to set one up.')}</p>{/if}
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
          {:else if $controller.awaitingAcknowledgement}<Button size="sm" variant="outline" onclick={() => void controller.retry()}><RefreshCw class="size-3" />{tr('Check message acceptance')}</Button>
          {:else if $controller.phase !== 'failed'}<span>{tr('Choose an agent and directory to connect.')}</span>{/if}
          <span class="flex-1"></span><SessionContextUsage usage={$controller.snapshot?.runtime.context_usage} />
        </div>
      </div>
      {#if actionFailure && selectedAgent && !$controller.awaitingAcknowledgement}
        <AgentFailureNotice failure={actionFailure} config={selectedAgent.config} inspection={selectedAgent.inspection} catalogId={selectedAgent.catalogId} hostId={selectedAgent.hostId} name={selectedAgent.name}
          onConfigure={configureCurrentAgent} onRetry={$controller.phase === 'failed' ? controller.retry : undefined} busy={locked || $controller.phase === 'preparing'} />
      {/if}
    </div>
  </div>
</section>

<style>
  .draft-composer :global(.ramble-composer-editor) { min-height: 6rem; }
</style>
