<script lang="ts">
  import { Eye, File, FileImage, FileText, Trash2 } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import type { AttachmentView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let attachments: AttachmentView[] = []
  export let attachmentBusy = false
  export let readOnly = false
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
    if (attachment.media_type.startsWith('image/')) return tr('Image')
    if (attachment.media_type === 'text/markdown') return 'Markdown'
    return attachment.file_name.split('.').pop()?.toUpperCase() ?? tr('Attachments')
  }
</script>

<!-- Same data that is inserted into the document as chips and images; this list
     only exists inside the footer's attachment popover for preview and removal. -->
<div class="divide-y" aria-label={tr('Document attachments')}>
  {#each attachments as attachment (attachment.attachment_id)}
    {@const Icon = mediaIcon(attachment)}
    <div class="flex min-w-0 items-center gap-2 px-2 py-2">
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
        aria-label={tr('Preview {name}', { name: attachment.file_name })}
        title={tr('Preview')}
        onclick={() => onPreview(attachment)}
      >
        <Eye />
      </Button>
      <Button
        variant="ghost"
        size="icon-xs"
        class="text-destructive hover:text-destructive"
        aria-label={tr('Delete {name}', { name: attachment.file_name })}
        title={tr('Delete')}
        disabled={attachmentBusy || readOnly}
        onclick={() => onRemove(attachment)}
      >
        <Trash2 />
      </Button>
    </div>
  {/each}
</div>
