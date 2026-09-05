<script lang="ts">
  import { Check, Folder, FolderOpen, GitBranch, LoaderCircle, MessageSquare, RefreshCw, Settings } from '@lucide/svelte'
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import { locale } from '$lib/preferences'
  import AgentComposer from './composer/AgentComposer.svelte'
  import SessionConfigurationControls from './configuration/SessionConfigurationControls.svelte'
  import SessionContextUsage from './SessionContextUsage.svelte'
  import { attachmentAccept, canAttachFiles } from './attachments/promptAttachments'
  import { attachmentText } from './attachments/attachmentText'
  import { isAbsoluteAgentDirectory } from './agentConfigForm'
  import { agentText } from './agentI18n'
  import type { DraftManagedSessionController } from './draftManagedSessionController'
  import { createManagedWorkspaceInfoController } from './managedWorkspaceInfoController'

  export let transport: ApplicationTransport
  export let controller: DraftManagedSessionController
  export let draftId: string
  export let onConfigure: () => void
  export let onChooseDirectory: (() => Promise<string | null>) | undefined = undefined
  let fileInput: HTMLInputElement | undefined
  let localError = ''
  let choosingDirectory = false
  let mounted = false
  const workspaceInfo = createManagedWorkspaceInfoController(transport)
  $: workspaceInfo.setSessionId($controller.snapshot?.session.session_id ?? null)
  $: locked = $controller.awaitingAcknowledgement || $controller.phase === 'closing' || $controller.phase === 'promoted'
  $: promptCapabilities = $controller.snapshot?.runtime.capabilities.prompt
  $: acceptsFiles = promptCapabilities ? canAttachFiles(promptCapabilities) : false
  $: directoryValid = !$controller.cwd || isAbsoluteAgentDirectory($controller.cwd.trim())

  const zh: Record<string, string> = {
    'New agent session': '新建 Agent 会话', 'What would you like to work on?': '准备开始什么任务？',
    'Choose an agent and a project, then describe your task.': '选择智能体和项目目录，然后描述你的任务。',
    'Manage agents': '管理智能体', 'Ready to send': '可以发送', 'Connecting…': '正在连接…',
    'Sending your first message…': '正在发送第一条消息…', 'Closing the draft…': '正在关闭草稿…',
    'Choose an agent and directory to connect.': '选择智能体和目录后即可连接。',
    'Connecting will load the agent’s session options.': '连接后会显示智能体提供的会话选项。',
    'Your session appears in the sidebar after the first message.': '发送第一条消息后，会话会出现在侧栏。',
    'No available agents. Add one in Agents settings.': '没有可用的智能体，请前往智能体设置添加。',
    'Loading agents…': '正在读取智能体…', 'Selected agent is unavailable': '所选智能体不可用',
    'Refresh agents': '刷新智能体', 'Retry connection': '重试连接',
    'Enter an absolute project directory.': '请输入项目目录的绝对路径。',
    'Could not confirm whether the first message was accepted. Retry to check the session.': '暂时无法确认第一条消息是否已接纳。请重试以检查会话。',
  }
  function tr(text: string) { return $locale === 'zh-CN' ? zh[text] ?? attachmentText($locale, agentText($locale, text)) : text }

  onMount(() => {
    mounted = true
    controller.start()
    const stopWorkspaceInfo = workspaceInfo.start()
    return () => { mounted = false; stopWorkspaceInfo() }
  })
  async function chooseDirectory() {
    if (!onChooseDirectory || locked || choosingDirectory) return
    choosingDirectory = true
    const choice = $controller.choice
    const cwd = $controller.cwd
    try {
      const directory = await onChooseDirectory()
      if (mounted && directory && $controller.choice === choice && $controller.cwd === cwd) controller.select(choice, directory)
    } catch { localError = tr('Could not choose the project directory.') }
    finally { choosingDirectory = false }
  }
  async function addFiles(files: readonly File[]) {
    localError = ''
    try { await controller.addFiles(files) }
    catch (cause) { localError = cause instanceof Error ? tr(cause.message) : tr('Could not add attachments.') }
  }
</script>

<section class="flex h-full min-h-0 min-w-0 flex-col bg-background" aria-label={tr('New agent session')}>
  <header class="flex items-center justify-between gap-3 border-b px-5 py-3">
    <h2 class="m-0 text-sm font-medium">{tr('New agent session')}</h2>
    <Button variant="ghost" size="sm" onclick={onConfigure}><Settings class="size-3.5" />{tr('Manage agents')}</Button>
  </header>
  <div class="flex min-h-0 flex-1 flex-col overflow-y-auto px-5 py-6">
    <div class="mx-auto my-auto w-full max-w-4xl space-y-5 py-4">
      <div class="pb-2 text-center">
        <MessageSquare class="mx-auto mb-4 size-7 text-muted-foreground/50" />
        <h3 class="m-0 text-lg font-medium">{tr('What would you like to work on?')}</h3>
        <p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{tr('Choose an agent and a project, then describe your task.')}</p>
      </div>
      <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
        <label class="min-w-0 space-y-1.5 text-xs">
          <span class="font-medium">{tr('Agent')}</span>
          <select value={$controller.choice} disabled={locked || $controller.loadingChoices} class="h-9 w-full min-w-0 truncate rounded-md border bg-background px-2 disabled:opacity-50"
            onchange={(event) => controller.select(event.currentTarget.value, $controller.cwd)}>
            <option value="" disabled>{tr($controller.loadingChoices ? 'Loading agents…' : 'Choose an agent')}</option>
            {#if $controller.choice && !$controller.choices.some((choice) => choice.key === $controller.choice)}<option value={$controller.choice} disabled>{tr('Selected agent is unavailable')}</option>{/if}
            {#each $controller.choices as choice (choice.key)}<option value={choice.key}>{choice.name}</option>{/each}
          </select>
        </label>
        <div class="min-w-0 space-y-1.5 text-xs">
          <label for={`draft-cwd-${draftId}`} class="font-medium">{tr('Project directory')}</label>
          <div class="flex gap-2">
            <input id={`draft-cwd-${draftId}`} value={$controller.cwd} disabled={locked} aria-invalid={!directoryValid} autocomplete="off" spellcheck="false"
              placeholder="/path/to/project" class="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 font-mono outline-none focus:ring-2 focus:ring-ring disabled:opacity-50"
              oninput={(event) => controller.select($controller.choice, event.currentTarget.value, 500)} />
            {#if onChooseDirectory}<Button variant="outline" size="icon" class="size-9" disabled={locked || choosingDirectory} title={tr('Browse…')} aria-label={tr('Browse…')} onclick={() => void chooseDirectory()}><FolderOpen class="size-3.5" /></Button>{/if}
          </div>
        </div>
      </div>
      {#if !directoryValid}<p role="alert" class="m-0 text-xs text-destructive">{tr('Enter an absolute project directory.')}</p>{/if}
      {#if !$controller.loadingChoices && $controller.choices.length === 0}<p class="m-0 text-xs text-muted-foreground">{tr('No available agents. Add one in Agents settings.')}</p>{/if}
      <div class="flex min-h-5 flex-wrap items-center gap-2 text-[11px] text-muted-foreground" role="status">
        {#if $controller.phase === 'preparing' || $controller.phase === 'sending' || $controller.phase === 'closing'}
          <LoaderCircle class="size-3.5 animate-spin" /><span>{tr($controller.phase === 'sending' ? 'Sending your first message…' : $controller.phase === 'closing' ? 'Closing the draft…' : 'Connecting…')}</span>
        {:else if $controller.phase === 'ready'}<Check class="size-3.5 text-primary" /><span>{tr('Ready to send')}</span>
        {:else if $controller.phase === 'failed'}<Button size="sm" variant="outline" onclick={() => void controller.retry()}><RefreshCw class="size-3" />{tr('Retry connection')}</Button>
        {:else}<span>{tr('Choose an agent and directory to connect.')}</span>{/if}
        <Button class="ml-auto" size="icon-sm" variant="ghost" disabled={locked || $controller.loadingChoices} title={tr('Refresh agents')} aria-label={tr('Refresh agents')} onclick={() => void controller.refreshChoices()}><RefreshCw class="size-3.5" /></Button>
      </div>
      {#if $controller.error || $controller.choicesError || localError}<p role="alert" class="m-0 break-words text-xs text-destructive">{tr(localError || $controller.error || $controller.choicesError)}</p>{/if}
      <input bind:this={fileInput} type="file" class="hidden" multiple tabindex="-1" accept={promptCapabilities ? attachmentAccept(promptCapabilities) : ''} aria-label={tr('Attach files')}
        onchange={(event) => { const files = Array.from(event.currentTarget.files ?? []); event.currentTarget.value = ''; void addFiles(files) }} />
      <div class="space-y-2" data-agent-composer>
      <AgentComposer value={$controller.text} draftKey={draftId} onchange={(text) => controller.edit(text)} onsubmit={(text) => controller.send(text)}
        disabled={$controller.phase === 'closing' || $controller.phase === 'promoted'} busy={$controller.phase === 'sending'} sendDisabled={$controller.phase !== 'ready'}
        attachments={$controller.attachments} onRemoveAttachment={controller.removeAttachment} onAddAttachments={acceptsFiles ? () => fileInput?.click() : undefined}
        onPasteFiles={acceptsFiles ? addFiles : undefined}>
        <svelte:fragment slot="footer">
          {#if $controller.snapshot}<span class="max-w-32 truncate px-1 text-[10px] text-muted-foreground">{$controller.choices.find(choice => choice.key === $controller.choice)?.name ?? ''}</span><SessionConfigurationControls configuration={$controller.snapshot.runtime.configuration} disabled={$controller.phase !== 'ready'} onChange={controller.configure} />
          {:else}<span class="px-1 text-[10px] text-muted-foreground">{tr('Connecting will load the agent’s session options.')}</span>{/if}
        </svelte:fragment>
      </AgentComposer>
      <div class="flex min-w-0 items-center gap-3 px-1 text-[10px] text-muted-foreground" data-workspace-metadata>
        {#if $controller.cwd}<span class="flex min-w-0 items-center gap-1.5" title={$controller.cwd}><Folder class="size-3 shrink-0" /><span class="truncate">{$controller.cwd}</span></span>{/if}
        {#if $workspaceInfo?.branch}<span class="flex min-w-0 max-w-[35%] items-center gap-1.5" title={$workspaceInfo.branch}><GitBranch class="size-3 shrink-0" /><span class="truncate">{$workspaceInfo.branch}</span></span>{/if}
        <span class="flex-1"></span><SessionContextUsage usage={$controller.snapshot?.runtime.context_usage} />
      </div>
      </div>
    </div>
  </div>
</section>
