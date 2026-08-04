<script lang="ts">
  import { Eye, File, FileImage, FileText, TextCursorInput, Trash2 } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import type { AttachmentView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let attachments: AttachmentView[] = []
  export let attachmentBusy = false
  export let readOnly = false
  export let onInsert: (attachment: AttachmentView) => void = () => {}
  export let onRemove: (attachment: AttachmentView) => void = () => {}
  export let onPreview: (attachment: AttachmentView) => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function mediaIcon(attachment: AttachmentView) {
    if (attachment.media_type.startsWith('image/')) return FileImage
    if (attachment.media_type === 'text/markdown') return FileText
    return File
  }

  function mediaLabel(attachment: AttachmentView) {
    if (attachment.media_type.startsWith('image/')) return tr('图片')
    if (attachment.media_type === 'text/markdown') return 'Markdown'
    return attachment.file_name.split('.').pop()?.toUpperCase() ?? tr('附件')
  }
</script>

<section class="border-b p-4">
  <header class="mb-2 flex items-center gap-2">
    <FileText class="size-4 text-muted-foreground" />
    <strong class="text-xs font-medium">{tr('附件')}</strong>
    <Badge variant="secondary" class="ml-auto h-5 px-1.5 text-[9px]">
      {attachments.length}
    </Badge>
  </header>

  {#if attachments.length > 0}
    <div class="divide-y" aria-label={tr('文档附件')}>
      {#each attachments as attachment (attachment.attachment_id)}
        {@const Icon = mediaIcon(attachment)}
        <div class="flex min-w-0 items-center gap-2 py-2">
          <Icon class="size-3.5 shrink-0 text-muted-foreground" />
          <div class="min-w-0 flex-1">
            <strong class="block truncate text-[10px] font-medium">{attachment.file_name}</strong>
            <span class="block text-[9px] text-muted-foreground">
              {mediaLabel(attachment)} · {(attachment.byte_size / 1024).toFixed(1)} KiB
            </span>
          </div>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={tr('预览 {name}', { name: attachment.file_name })}
            title={tr('预览')}
            onclick={() => onPreview(attachment)}
          >
            <Eye />
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={tr('插入正文 {name}', { name: attachment.file_name })}
            title={tr('插入正文')}
            disabled={attachmentBusy || readOnly}
            onclick={() => onInsert(attachment)}
          >
            <TextCursorInput />
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            class="text-destructive hover:text-destructive"
            aria-label={tr('删除 {name}', { name: attachment.file_name })}
            title={tr('删除')}
            disabled={attachmentBusy || readOnly}
            onclick={() => onRemove(attachment)}
          >
            <Trash2 />
          </Button>
        </div>
      {/each}
    </div>
  {:else}
    <p class="m-0 py-2 text-[10px] leading-4 text-muted-foreground">
      {tr('截图和导入的文件会保存在这里。')}
    </p>
  {/if}
</section>
