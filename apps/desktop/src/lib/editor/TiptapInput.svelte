<script lang="ts">
  import { Editor, type EditorOptions } from '@tiptap/core'
  import type { EditorState } from '@tiptap/pm/state'
  import { onMount } from 'svelte'
  import { distinguishUntidiedText } from '../preferences'

  /** Adapters own their schema and value projection; this surface owns editing lifecycle. */
  export let options: Partial<EditorOptions>
  export let editor: Editor | null = null
  export let disabled = false
  export let label: string
  export let placeholder = ''
  export let attributes: Record<string, string> = {}
  export let hostClass = ''
  export let contentClass = ''
  export let onFocus: () => void = () => {}
  export let focusOnMount = false

  let editorHost: HTMLDivElement

  function inputAttributes(state: EditorState) {
    const supplied = options.editorProps?.attributes
    const base = typeof supplied === 'function' ? supplied(state) : supplied
    return {
      ...base,
      ...attributes,
      class: ['tiptap-content', base?.class, attributes.class, contentClass].filter(Boolean).join(' '),
      role: 'textbox',
      'aria-multiline': 'true',
      'aria-label': label,
      'aria-disabled': String(disabled),
      'data-placeholder': placeholder,
    }
  }

  function refreshAttributes() {
    if (!editor) return
    editor.setOptions({ editorProps: { ...editor.options.editorProps, attributes: inputAttributes } })
    refreshEmptyState(editor)
  }

  function refreshEmptyState(source: Editor) {
    source.view.dom.setAttribute('data-empty', String(source.isEmpty))
  }

  onMount(() => {
    editor = new Editor({
      ...options,
      element: editorHost,
      editable: !disabled,
      editorProps: { ...options.editorProps, attributes: inputAttributes },
      onCreate: (event) => {
        refreshEmptyState(event.editor)
        options.onCreate?.(event)
        if (focusOnMount && !disabled) event.editor.commands.focus(undefined, { scrollIntoView: false })
      },
      onTransaction: (event) => {
        refreshEmptyState(event.editor)
        options.onTransaction?.(event)
      },
      onFocus: (event) => {
        options.onFocus?.(event)
        if (!disabled) onFocus()
      },
    })
    return () => {
      editor?.destroy()
      editor = null
    }
  })

  $: if (editor && editor.isEditable === disabled) editor.setEditable(!disabled, false)
  $: {
    editor, attributes, contentClass, label, placeholder, disabled
    refreshAttributes()
  }
</script>

<div class={`tiptap-input ${hostClass}`} class:distinguish-untidied={$distinguishUntidiedText} bind:this={editorHost}></div>

<style>
  .tiptap-input :global(.tiptap-content) {
    outline: none;
    overflow-wrap: anywhere;
  }

  .tiptap-input :global(.tiptap-content[data-empty='true']::before) {
    float: left;
    height: 0;
    color: var(--muted-foreground);
    content: attr(data-placeholder);
    pointer-events: none;
  }

  .tiptap-input.distinguish-untidied :global(.tiptap-content p[data-cleanup-state='pending']),
  .tiptap-input :global(.tiptap-content p.speech-segment-tidying) {
    position: relative;
    padding-inline-start: 22px;
  }

  .tiptap-input :global(.speech-origin-marker) {
    display: none;
  }

  .tiptap-input.distinguish-untidied :global(.speech-origin-marker),
  .tiptap-input :global(.speech-origin-marker.speech-segment-tidying) {
    display: inline-block;
    width: 14px;
    height: 14px;
    margin-inline-end: 6px;
    vertical-align: -0.12em;
    pointer-events: none;
    user-select: none;
  }

  .tiptap-input.distinguish-untidied :global(.tiptap-content p[data-cleanup-state='pending']:not(.speech-segment-tidying)::before),
  .tiptap-input.distinguish-untidied :global(.speech-origin-marker:not(.speech-segment-tidying)::before) {
    display: block;
    width: 14px;
    height: 14px;
    background-color: color-mix(in oklab, var(--primary) 58%, var(--muted-foreground));
    content: '';
    -webkit-mask: url("data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20viewBox='0%200%2024%2024'%3E%3Cpath%20fill='black'%20d='M12%2014q1.25%200%202.125-.875T15%2011V5q0-1.25-.875-2.125T12%202q-1.25%200-2.125.875T9%205v6q0%201.25.875%202.125T12%2014Zm-1%207v-3.075q-2.6-.35-4.3-2.325T5%2011h2q0%202.075%201.463%203.537T12%2016q2.075%200%203.538-1.463T17%2011h2q0%202.625-1.7%204.6T13%2017.925V21Z'/%3E%3C/svg%3E") center / contain no-repeat;
    mask: url("data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20viewBox='0%200%2024%2024'%3E%3Cpath%20fill='black'%20d='M12%2014q1.25%200%202.125-.875T15%2011V5q0-1.25-.875-2.125T12%202q-1.25%200-2.125.875T9%205v6q0%201.25.875%202.125T12%2014Zm-1%207v-3.075q-2.6-.35-4.3-2.325T5%2011h2q0%202.075%201.463%203.537T12%2016q2.075%200%203.538-1.463T17%2011h2q0%202.625-1.7%204.6T13%2017.925V21Z'/%3E%3C/svg%3E") center / contain no-repeat;
  }

  .tiptap-input.distinguish-untidied :global(.tiptap-content p[data-cleanup-state='pending']:not(.speech-segment-tidying)::before) {
    position: absolute;
    top: 0.42em;
    inset-inline-start: 1px;
  }

  .tiptap-input :global(.tiptap-content p.speech-segment-tidying::before),
  .tiptap-input :global(.speech-origin-marker.speech-segment-tidying::before) {
    display: block;
    width: 12px;
    height: 12px;
    border: 2px solid color-mix(in oklab, var(--primary) 22%, transparent);
    border-top-color: var(--primary);
    border-radius: 999px;
    animation: speech-tidying-spin 0.75s linear infinite;
    content: '';
  }

  .tiptap-input :global(.tiptap-content p.speech-segment-tidying::before) {
    position: absolute;
    top: 0.48em;
    inset-inline-start: 1px;
  }

  @keyframes speech-tidying-spin {
    to { transform: rotate(360deg); }
  }

  @media (prefers-reduced-motion: reduce) {
    .tiptap-input :global(.speech-segment-tidying::before) { animation-duration: 1.8s; }
  }
</style>
