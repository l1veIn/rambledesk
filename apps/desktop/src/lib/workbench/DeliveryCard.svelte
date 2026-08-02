<script lang="ts">
  import { Ban, CheckCircle2, FolderOpen, Send } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import type { FeedbackResultView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { desktopPath } from '$lib/nativePath'
  import { locale } from '$lib/preferences'

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
</script>

<section class="p-4">
  <header class="mb-3 flex items-center gap-2">
    <strong class="text-xs font-medium">Feedback Package</strong>
    {#if feedbackResult}
      <Badge class="ml-auto bg-success text-white">
        <CheckCircle2 class="size-3" />
        {tr('已发布')}
      </Badge>
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
  {:else if cancelled}
    <p class="m-0 text-[10px] leading-4 text-muted-foreground">
      {tr('宿主可以读取取消状态并继续会话。')}
    </p>
  {:else}
    <div class="grid gap-2">
      <Button class="w-full" disabled={!canSubmit} onclick={onSubmit}>
        <Send data-icon="inline-start" />
        {submitting ? tr('正在发布…') : tr('提交反馈')}
      </Button>
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
