<script lang="ts">
  import type { FeedbackResultView } from '../feedback'
  import { t } from '../i18n'
  import { desktopPath } from '../nativePath'
  import { locale } from '../preferences'

  export let feedbackResult: FeedbackResultView | null = null
  export let cancelled = false
  export let canSubmit = false
  export let submitting = false
  export let canCancel = false
  export let cancelling = false
  export let onOpenPackage: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: ready = feedbackResult !== null || cancelled || canSubmit
</script>

<section class="delivery-card">
  <div class="delivery-status">
    <span class:ready></span>
    <div>
      <strong>{feedbackResult ? tr('反馈包已归档') : cancelled ? tr('反馈已取消') : 'Feedback Package'}</strong>
      <small>
        {feedbackResult
          ? tr('Agent 已可读取不可变结果')
          : cancelled
            ? tr('Agent 已可读取取消状态')
            : tr('正文保存后即可提交')}
      </small>
    </div>
  </div>
  {#if feedbackResult}
    <code>{desktopPath(feedbackResult.directory_path)}</code>
    <button class="package-button" onclick={onOpenPackage}>{tr('打开 Feedback Package')}</button>
  {:else if cancelled}
    <p class="delivery-note">{tr('请求已取消，Agent 恢复后会读取取消状态。')}</p>
  {:else}
    <div class="delivery-actions">
      <button class="primary-button wide-button" disabled={!canSubmit} onclick={onSubmit}>
        {submitting ? tr('正在发布…') : tr('提交反馈')}
      </button>
      <button class="cancel-button wide-button" disabled={!canCancel} onclick={onCancel}>
        {cancelling ? tr('正在取消…') : tr('取消请求')}
      </button>
    </div>
  {/if}
</section>
