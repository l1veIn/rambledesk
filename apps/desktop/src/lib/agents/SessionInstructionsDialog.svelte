<script lang="ts">
  import { Check, Copy, FileText, LoaderCircle } from '@lucide/svelte'
  import { onDestroy } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import type { SessionConnectionState } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'

  export let instructions: string | undefined = undefined
  export let connection: SessionConnectionState

  let open = false
  let title: HTMLElement | null = null
  let copyState: 'idle' | 'copying' | 'copied' | 'failed' = 'idle'
  let generation = 0
  let displayedInstructions = instructions
  $: if (instructions !== displayedInstructions) resetInstructions(instructions)
  const zh: Record<string, string> = {
    'Built-in session instructions': '内置会话指令',
    'RambleDesk attaches these instructions to each message sent to the agent. They are read-only and do not appear as messages in the conversation history.': 'RambleDesk 会随每轮消息向 Agent 附加这些指令。此处只读，指令不会作为独立消息出现在会话历史中。',
    'Current connection': '当前连接',
    'Most recent connection': '最近一次连接',
    'These are the instructions from the most recent connection, not a record of every past turn. Reconnecting may update them.': '这里显示最近一次连接使用的指令，不代表所有历史回合；重新连接后可能更新。',
    'No instructions are available for this session yet. Connect the agent to read the instructions supplied by the application.': '当前尚无可查看的内置指令。连接 Agent 后，可查看应用提供的实际指令。',
    'This connection did not provide built-in instructions.': '当前连接未提供可查看的内置指令。',
    'Instruction text': '指令原文',
    'Copy instructions': '复制指令',
    'Copying…': '正在复制…',
    'Copied': '已复制',
    'Could not copy. Select the text and copy it manually.': '复制失败，请选中文字手动复制。',
    'Close': '关闭',
  }
  const tr = (text: string) => $locale === 'zh-CN' ? zh[text] ?? text : text

  function changeOpen(next: boolean) {
    open = next
    copyState = 'idle'
    generation += 1
  }

  function resetInstructions(next: string | undefined) {
    displayedInstructions = next
    copyState = 'idle'
    generation += 1
  }

  async function copy() {
    if (!instructions || copyState === 'copying') return
    const text = instructions
    const attempt = generation
    copyState = 'copying'
    try {
      await navigator.clipboard.writeText(text)
      if (open && generation === attempt && instructions === text) copyState = 'copied'
    } catch {
      if (open && generation === attempt && instructions === text) copyState = 'failed'
    }
  }

  onDestroy(() => { generation += 1 })
</script>

<Dialog.Root {open} onOpenChange={changeOpen}>
  <Dialog.Trigger>
    {#snippet child({ props })}
      <Button {...props} variant="ghost" size="sm" class="h-7 shrink-0 gap-1.5 px-2 text-xs" title={tr('Built-in session instructions')} aria-label={tr('Built-in session instructions')}>
        <FileText class="size-3.5" /><span class="hidden sm:inline">{tr('Built-in session instructions')}</span>
      </Button>
    {/snippet}
  </Dialog.Trigger>
  <Dialog.Content class="flex max-h-[85dvh] w-[calc(100vw-2rem)] max-w-3xl flex-col gap-4 sm:max-w-3xl" showCloseButton={false}
    onOpenAutoFocus={(event) => { event.preventDefault(); title?.focus({ preventScroll: true }) }}>
    <Dialog.Header>
      <Dialog.Title bind:ref={title} tabindex={-1} class="outline-none">{tr('Built-in session instructions')}</Dialog.Title>
      <Dialog.Description class="mt-2 leading-6">{tr('RambleDesk attaches these instructions to each message sent to the agent. They are read-only and do not appear as messages in the conversation history.')}</Dialog.Description>
    </Dialog.Header>
    {#if instructions}
      <div class="flex min-h-0 flex-1 flex-col gap-2">
        <p class="m-0 text-xs font-medium text-muted-foreground">{tr(connection === 'connected' ? 'Current connection' : 'Most recent connection')}</p>
        {#if connection !== 'connected'}<p class="m-0 text-xs leading-5 text-muted-foreground">{tr('These are the instructions from the most recent connection, not a record of every past turn. Reconnecting may update them.')}</p>{/if}
        <textarea readonly aria-label={tr('Instruction text')} value={instructions} spellcheck="false"
          class="min-h-0 h-[48dvh] w-full resize-none rounded-lg border bg-muted/30 p-4 font-mono text-xs leading-6 outline-none focus-visible:ring-2 focus-visible:ring-ring"></textarea>
      </div>
    {:else}
      <p class="m-0 rounded-lg border border-dashed bg-muted/20 p-5 text-sm leading-6 text-muted-foreground" role="status">{tr(connection === 'connected' ? 'This connection did not provide built-in instructions.' : 'No instructions are available for this session yet. Connect the agent to read the instructions supplied by the application.')}</p>
    {/if}
    {#if copyState === 'failed'}<p class="m-0 text-xs text-destructive" role="alert">{tr('Could not copy. Select the text and copy it manually.')}</p>{/if}
    <Dialog.Footer class="gap-2">
      <Button variant="outline" onclick={() => changeOpen(false)}>{tr('Close')}</Button>
      {#if instructions}
        <Button variant="secondary" disabled={copyState === 'copying'} onclick={() => void copy()}>
          {#if copyState === 'copying'}<LoaderCircle data-icon="inline-start" class="animate-spin" />{tr('Copying…')}
          {:else if copyState === 'copied'}<Check data-icon="inline-start" />{tr('Copied')}
          {:else}<Copy data-icon="inline-start" />{tr('Copy instructions')}{/if}
        </Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
