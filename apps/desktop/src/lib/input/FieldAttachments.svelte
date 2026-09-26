<script lang="ts">
  import { Image, Paperclip, X } from '@lucide/svelte'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import { fieldAttachmentText, removeFieldAttachment } from './fieldAttachmentText'
  import { unavailableInputToolsState, useInputTools } from './inputToolsContext'

  export let value = ''
  export let disabled = false
  export let onChange: (value: string) => void
  const tools = useInputTools()
  const toolsState = tools?.state ?? unavailableInputToolsState
  $: ids = fieldAttachmentText(value, $toolsState.attachments).attachmentIds
  $: attachments = ids.flatMap((id) => $toolsState.attachments.filter((attachment) => attachment.attachment_id === id))
</script>

{#if attachments.length > 0}
  <div class="flex flex-wrap gap-1.5 border-t px-3 py-2" aria-label={t($locale, 'Attachments')} data-field-attachments>
    {#each attachments as attachment (attachment.attachment_id)}
      <span class="inline-flex max-w-full items-center rounded-md border bg-muted/40 text-xs">
        <button type="button" class="flex min-w-0 items-center gap-1.5 rounded-l-md px-2 py-1.5 hover:bg-muted"
          title={attachment.file_name} aria-label={t($locale, 'Preview {name}', { name: attachment.file_name })}
          onclick={() => tools?.preview(attachment.attachment_id)}>
          {#if attachment.media_type.startsWith('image/')}<Image class="size-3 shrink-0" />{:else}<Paperclip class="size-3 shrink-0" />{/if}
          <span class="truncate">{attachment.file_name}</span>
        </button>
        {#if !disabled && !$toolsState.disabled}
          <button type="button" class="mr-1 shrink-0 rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
            aria-label={t($locale, 'Remove attachment reference {name}', { name: attachment.file_name })}
            title={t($locale, 'Remove attachment reference {name}', { name: attachment.file_name })}
            onclick={() => onChange(removeFieldAttachment(value, attachment.attachment_id, $toolsState.attachments))}><X class="size-3" /></button>
        {/if}
      </span>
    {/each}
  </div>
{/if}
