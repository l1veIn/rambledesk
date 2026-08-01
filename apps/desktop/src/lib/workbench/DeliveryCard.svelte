<script lang="ts">
  import type { FeedbackResultView } from '../feedback'
  import { t } from '../i18n'
  import { desktopPath } from '../nativePath'
  import { locale } from '../preferences'

  export let feedbackResult: FeedbackResultView | null = null
  export let canSubmit = false
  export let submitting = false
  export let onOpenPackage: () => void = () => {}
  export let onSubmit: () => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<section class="delivery-card">
  <div class="delivery-status">
    <span class:ready={canSubmit}></span>
    <div>
      <strong>{feedbackResult ? tr('反馈包已归档') : 'Feedback Package'}</strong>
      <small>{feedbackResult ? tr('Agent 已可读取不可变结果') : tr('正文保存后即可提交')}</small>
    </div>
  </div>
  {#if feedbackResult}
    <code>{desktopPath(feedbackResult.directory_path)}</code>
    <button class="package-button" onclick={onOpenPackage}>{tr('打开 Feedback Package')}</button>
  {:else}
    <button class="primary-button wide-button" disabled={!canSubmit} onclick={onSubmit}>
      {submitting ? tr('正在发布…') : tr('提交反馈')}
    </button>
  {/if}
</section>
