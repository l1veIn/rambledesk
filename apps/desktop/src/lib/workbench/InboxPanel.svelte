<script lang="ts">
  import type { FeedbackRequestSummary } from '../feedback'
  import { requestStatusLabel } from '../feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import type { AdapterPresentation } from './types'

  export let inboxMode: 'open' | 'history'
  export let loadingInbox = false
  export let loadingHistory = false
  export let requests: FeedbackRequestSummary[] = []
  export let activeRequestId: string | null = null
  export let adapterPresentation: (hostId: string) => AdapterPresentation
  export let formatTime: (value: string | null | undefined) => string
  export let onRefresh: () => void = () => {}
  export let onShowOpen: () => void = () => {}
  export let onShowHistory: () => void = () => {}
  export let onOpenRequest: (requestId: string) => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<aside class="inbox-panel">
  <div class="panel-heading">
    <div>
      <p class="eyebrow">{inboxMode === 'open' ? 'INBOX' : 'HISTORY'}</p>
      <h1>{inboxMode === 'open' ? tr('待反馈') : tr('历史记录')}</h1>
    </div>
    <button class="icon-button" aria-label={tr('刷新反馈请求')} onclick={onRefresh}>↻</button>
  </div>

  <div class="inbox-tabs" aria-label={tr('反馈列表范围')}>
    <button class:active={inboxMode === 'open'} onclick={onShowOpen}>{tr('待处理')}</button>
    <button class:active={inboxMode === 'history'} onclick={onShowHistory}>{tr('全部历史')}</button>
  </div>

  {#if (inboxMode === 'open' && loadingInbox) || (inboxMode === 'history' && loadingHistory)}
    <p class="empty-state">{tr('正在读取持久请求…')}</p>
  {:else if requests.length === 0}
    <div class="empty-state">
      <strong>{inboxMode === 'open' ? tr('当前没有待处理请求') : tr('还没有反馈历史')}</strong>
      <span>
        {inboxMode === 'open'
          ? tr('保持工作台开启，Agent 的新请求会出现在这里。')
          : tr('创建过的请求会按最近更新时间显示在这里。')}
      </span>
    </div>
  {:else}
    <nav aria-label={inboxMode === 'open' ? tr('待反馈请求') : tr('反馈历史')}>
      {#each requests as request (request.request_id)}
        <button
          class:active={activeRequestId === request.request_id}
          class="request-card"
          onclick={() => onOpenRequest(request.request_id)}
        >
          <span class="request-meta">
            <b>{request.project_name}</b>
            <em>{requestStatusLabel(request.status, $locale)}</em>
          </span>
          {#if request.title.trim()}<strong>{request.title}</strong>{/if}
          <small class="request-byline">
            <span class="adapter-mark" aria-hidden="true">{@html adapterPresentation(request.agent).icon_svg}</span>
            {adapterPresentation(request.agent).label} · {formatTime(request.updated_at)}
          </small>
        </button>
      {/each}
    </nav>
  {/if}

</aside>
