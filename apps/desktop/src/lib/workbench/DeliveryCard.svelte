<script lang="ts">
  import { Ban, CheckCircle2, FolderOpen, Send, ThumbsUp } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import type { FeedbackResultView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { desktopPath } from '$lib/nativePath'
  import { cookingEnabled, locale } from '$lib/preferences'
  import type { SubmitStage } from './types'

  export let feedbackResult: FeedbackResultView | null = null
  export let cancelled = false
  export let approved = false
  export let canSubmit = false
  export let submitting = false
  export let submitStage: SubmitStage = 'idle'
  export let canCancel = false
  export let cancelling = false
  export let allowFinish = false
  export let finalSummary = ''
  export let approving = false
  export let onOpenPackage: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<section class="p-4">
  <header class="mb-3 flex items-center gap-2">
    <strong class="text-xs font-medium">Feedback Package</strong>
    {#if feedbackResult}
      <Badge class="ml-auto bg-success text-white">
        <CheckCircle2 class="size-3" />
        {tr('已发布')}
      </Badge>
    {:else if approved}
      <Badge class="ml-auto bg-success text-white"><CheckCircle2 class="size-3" />{tr('已同意结束')}</Badge>
    {:else if cancelled}
      <Badge variant="destructive" class="ml-auto">{tr('已取消')}</Badge>
    {/if}
  </header>

  {#if feedbackResult}
    <p class="m-0 truncate font-mono text-[9px] text-muted-foreground" title={desktopPath(feedbackResult.directory_path)}>
      {desktopPath(feedbackResult.directory_path)}
    </p>
    <Button class="mt-3 w-full" variant="outline" onclick={onOpenPackage}>
      <FolderOpen data-icon="inline-start" />
      {tr('打开反馈包')}
    </Button>
  {:else if approved}
    <p class="m-0 text-[10px] leading-4 text-muted-foreground">
      {tr('用户已认可 Agent 的最终总结，Ramble 流程已经结束。')}
    </p>
  {:else if cancelled}
    <p class="m-0 text-[10px] leading-4 text-muted-foreground">
      {tr('宿主可以读取取消状态并继续会话。')}
    </p>
  {:else}
    {#if allowFinish && finalSummary}
      <div class="mb-3 rounded-md border border-primary/25 bg-primary/5 p-3">
        <strong class="block text-[10px] font-medium">{tr('Agent 最终总结')}</strong>
        <p class="m-0 mt-1 whitespace-pre-wrap text-[10px] leading-4 text-muted-foreground">{finalSummary}</p>
      </div>
    {/if}
    <div class="grid gap-2">
      <Button class="w-full" disabled={!canSubmit} onclick={onSubmit}>
        <Send data-icon="inline-start" />
        {submitting
          ? submitStage === 'cooking'
            ? tr('正在 Cooking…')
            : tr('正在发布…')
          : $cookingEnabled
            ? tr('Cook 并提交')
            : tr('提交反馈')}
      </Button>
      {#if allowFinish}
        <Button class="w-full" variant="secondary" disabled={approving || submitting || cancelling} onclick={onApprove}>
          <ThumbsUp data-icon="inline-start" />
          {approving ? tr('正在结束…') : tr('同意并结束')}
        </Button>
      {/if}
      <Button
        class="w-full"
        variant="destructive"
        disabled={!canCancel}
        onclick={onCancel}
      >
        <Ban data-icon="inline-start" />
        {cancelling ? tr('正在取消…') : tr('取消请求')}
      </Button>
    </div>
  {/if}
</section>
