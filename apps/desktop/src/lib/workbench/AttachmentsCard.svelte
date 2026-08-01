<script lang="ts">
  import type { AttachmentView } from '../feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'

  export let attachments: AttachmentView[] = []
  export let attachmentBusy = false
  export let readOnly = false
  export let onInsert: (attachment: AttachmentView) => void = () => {}
  export let onRemove: (attachment: AttachmentView) => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<section class="attachments-card">
  <div class="rail-heading">
    <div>
      <p class="eyebrow">ATTACHMENTS</p>
      <strong>{tr('文档素材')}</strong>
    </div>
    <span>{attachments.length}</span>
  </div>
  {#if attachments.length > 0}
    <div class="attachment-list" aria-label={tr('文档附件')}>
      {#each attachments as attachment (attachment.attachment_id)}
        <div class="attachment-row">
          <span class="attachment-dot"></span>
          <div>
            <strong>{attachment.file_name}</strong>
            <span>{(attachment.byte_size / 1024).toFixed(1)} KiB</span>
          </div>
          <div class="attachment-actions">
            <button
              aria-label={tr('插入正文 {name}', { name: attachment.file_name })}
              disabled={attachmentBusy || readOnly}
              onclick={() => onInsert(attachment)}
            >{tr('插入')}</button>
            <button
              class="remove-attachment"
              aria-label={tr('删除 {name}', { name: attachment.file_name })}
              disabled={attachmentBusy || readOnly}
              onclick={() => onRemove(attachment)}
            >×</button>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <p class="rail-empty">{tr('截图和导入的文件会直接进入正文，也会在这里留档。')}</p>
  {/if}
</section>
