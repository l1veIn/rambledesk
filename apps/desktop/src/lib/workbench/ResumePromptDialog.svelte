<script lang="ts">
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import type { ResumePrompt } from './types'

  export let prompt: ResumePrompt
  export let copyState: 'idle' | 'copied' | 'failed' = 'idle'
  export let onCopy: () => void = () => {}
  export let onDismiss: () => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<div
  class="resume-prompt-backdrop"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onDismiss()
  }}
>
  <div
    class="resume-prompt-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="resume-prompt-title"
  >
    <div class="resume-prompt-header">
      <span class="resume-prompt-kicker">WAKE · GENERIC</span>
      <h2 id="resume-prompt-title">{prompt.title}</h2>
      <p>{prompt.body}</p>
    </div>
    <div class="resume-prompt-meta">
      <span>{tr('宿主')}</span>
      <strong>{prompt.host_label}</strong>
      <span>request_id</span>
      <code>{prompt.request_id}</code>
    </div>
    <label class="resume-prompt-label" for="resume-prompt-text">{tr('恢复提示（复制到宿主对话）')}</label>
    <textarea
      id="resume-prompt-text"
      class="resume-prompt-text"
      readonly
      rows="4"
      value={prompt.resume_prompt}
    ></textarea>
    <div class="resume-prompt-actions">
      <button class="primary-button" onclick={onCopy}>
        {copyState === 'copied'
          ? tr('已复制')
          : copyState === 'failed'
            ? tr('复制失败，请手动选择')
            : tr('复制恢复提示')}
      </button>
      <button class="secondary-button" onclick={onDismiss}>{tr('知道了')}</button>
    </div>
  </div>
</div>
