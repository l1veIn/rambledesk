<script lang="ts">
  import {
    AlertCircle,
    Check,
    ChefHat,
    CloudCog,
    Columns2,
    FileText,
    LoaderCircle,
    Sparkles,
    Undo2,
  } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import RichFeedbackEditor from '$lib/RichFeedbackEditor.svelte'
  import type { AttachmentView, FeedbackWorkspaceView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { SavePhase } from './types'

  export let workspace: FeedbackWorkspaceView
  export let draftBody = ''
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let cooking = false
  export let cookingEnabled = false
  export let cookedPreviewActive = false
  export let cookedPreviewModel = ''
  export let locked = false
  export let cookedMarkdown = ''
  export let uncookedMarkdown = ''
  export let formatTime: (value: string | null | undefined) => string
  export let onChange: (markdown: string) => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onRestoreOriginal: () => void = () => {}
  export let onOpenAttachment: (attachmentId: string) => void = () => {}

  let richEditor: RichFeedbackEditor
  let publishedView: 'cooked' | 'uncooked' | 'compare' = 'cooked'

  $: readOnly =
    workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  $: editingDisabled = readOnly || locked
  $: hasPublishedFeedback = readOnly && cookedMarkdown.trim().length > 0
  $: hasCookingDifference =
    hasPublishedFeedback && cookedMarkdown.trim() !== uncookedMarkdown.trim()
  $: displayedMarkdown =
    hasPublishedFeedback && publishedView === 'cooked'
      ? cookedMarkdown
      : hasPublishedFeedback && publishedView === 'uncooked'
        ? uncookedMarkdown
        : draftBody

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function saveLabel() {
    if (savePhase === 'saving') return tr('正在保存…')
    if (savePhase === 'unsaved') return tr('等待自动保存')
    if (savePhase === 'error') return tr('保存失败')
    return `${tr('已保存')} · r${savedRevision}`
  }

  export function insertAttachments(attachments: AttachmentView[]) {
    return richEditor?.insertAttachments(attachments) ?? false
  }

  export function applyExternalMarkdown(markdown: string): boolean {
    return richEditor?.applyExternalMarkdown(markdown) ?? false
  }

  export function appendTranscript(text: string) {
    richEditor?.appendTranscript(text)
  }

  export function appendClipboardCapture(text: string, label: string) {
    return richEditor?.appendClipboardCapture(text, label) ?? false
  }

  export function appendCapturedAttachment(attachment: AttachmentView, label: string) {
    return richEditor?.appendCapturedAttachment(attachment, label) ?? false
  }

  export function removeAttachmentReference(attachmentId: string) {
    richEditor?.removeAttachmentReference(attachmentId)
  }
</script>

<section
  class={[
    'flex h-full min-h-0 flex-1 flex-col p-5 transition-colors',
    dragActive ? 'bg-primary/5 ring-2 ring-inset ring-primary/30' : '',
  ]}
>
  <header class="mb-3 flex items-center gap-3">
    <div class="min-w-0 flex-1">
      <h2 class="m-0 text-xs font-medium">{tr('反馈正文')}</h2>
      <p class="m-0 mt-0.5 text-[10px] text-muted-foreground">
        {readOnly ? tr('此请求已结束，正文只读。') : tr('记录观察、问题和建议。')}
      </p>
    </div>
    {#if hasPublishedFeedback}
      <div class="ml-auto flex items-center gap-1 rounded-md border bg-muted/30 p-0.5">
        <Button
          variant={publishedView === 'cooked' ? 'secondary' : 'ghost'}
          size="sm"
          class={publishedView === 'cooked' ? 'h-7 px-2 text-[10px]' : 'size-7 p-0'}
          aria-label="Cooked"
          title="Cooked"
          onclick={() => (publishedView = 'cooked')}
        >
          <Sparkles data-icon={publishedView === 'cooked' ? 'inline-start' : undefined} />
          {#if publishedView === 'cooked'}Cooked{/if}
        </Button>
        <Button
          variant={publishedView === 'uncooked' ? 'secondary' : 'ghost'}
          size="sm"
          class={publishedView === 'uncooked' ? 'h-7 px-2 text-[10px]' : 'size-7 p-0'}
          aria-label="Uncooked"
          title="Uncooked"
          onclick={() => (publishedView = 'uncooked')}
        >
          <FileText data-icon={publishedView === 'uncooked' ? 'inline-start' : undefined} />
          {#if publishedView === 'uncooked'}Uncooked{/if}
        </Button>
        {#if hasCookingDifference}
          <Button
            variant={publishedView === 'compare' ? 'secondary' : 'ghost'}
            size="sm"
            class={publishedView === 'compare' ? 'h-7 px-2 text-[10px]' : 'size-7 p-0'}
            aria-label={tr('对比')}
            title={tr('对比')}
            onclick={() => (publishedView = 'compare')}
          >
            <Columns2 data-icon={publishedView === 'compare' ? 'inline-start' : undefined} />
            {#if publishedView === 'compare'}{tr('对比')}{/if}
          </Button>
        {/if}
      </div>
    {:else if cookingEnabled && !readOnly && !locked && !cooking}
      <Button
        variant="ghost"
        size="sm"
        class="ml-auto h-7 shrink-0 gap-1 px-2 text-[10px] text-muted-foreground hover:text-foreground"
        onclick={onCookPreview}
      >
        <Sparkles class="size-3.5" />
        {tr('先看 Cook 结果')}
      </Button>
    {/if}
  </header>

  {#if cookedPreviewActive}
    <div
      class="mb-2 flex items-center gap-2 rounded-md border border-primary/20 bg-primary/5 px-3 py-2 text-[10px] text-foreground"
      aria-live="polite"
    >
      <Sparkles class="size-3.5 shrink-0 text-primary" />
      <span class="min-w-0 flex-1 truncate">
        {cookedPreviewModel ? tr('已用 Cooking 整理（{model}），提交将直接使用整理稿。', { model: cookedPreviewModel }) : tr('已用 Cooking 整理，提交将直接使用整理稿。')}
      </span>
      <Button
        variant="outline"
        size="sm"
        class="h-6 shrink-0 gap-1 px-2 text-[10px]"
        onclick={onRestoreOriginal}
      >
        <Undo2 class="size-3" />
        {tr('恢复原文')}
      </Button>
    </div>
  {/if}

  <div class="relative flex min-h-0 flex-1">
    {#if hasPublishedFeedback && publishedView === 'compare' && hasCookingDifference}
      <div class="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <section class="flex min-h-0 flex-col gap-2">
          <strong class="text-[10px] font-medium text-muted-foreground">Uncooked</strong>
          <RichFeedbackEditor
            markdown={uncookedMarkdown}
            previews={attachmentPreviews}
            disabled={true}
            {onOpenAttachment}
          />
        </section>
        <section class="flex min-h-0 flex-col gap-2">
          <strong class="text-[10px] font-medium text-muted-foreground">Cooked</strong>
          <RichFeedbackEditor
            markdown={cookedMarkdown}
            previews={attachmentPreviews}
            disabled={true}
            {onOpenAttachment}
          />
        </section>
      </div>
    {:else}
      <RichFeedbackEditor
        bind:this={richEditor}
        markdown={displayedMarkdown}
        previews={attachmentPreviews}
        disabled={editingDisabled}
        {onOpenAttachment}
        onChange={(markdown) => {
          if (!editingDisabled) onChange(markdown)
        }}
      />
    {/if}

    {#if cooking}
      <div
        class="absolute inset-0 z-10 grid place-items-center rounded-md border bg-background/85 p-6 text-center backdrop-blur-sm"
        aria-live="assertive"
        aria-busy="true"
      >
        <div class="max-w-xs">
          <span class="mx-auto grid size-12 place-items-center rounded-full bg-primary/10 text-primary">
            <ChefHat class="size-6 animate-pulse" />
          </span>
          <strong class="mt-3 block text-sm font-medium">{tr('Cooking 中…')}</strong>
          <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
            {tr('正在整理原始反馈并保留 uncooked 原稿，请稍候。')}
          </p>
          <LoaderCircle class="mx-auto mt-3 size-4 animate-spin text-primary" />
        </div>
      </div>
    {/if}
  </div>

  <footer class="mt-2 flex items-center gap-3 text-[9px] text-muted-foreground">
    <span>{tr('{count} 字符', { count: draftBody.length.toLocaleString($locale) })}</span>
    <span>Markdown</span>
    <Badge
      variant={savePhase === 'error' ? 'destructive' : 'secondary'}
      class="ml-auto h-6 gap-1 px-2 text-[9px]"
      aria-live="polite"
    >
      {#if savePhase === 'saving'}
        <LoaderCircle class="size-3 animate-spin" />
      {:else if savePhase === 'error'}
        <AlertCircle class="size-3" />
      {:else if savePhase === 'unsaved'}
        <CloudCog class="size-3" />
      {:else}
        <Check class="size-3" />
      {/if}
      {saveLabel()}
    </Badge>
    <span>{formatTime(workspace.draft.updated_at)}</span>
  </footer>
</section>
