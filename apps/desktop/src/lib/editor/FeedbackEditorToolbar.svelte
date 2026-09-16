<script lang="ts">
  import type { Editor } from '@tiptap/core'
  import type { Snippet } from 'svelte'
  import { Bold, Heading2, Italic, List, Quote, Redo2, Type, Undo2 } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import { t } from '../i18n'
  import { locale } from '../preferences'

  export let editor: Editor | null = null
  export let disabled = false
  export let actions: Snippet | undefined = undefined
  export let state: {
    canUndo: boolean
    canRedo: boolean
    bold: boolean
    italic: boolean
    heading2: boolean
    bulletList: boolean
    blockquote: boolean
  }

  let formattingOpen = false
</script>

<div class="shrink-0 border-b bg-muted/30">
  <div class="flex min-h-10 flex-wrap items-center gap-1 px-2 py-1">
    {@render actions?.()}
    <div class="ml-auto flex items-center gap-0.5">
      <Button
        variant={formattingOpen ? 'secondary' : 'ghost'}
        size="icon-sm"
        aria-label={t($locale, 'Document formatting')}
        title={t($locale, 'Document formatting')}
        aria-expanded={formattingOpen}
        {disabled}
        onclick={() => formattingOpen = !formattingOpen}
      >
        <Type />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={t($locale, 'Undo')}
        title={t($locale, 'Undo')}
        disabled={disabled || !state.canUndo}
        onclick={() => editor?.chain().focus().undo().run()}
      >
        <Undo2 />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={t($locale, 'Redo')}
        title={t($locale, 'Redo')}
        disabled={disabled || !state.canRedo}
        onclick={() => editor?.chain().focus().redo().run()}
      >
        <Redo2 />
      </Button>
    </div>
  </div>
  {#if formattingOpen}
    <div class="flex flex-wrap items-center gap-1 border-t px-2 py-1" aria-label={t($locale, 'Document formatting')}>
      <Button
        variant={state.bold ? 'secondary' : 'ghost'}
        size="icon-sm"
        aria-label={t($locale, 'Bold')}
        title={t($locale, 'Bold')}
        aria-pressed={state.bold}
        {disabled}
        onclick={() => editor?.chain().focus().toggleBold().run()}
      >
        <Bold />
      </Button>
      <Button
        variant={state.italic ? 'secondary' : 'ghost'}
        size="icon-sm"
        aria-label={t($locale, 'Italic')}
        title={t($locale, 'Italic')}
        aria-pressed={state.italic}
        {disabled}
        onclick={() => editor?.chain().focus().toggleItalic().run()}
      >
        <Italic />
      </Button>
      <Button
        variant={state.heading2 ? 'secondary' : 'ghost'}
        size="icon-sm"
        aria-label={t($locale, 'Heading 2')}
        title={t($locale, 'Heading 2')}
        aria-pressed={state.heading2}
        {disabled}
        onclick={() => editor?.chain().focus().toggleHeading({ level: 2 }).run()}
      >
        <Heading2 />
      </Button>
      <Button
        variant={state.bulletList ? 'secondary' : 'ghost'}
        size="icon-sm"
        aria-label={t($locale, 'Bullet list')}
        title={t($locale, 'Bullet list')}
        aria-pressed={state.bulletList}
        {disabled}
        onclick={() => editor?.chain().focus().toggleBulletList().run()}
      >
        <List />
      </Button>
      <Button
        variant={state.blockquote ? 'secondary' : 'ghost'}
        size="icon-sm"
        aria-label={t($locale, 'Quote')}
        title={t($locale, 'Quote')}
        aria-pressed={state.blockquote}
        {disabled}
        onclick={() => editor?.chain().focus().toggleBlockquote().run()}
      >
        <Quote />
      </Button>
    </div>
  {/if}
</div>
