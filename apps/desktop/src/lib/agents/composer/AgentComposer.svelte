<!-- Svelte adaptation of Codeg rich-composer.tsx / message input at commit 3ebdfed.
     SPDX-License-Identifier: Apache-2.0
     RambleDesk: controlled text contract, session-scoped actions, host capability providers. -->
<script lang="ts">
  import { Editor } from '@tiptap/core'
  import { ArrowUp, LoaderCircle, Square } from '@lucide/svelte'
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { locale as appLocale, type Locale } from '$lib/preferences'
  import { buildComposerExtensions } from './editor-config'
  import { replaceComposerText } from './composer-commands'
  import { decidePastedContent, textToSeededDoc } from './plain-text-content'
  import { composerLeafText, serializeDocToText } from './to-prompt-blocks'
  import { decideComposerKey } from './submit-key'
  import { isImeCompositionKey } from './ime-composition'
  import { composerText } from './composer-text'
  import type { ComposerSubmitShortcut } from './types'

  export let value = ''
  export let draftKey = ''
  export let onchange: (text: string) => void
  export let onsubmit: (text: string) => void | Promise<void>
  export let disabled = false
  export let busy = false
  export let sendDisabled = false
  export let oncancel: (() => void | Promise<void>) | undefined = undefined
  export let placeholder = ''
  export let ariaLabel = ''
  export let locale: Locale | undefined = undefined
  export let submitShortcut: ComposerSubmitShortcut = 'enter'

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let loadedKey = draftKey
  let editorText = value
  let actionEpoch = 0
  let alive = false
  let sending = false
  let cancelling = false
  let failure = ''
  let deferredValue = false

  $: effectiveLocale = locale ?? $appLocale
  $: hint = composerText(effectiveLocale, busy ? 'Draft saved while the agent works' : submitShortcut === 'enter'
    ? 'Enter to send · Shift+Enter for a new line' : 'Ctrl/⌘+Enter to send · Enter for a new line')
  $: canSend = !disabled && !sendDisabled && !busy && !sending && editorText.trim().length > 0
  $: if (editor) syncValue(editor, value, draftKey)
  $: if (editor) syncEditorOptions(editor, disabled, effectiveLocale, placeholder, ariaLabel)

  function tr(text: string) { return composerText(effectiveLocale, text) }

  onMount(() => {
    alive = true
    loadedKey = draftKey
    editor = new Editor({
      element: editorHost,
      extensions: buildComposerExtensions({ placeholder: () => placeholder || tr('Message the agent…') }),
      content: textToSeededDoc(value),
      editable: !disabled,
      editorProps: {
        attributes: { class: 'ramble-composer-editor', role: 'textbox', 'aria-multiline': 'true' },
        clipboardTextSerializer: (slice) => slice.content.textBetween(0, slice.content.size, '\n', composerLeafText),
        handleKeyDown: (_view, event) => handleKey(event),
        handlePaste: (_view, event) => {
          if (event.clipboardData?.files.length && !event.clipboardData.getData('text/plain')) {
            event.preventDefault()
            failure = 'Add attachments in the Ramble request.'
            return true
          }
          const content = decidePastedContent({ html: event.clipboardData?.getData('text/html') ?? '', text: event.clipboardData?.getData('text/plain') ?? '' })
          if (!content || !editor || disabled) return false
          event.preventDefault()
          return editor.commands.insertContent(content)
        },
        handleDrop: (_view, event) => {
          if (!event.dataTransfer?.files.length) return false
          event.preventDefault()
          failure = 'Add attachments in the Ramble request.'
          return true
        },
        handleDOMEvents: { compositionend: () => {
          if (deferredValue) queueMicrotask(() => { if (alive && editor) syncValue(editor, value, draftKey) })
          return false
        } },
      },
      onUpdate: ({ editor: changed }) => {
        if (loadedKey !== draftKey) return
        editorText = serializeDocToText(changed.state.doc)
        failure = ''
        onchange(editorText)
      },
    })
    editorText = serializeDocToText(editor.state.doc)
    return () => { alive = false; actionEpoch += 1; editor?.destroy(); editor = null }
  })

  function syncValue(target: Editor, next: string, key: string) {
    const switched = loadedKey !== key
    if (!switched && target.view.composing) { deferredValue = true; return }
    deferredValue = false
    if (switched) {
      actionEpoch += 1
      loadedKey = key
      sending = false; cancelling = false; failure = ''
    }
    const normalized = next.replace(/\r\n?/g, '\n')
    if (switched || serializeDocToText(target.state.doc) !== normalized) replaceComposerText(target, normalized, switched)
    editorText = serializeDocToText(target.state.doc)
  }

  function syncEditorOptions(target: Editor, locked: boolean, language: Locale, hintText: string, label: string) {
    if (target.options.editable === locked) target.setEditable(!locked, false)
    target.view.dom.setAttribute('aria-label', label || composerText(language, 'Message the agent'))
    target.view.dom.setAttribute('aria-disabled', String(locked))
    // Re-evaluate the placeholder getter after language/hint changes without replacing the doc.
    target.view.dispatch(target.state.tr.setMeta('composer-placeholder', hintText))
  }

  async function runAction(kind: 'submit' | 'cancel', operation: () => void | Promise<void>) {
    if ((kind === 'submit' && sending) || (kind === 'cancel' && cancelling)) return
    const epoch = actionEpoch
    const key = draftKey
    if (kind === 'submit') sending = true
    else cancelling = true
    failure = ''
    try { await operation() } catch {
      if (alive && epoch === actionEpoch && key === draftKey) failure = kind === 'submit' ? 'Could not send. Your draft is preserved.'
        : 'Could not cancel the current turn.'
    } finally {
      if (alive && epoch === actionEpoch && key === draftKey) {
        if (kind === 'submit') sending = false
        else cancelling = false
      }
    }
  }

  function submit() {
    if (disabled || sendDisabled || busy || sending || !editor || editor.view.composing) return
    const text = serializeDocToText(editor.state.doc).trim()
    if (!text) return
    // The host owns submit/restore draft transitions. Never erase newer typing here.
    void runAction('submit', () => onsubmit(text))
  }

  function handleKey(event: KeyboardEvent): boolean {
    if (!editor || disabled || isImeCompositionKey(event) || editor.view.composing) return false
    const action = decideComposerKey(event, { composing: editor.view.composing, inCodeBlock: false, inList: false }, {
      submit: submitShortcut, newline: submitShortcut === 'enter' ? 'shift+enter' : 'enter',
    })
    if (action === 'submit') { submit(); return true }
    if (action === 'newline') { editor.commands.setHardBreak(); return true }
    return false
  }

</script>

<div class="agent-composer relative rounded-2xl border bg-background shadow-sm transition-colors focus-within:border-primary/40 focus-within:ring-2 focus-within:ring-primary/10" class:opacity-60={disabled}>
  <div bind:this={editorHost} class="min-h-12 px-4 pb-1 pt-3"></div>
  {#if failure}<p class="m-0 px-4 pb-2 text-xs text-destructive" role="alert">{tr(failure)}</p>{/if}
  <footer class="flex min-h-11 items-center gap-1.5 px-2.5 pb-2">
    <div class="flex min-w-0 flex-1 flex-wrap items-center gap-1"><slot name="footer" /></div>
    <span class="sr-only">{hint}</span>
    {#if busy && oncancel}
      <Button variant="secondary" size="icon" class="size-8 shrink-0 rounded-xl" aria-label={tr('Cancel current turn')} title={tr('Cancel current turn')} disabled={disabled || cancelling}
        onclick={() => void runAction('cancel', () => oncancel?.())}>{#if cancelling}<LoaderCircle class="size-4 animate-spin" />{:else}<Square class="size-3.5 fill-current" />{/if}</Button>
    {:else}
      <Button size="icon" class="size-8 shrink-0 rounded-xl" aria-label={tr('Send message')} title={`${tr('Send message')} · ${hint}`} disabled={!canSend} onclick={submit}>
        {#if sending}<LoaderCircle class="size-4 animate-spin" />{:else}<ArrowUp class="size-4" />{/if}
      </Button>
    {/if}
  </footer>
</div>

<style>
  .agent-composer :global(.ramble-composer-editor) { min-height: 1.75rem; max-height: min(16rem, 35vh); overflow-y: auto; outline: none; font-size: 0.875rem; line-height: 1.65; overflow-wrap: anywhere; white-space: pre-wrap; padding-bottom: 0.25rem; }
  .agent-composer :global(.ramble-composer-editor p) { margin: 0; }
  .agent-composer :global(.ramble-composer-empty::before) { color: var(--muted-foreground); content: attr(data-placeholder); float: left; height: 0; pointer-events: none; }
  .agent-composer :global(.ramble-composer-quote-marker) { color: transparent; border-left: 2px solid var(--muted-foreground); margin-left: 0.1em; }
  .agent-composer :global(.ramble-composer-inactive-selection) { background: color-mix(in oklch, var(--primary) 18%, transparent); }
  .agent-composer :global(.ramble-composer-reference) { display: inline; border-radius: 0.35rem; background: var(--muted); padding: 0.1rem 0.3rem; color: var(--foreground); font-size: 0.85em; white-space: normal; }
  .agent-composer :global(.ProseMirror-selectednode) { outline: 1px solid var(--primary); }
</style>
