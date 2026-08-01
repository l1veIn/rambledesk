<script lang="ts">
  import RichFeedbackEditor from '../RichFeedbackEditor.svelte'
  import type { AttachmentView, FeedbackWorkspaceView } from '../feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import type { SavePhase } from './types'

  export let workspace: FeedbackWorkspaceView
  export let draftBody = ''
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let attachmentMessage = ''
  export let saveMessage = ''
  export let formatTime: (value: string | null | undefined) => string
  export let onChange: (markdown: string) => void = () => {}

  let richEditor: RichFeedbackEditor

  $: readOnly =
    workspace.request.status === 'completed' || workspace.request.status === 'cancelled'

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  export function insertAttachments(attachments: AttachmentView[]) {
    return richEditor?.insertAttachments(attachments) ?? false
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

<section class:drag-active={dragActive} class="editor-section">
  <div class="editor-heading">
    <div>
      <p class="eyebrow">YOUR FEEDBACK</p>
      <h3>{tr('边体验，边记下来')}</h3>
    </div>
    <div class:failed={savePhase === 'error'} class="save-state" aria-live="polite">
      <span class="save-dot"></span>
      {#if savePhase === 'saving'}
        {tr('正在保存…')}
      {:else if savePhase === 'unsaved'}
        {tr('等待自动保存')}
      {:else if savePhase === 'error'}
        {tr('保存失败')}
      {:else}
        {tr('已保存')} · revision {savedRevision}
      {/if}
    </div>
  </div>

  <RichFeedbackEditor
    bind:this={richEditor}
    markdown={draftBody}
    previews={attachmentPreviews}
    disabled={readOnly}
    onChange={onChange}
  />

  {#if attachmentMessage}
    <p class="inline-error">{attachmentMessage}</p>
  {/if}

  {#if saveMessage}
    <p class="inline-error">{saveMessage}。{tr('请重新载入后再试，当前文字仍保留在编辑器中。')}</p>
  {/if}

  <footer class="editor-footer">
    <span>{tr('{count} 字符', { count: draftBody.length.toLocaleString($locale) })}</span>
    <span>{tr('Markdown 文档流')}</span>
    <span>{formatTime(workspace.draft.updated_at)}</span>
  </footer>
</section>
