<script lang="ts">
  import { CircleStop, Mic, Pause, Play, X } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { RamblePhase } from './types'

  export let rambleEngaged = false
  export let rambleActive = false
  export let ramblePhase: RamblePhase = 'idle'
  export let rambleBusy = false
  export let rambleStartedOnce = false
  export let readOnly = false
  export let voiceDevice = ''
  export let voiceChunkIndex = 0
  export let voicePartial = ''
  export let voiceLevel = 0
  export let message = ''
  export let onToggle: () => void = () => {}
  export let onExit: () => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function primaryLabel() {
    if (ramblePhase === 'starting') return tr('正在启动…')
    if (ramblePhase === 'stopping') return tr('正在暂停…')
    if (rambleActive) return tr('暂停记录')
    if (rambleStartedOnce) return tr('继续记录')
    return tr('开始记录')
  }
</script>

<section class="border-b p-4">
  <header class="mb-3 flex items-center gap-2">
    <Mic class="size-4 text-muted-foreground" />
    <strong class="text-xs font-medium">Ramble</strong>
    <Badge
      variant={ramblePhase === 'error' ? 'destructive' : rambleActive ? 'default' : 'secondary'}
      class="ml-auto h-5 px-1.5 text-[9px]"
    >
      {rambleActive ? tr('记录中') : rambleEngaged ? tr('已暂停') : tr('待命')}
    </Badge>
  </header>

  <div class="flex gap-2">
    <Button
      class="flex-1"
      variant={rambleActive ? 'secondary' : 'default'}
      disabled={rambleBusy || readOnly}
      onclick={onToggle}
      title={tr('全局快捷键 Ctrl + Shift + R')}
    >
      {#if rambleActive}
        <Pause data-icon="inline-start" />
      {:else if rambleStartedOnce}
        <Play data-icon="inline-start" />
      {:else}
        <CircleStop data-icon="inline-start" />
      {/if}
      {primaryLabel()}
    </Button>
    {#if rambleEngaged}
      <Button
        variant="outline"
        size="icon"
        disabled={rambleBusy}
        onclick={onExit}
        aria-label={tr('退出 Ramble 操作台')}
        title={tr('退出 Ramble 操作台')}
      >
        <X />
      </Button>
    {/if}
  </div>

  <div class="mt-3 text-[10px] leading-4 text-muted-foreground">
    <div class="flex items-center gap-1.5">
      <span class={['size-1.5 rounded-full', rambleActive ? 'bg-destructive' : 'bg-muted-foreground/40']}></span>
      <span class="min-w-0 flex-1 truncate">{voiceDevice || tr('默认麦克风')}</span>
      {#if voiceChunkIndex > 0}
        <span class="tabular-nums">{tr('{count} 段', { count: voiceChunkIndex })}</span>
      {/if}
    </div>
    <p class="m-0 mt-1">{message || tr('录音会在本机转写并写入正文。')}</p>
    {#if voicePartial}
      <p class="m-0 mt-1 truncate text-foreground">
        {tr('正在听：{text}', { text: voicePartial })}
      </p>
    {/if}
    <div class="mt-2 h-1 overflow-hidden rounded-full bg-muted" aria-label={tr('麦克风音量')}>
      <span
        class="block h-full bg-primary transition-[width]"
        style={`width: ${voiceLevel * 100}%`}
      ></span>
    </div>
  </div>
</section>
