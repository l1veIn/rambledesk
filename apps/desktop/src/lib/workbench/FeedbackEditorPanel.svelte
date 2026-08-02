<script lang="ts">
  import { AlertCircle, Check, CheckCircle2, CloudCog, Info, LoaderCircle } from '@lucide/svelte'

  import * as Alert from '$lib/components/ui/alert'
  import { Badge } from '$lib/components/ui/badge'
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
  export let attachmentMessage = ''
  export let attachmentMessageTone: 'info' | 'success' | 'error' = 'info'
  export let saveMessage = ''
  export let formatTime: (value: string | null | undefined) => string
  export let onChange: (markdown: string) => void = () => {}

  let richEditor: RichFeedbackEditor

  $: readOnly =
    workspace.request.status === 'completed' || workspace.request.status === 'cancelled'

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
    'flex min-h-0 flex-1 flex-col p-5 transition-colors',
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
    <Badge
      variant={savePhase === 'error' ? 'destructive' : 'secondary'}
      class="h-6 gap-1 px-2 text-[9px]"
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
  </header>

  <RichFeedbackEditor
    bind:this={richEditor}
    markdown={draftBody}
    previews={attachmentPreviews}
    disabled={readOnly}
    onChange={onChange}
  />

  {#if attachmentMessage}
    <Alert.Root
      variant={attachmentMessageTone === 'error' ? 'destructive' : 'default'}
      class={[
        'mt-3',
        attachmentMessageTone === 'success'
          ? 'border-success/30 bg-success/5 text-success'
          : attachmentMessageTone === 'info'
            ? 'border-info/30 bg-info/5 text-info'
            : '',
      ]}
    >
      {#if attachmentMessageTone === 'success'}
        <CheckCircle2 />
      {:else if attachmentMessageTone === 'info'}
        <Info />
      {:else}
        <AlertCircle />
      {/if}
      <Alert.Title>
        {attachmentMessageTone === 'success'
          ? tr('附件操作完成')
          : attachmentMessageTone === 'info'
            ? tr('附件操作状态')
            : tr('附件操作失败')}
      </Alert.Title>
      <Alert.Description>{attachmentMessage}</Alert.Description>
    </Alert.Root>
  {/if}

  {#if saveMessage}
    <Alert.Root variant="destructive" class="mt-3">
      <AlertCircle />
      <Alert.Title>{tr('保存失败')}</Alert.Title>
      <Alert.Description>
        {saveMessage}。{tr('请重新载入后再试，当前文字仍保留在编辑器中。')}
      </Alert.Description>
    </Alert.Root>
  {/if}

  <footer class="mt-2 flex items-center gap-3 text-[9px] text-muted-foreground">
    <span>{tr('{count} 字符', { count: draftBody.length.toLocaleString($locale) })}</span>
    <span>Markdown</span>
    <span class="ml-auto">{formatTime(workspace.draft.updated_at)}</span>
  </footer>
</section>
