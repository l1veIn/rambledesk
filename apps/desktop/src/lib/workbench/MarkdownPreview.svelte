<script lang="ts">
  import { Editor } from '@tiptap/core'
  import { TableKit } from '@tiptap/extension-table'
  import TaskItem from '@tiptap/extension-task-item'
  import TaskList from '@tiptap/extension-task-list'
  import { Markdown } from '@tiptap/markdown'
  import StarterKit from '@tiptap/starter-kit'
  import { onMount } from 'svelte'

  export let markdown = ''

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let renderedMarkdown = ''

  onMount(() => {
    editor = new Editor({
      element: editorHost,
      extensions: [
        StarterKit,
        TableKit,
        TaskList,
        TaskItem.configure({ nested: true }),
        Markdown,
      ],
      content: markdown,
      contentType: 'markdown',
      editable: false,
      editorProps: {
        attributes: {
          class: 'feedback-prose attachment-markdown-prose',
          'aria-label': 'Markdown preview',
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

  :global(.attachment-markdown-prose li[data-type='taskItem']) {
    display: flex;
    align-items: flex-start;
    gap: 0.55em;
  }

  :global(.attachment-markdown-prose li[data-type='taskItem'] > label) {
    margin-top: 0.26em;
  }

  :global(.attachment-markdown-prose li[data-type='taskItem'] > div) {
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
</style>
