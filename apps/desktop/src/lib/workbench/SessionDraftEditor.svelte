<script lang="ts">
  import { onDestroy } from 'svelte'

  import RichFeedbackEditor from '$lib/RichFeedbackEditor.svelte'
  import type { FeedbackEditorHandle } from './types'

  export let requestId: string
  export let markdown = ''
  export let previews: Record<string, string> = {}
  export let disabled = false
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let onChange: (markdown: string) => void = () => {}
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
  {previews}
  {disabled}
  {onOpenAttachment}
  {onChange}
/>
