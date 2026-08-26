<script lang="ts">
  import { onDestroy } from 'svelte'

  import RichFeedbackEditor from '$lib/RichFeedbackEditor.svelte'
  import type { FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import type { FeedbackEditorHandle } from './types'

  export let requestId: string
  export let markdown = ''
  export let documentJson: string | null = null
  export let previews: Record<string, string> = {}
  export let disabled = false
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let onChange: (snapshot: FeedbackDraftSnapshot) => void = () => {}
  export let onReady: (requestId: string, editor: FeedbackEditorHandle | null) => void = () => {}

  let editor: RichFeedbackEditor

  $: if (editor) onReady(requestId, editor)

  onDestroy(() => {
    onReady(requestId, null)
  })
</script>

<RichFeedbackEditor
  bind:this={editor}
  {markdown}
  {documentJson}
  {previews}
  {disabled}
  acceptExternalMarkdown={false}
  {onOpenAttachment}
  {onChange}
/>
