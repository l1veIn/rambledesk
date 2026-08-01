<script lang="ts">
  import { t } from '../i18n'
  import { locale } from '../preferences'

  export let attachmentCount = 0
  export let attachmentBusy = false
  export let rambleEngaged = false
  export let readOnly = false
  export let onScreenCapture: () => void = () => {}
  export let onImportClipboard: () => void = () => {}
  export let onFileSelection: (event: Event) => void = () => {}

  let attachmentInput: HTMLInputElement

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<section class="tool-card">
  <div class="rail-heading">
    <div>
      <p class="eyebrow">CAPTURE</p>
      <strong>{tr('添加上下文')}</strong>
    </div>
    <span>{attachmentCount}</span>
  </div>
  <div class="tool-grid">
    <button
      disabled={!rambleEngaged || attachmentBusy || readOnly}
      onclick={onScreenCapture}
      title="Ctrl + Shift + 1"
    >
      <span class="tool-icon">⌗</span>
      <strong>{tr('截图')}</strong>
      <small>{tr('区域捕获')}</small>
    </button>
    <button
      disabled={!rambleEngaged || attachmentBusy || readOnly}
      onclick={onImportClipboard}
    >
      <span class="tool-icon">▣</span>
      <strong>{tr('剪贴板')}</strong>
      <small>{tr('显式导入')}</small>
    </button>
    <button
      disabled={attachmentBusy || readOnly}
      onclick={() => attachmentInput.click()}
    >
      <span class="tool-icon">＋</span>
      <strong>{tr('文件')}</strong>
      <small>{tr('选择或拖入')}</small>
    </button>
  </div>
  <input
    bind:this={attachmentInput}
    class="visually-hidden"
    type="file"
    multiple
    onchange={onFileSelection}
  />
  <p class="tool-hint">{tr('不会监听剪贴板；只有点击导入时才读取一次当前内容。')}</p>
</section>
