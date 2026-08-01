<script lang="ts">
  import { t } from '../i18n'
  import { locale } from '../preferences'
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
</script>

<section
  class:active={rambleEngaged}
  class:error={ramblePhase === 'error'}
  class="ramble-console"
>
  <div class="rail-heading">
    <div>
      <p class="eyebrow">RAMBLE</p>
      <strong>{rambleActive ? tr('正在记录') : rambleEngaged ? tr('Ramble 已暂停') : tr('记录待命')}</strong>
    </div>
    <span class="ramble-led"></span>
  </div>

  <button
    class:recording={rambleActive}
    class="ramble-primary"
    disabled={rambleBusy || readOnly}
    onclick={onToggle}
    title={tr('全局快捷键 Ctrl + Shift + R')}
  >
    <span>{rambleActive ? 'Ⅱ' : '●'}</span>
    {#if ramblePhase === 'starting'}
      {tr('正在启动…')}
    {:else if ramblePhase === 'stopping'}
      {tr('正在暂停…')}
    {:else if rambleActive}
      {tr('暂停 Ramble')}
    {:else if rambleStartedOnce}
      {tr('继续 Ramble')}
    {:else}
      {tr('开始 Ramble')}
    {/if}
  </button>

  {#if rambleEngaged}
    <button class="ramble-exit" disabled={rambleBusy} onclick={onExit}>
      {tr('退出 Ramble 操作台')}
    </button>
  {/if}

  <div class="voice-status">
    <div class="voice-title">
      <span class="voice-dot"></span>
      <strong>{voiceDevice || tr('默认麦克风')}</strong>
      {#if voiceChunkIndex > 0}<span>{tr('{count} 段', { count: voiceChunkIndex })}</span>{/if}
    </div>
    <span>{message || tr('开始后可离开窗口继续操作，录音会实时写入正文。')}</span>
    {#if voicePartial}
      <em class="voice-partial">{tr('正在听：{text}', { text: voicePartial })}</em>
    {/if}
    <small>{tr('Sherpa X-ASR · 本地流式转写')}</small>
    <div class="voice-meter" aria-label={tr('麦克风音量')}>
      <span style={`width: ${voiceLevel * 100}%`}></span>
    </div>
  </div>
</section>
