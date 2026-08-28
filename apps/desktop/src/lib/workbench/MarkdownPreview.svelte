<script lang="ts">
  import { Editor } from '@tiptap/core'
  import { onMount } from 'svelte'

  import { attachmentIdFromUrl } from '$lib/attachmentMarkdown'
  import { feedbackEditorExtensions } from '$lib/feedbackEditorExtensions'
  import { isSafeHttpUrl } from '$lib/linkify'
  import { openExternalUrl } from '$lib/openExternalUrl'

  export let markdown = ''
  export let previews: Record<string, string> = {}
  export let onOpenAttachment: (attachmentId: string) => void = () => {}

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let renderedMarkdown = ''

  onMount(() => {
    editor = new Editor({
      element: editorHost,
      extensions: feedbackEditorExtensions(),
      content: markdown,
      contentType: 'markdown',
      editable: false,
      editorProps: {
        attributes: {
          class: 'feedback-prose attachment-markdown-prose',
          'aria-label': 'Markdown preview',
        },
        handleClick: (_view, _pos, event) => {
          const target = event.target as HTMLElement | null
          const attachment = target?.closest?.('[data-attachment-id]')
          const attachmentId = attachment?.getAttribute('data-attachment-id')
          if (attachmentId) {
            event.preventDefault()
            event.stopPropagation()
            onOpenAttachment(attachmentId)
            return true
          }
          const anchor = target?.closest?.('a[href]')
          if (!anchor) return false
          const href = anchor.getAttribute('href') ?? ''
          if (!isSafeHttpUrl(href)) return false
          event.preventDefault()
          event.stopPropagation()
          void openExternalUrl(href).catch((cause) => {
            console.warn('Could not open external URL', cause)
          })
          return true
        },
      },
      onCreate: () => {
        renderedMarkdown = markdown
      },
    })

    return () => {
      editor?.destroy()
      editor = null
    }
  })

  $: if (editor && markdown !== renderedMarkdown) {
    editor.commands.setContent(markdown, {
      contentType: 'markdown',
      emitUpdate: false,
    })
    renderedMarkdown = markdown
    hydrateAttachmentImages()
  }
  $: if (editor) {
    previews
    hydrateAttachmentImages()
  }

  function hydrateAttachmentImages() {
    if (!editor) return
    let transaction = editor.state.tr
    let changed = false
    editor.state.doc.descendants((node, position) => {
      if (node.type.name !== 'image') return
      const attachmentId = node.attrs.attachmentId ?? attachmentIdFromUrl(node.attrs.src)
      const preview = attachmentId ? previews[attachmentId] : undefined
      if (!attachmentId || !preview || node.attrs.src === preview) return
      transaction = transaction.setNodeMarkup(position, undefined, {
        ...node.attrs,
        attachmentId,
        src: preview,
      })
      changed = true
    })
    if (changed) editor.view.dispatch(transaction)
  }
</script>

<div class="h-full min-h-0 overflow-auto rounded-lg border bg-background px-6 py-5">
  <div bind:this={editorHost}></div>
</div>

<style>
  :global(.attachment-markdown-prose) {
    max-width: 82ch;
    min-height: 100%;
    margin: 0 auto;
    color: var(--foreground);
    font-size: 14px;
    line-height: 1.75;
    outline: none;
  }

  :global(.attachment-markdown-prose > *:first-child) {
    margin-top: 0;
  }

  :global(.attachment-markdown-prose h1),
  :global(.attachment-markdown-prose h2),
  :global(.attachment-markdown-prose h3) {
    color: var(--foreground);
    font-weight: 650;
    line-height: 1.3;
  }

  :global(.attachment-markdown-prose h1) {
    margin: 0 0 0.8em;
    font-size: 1.75em;
  }

  :global(.attachment-markdown-prose h2) {
    margin: 1.6em 0 0.65em;
    padding-bottom: 0.35em;
    border-bottom: 1px solid var(--border);
    font-size: 1.35em;
  }

  :global(.attachment-markdown-prose h3) {
    margin: 1.35em 0 0.55em;
    font-size: 1.12em;
  }

  :global(.attachment-markdown-prose p) {
    margin: 0.75em 0;
  }

  :global(.attachment-markdown-prose ul:not([data-type='taskList'])) {
    margin: 0.75em 0;
    padding-left: 1.6em;
    list-style: disc;
  }

  :global(.attachment-markdown-prose ol) {
    margin: 0.75em 0;
    padding-left: 1.7em;
    list-style: decimal;
  }

  :global(.attachment-markdown-prose li) {
    margin: 0.28em 0;
  }

  :global(.attachment-markdown-prose li > p) {
    margin: 0;
  }

  :global(.attachment-markdown-prose ul[data-type='taskList']) {
    margin: 0.75em 0;
    padding: 0;
    list-style: none;
  }

  /* Task items carry `data-checked`, not `data-type`; scope through the list. */
  :global(.attachment-markdown-prose ul[data-type='taskList'] > li) {
    display: flex;
    align-items: flex-start;
    gap: 0.55em;
  }

  :global(.attachment-markdown-prose ul[data-type='taskList'] > li > label) {
    margin-top: 0.26em;
  }

  :global(.attachment-markdown-prose ul[data-type='taskList'] > li > div) {
    min-width: 0;
    flex: 1;
  }

  :global(.attachment-markdown-prose blockquote) {
    margin: 1em 0;
    padding: 0.55em 1em;
    border-left: 3px solid var(--primary);
    color: var(--muted-foreground);
    background: color-mix(in oklab, var(--muted) 65%, transparent);
  }

  :global(.attachment-markdown-prose pre) {
    margin: 1em 0;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: calc(var(--radius) - 2px);
    background: var(--muted);
    padding: 0.9em 1em;
    font-size: 0.9em;
    line-height: 1.6;
  }

  :global(.attachment-markdown-prose code) {
    border-radius: 4px;
    background: var(--muted);
    padding: 0.12em 0.34em;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.9em;
  }

  :global(.attachment-markdown-prose pre code) {
    background: transparent;
    padding: 0;
    font-size: inherit;
  }

  :global(.attachment-markdown-prose table) {
    width: 100%;
    margin: 1em 0;
    border-collapse: collapse;
    font-size: 0.95em;
  }

  :global(.attachment-markdown-prose th),
  :global(.attachment-markdown-prose td) {
    border: 1px solid var(--border);
    padding: 0.55em 0.7em;
    text-align: left;
    vertical-align: top;
  }

  :global(.attachment-markdown-prose th) {
    background: var(--muted);
    font-weight: 650;
  }

  :global(.attachment-markdown-prose hr) {
    margin: 1.6em 0;
    border: 0;
    border-top: 1px solid var(--border);
  }

  :global(.attachment-markdown-prose a) {
    color: var(--primary);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  :global(.attachment-markdown-prose img) {
    display: block;
    width: auto;
    max-width: min(100%, 900px);
    max-height: 620px;
    margin: 18px auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    object-fit: contain;
    background: var(--muted);
  }

  :global(.attachment-markdown-prose a.attachment-file-chip) {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0 2px;
    padding: 2px 10px 2px 4px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: color-mix(in oklab, var(--muted) 55%, transparent);
    color: var(--foreground);
    font-size: 12px;
    text-decoration: none;
    cursor: pointer;
  }

  :global(.attachment-markdown-prose .attachment-file-chip-ext) {
    padding: 1px 5px;
    border-radius: 999px;
    background: var(--primary);
    color: var(--primary-foreground);
    font-size: 9px;
    font-weight: 650;
  }
</style>
