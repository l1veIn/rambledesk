<!-- Svelte adaptation of Codeg rich-composer.tsx / message input at commit 3ebdfed.
     SPDX-License-Identifier: Apache-2.0
     RambleDesk: controlled text contract, session-scoped actions, host capability providers. -->
<script lang="ts">
  import { Editor } from '@tiptap/core'
  import { AtSign, ArrowUp, LoaderCircle, Paperclip, Square, X } from '@lucide/svelte'
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { locale as appLocale, type Locale } from '$lib/preferences'
  import { buildComposerExtensions } from './editor-config'
  import { appendComposerQuote, insertComposerText, replaceComposerText } from './composer-commands'
  import { decidePastedContent, textToSeededDoc } from './plain-text-content'
  import { composerLeafText, serializeDocToText } from './to-prompt-blocks'
  import { decideComposerKey } from './submit-key'
  import { isImeCompositionKey } from './ime-composition'
  import { canStartMentionAt, createReferenceLookup, findReferenceQuery, type ReferenceQuery } from './reference-search'
  import { composerText } from './composer-text'
  import type { ComposerAttachment, ComposerReference, ComposerSubmitShortcut, ReferenceSearch } from './types'

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
  export let referenceSearch: ReferenceSearch | undefined = undefined
  export let attachments: readonly ComposerAttachment[] = []
  export let onAddAttachments: (() => void | Promise<void>) | undefined = undefined
  export let onRemoveAttachment: ((id: string) => void | Promise<void>) | undefined = undefined
  export let onPasteFiles: ((files: readonly File[]) => void | Promise<void>) | undefined = undefined

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let loadedKey = draftKey
  let editorText = value
  let actionEpoch = 0
  let alive = false
  let sending = false
  let cancelling = false
  let adding = false
  let failure = ''
  let deferredValue = false
  let query: ReferenceQuery | null = null
  let queryIdentity = ''
  let dismissedIdentity = ''
  let suggestions: readonly ComposerReference[] = []
  let selectedSuggestion = 0
  let searching = false
  let searchFailed = false
  const lookup = createReferenceLookup()

  $: effectiveLocale = locale ?? $appLocale
  $: hint = composerText(effectiveLocale, busy ? 'Draft saved while the agent works' : submitShortcut === 'enter'
    ? 'Enter to send · Shift+Enter for a new line' : 'Ctrl/⌘+Enter to send · Enter for a new line')
  $: canSend = !disabled && !sendDisabled && !busy && !sending && !adding && (editorText.trim().length > 0 || attachments.length > 0)
  $: if (editor) syncValue(editor, value, draftKey)
  $: if (editor) syncEditorOptions(editor, disabled, effectiveLocale, placeholder, ariaLabel)
  $: if (!referenceSearch || disabled) closeSuggestions()

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
          const files = Array.from(event.clipboardData?.files ?? [])
          if (files.length > 0) return receiveFiles(files)
          const content = decidePastedContent({ html: event.clipboardData?.getData('text/html') ?? '', text: event.clipboardData?.getData('text/plain') ?? '' })
          if (!content || !editor || disabled) return false
          event.preventDefault()
          return editor.commands.insertContent(content)
        },
        handleDrop: (_view, event) => {
          const files = Array.from(event.dataTransfer?.files ?? [])
          if (!files.length) return false
          event.preventDefault()
          return receiveFiles(files)
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
        refreshSuggestions()
      },
      onSelectionUpdate: () => refreshSuggestions(),
      onBlur: () => closeSuggestions(),
    })
    editorText = serializeDocToText(editor.state.doc)
    return () => { alive = false; actionEpoch += 1; lookup.cancel(); editor?.destroy(); editor = null }
  })

  function syncValue(target: Editor, next: string, key: string) {
    const switched = loadedKey !== key
    if (!switched && target.view.composing) { deferredValue = true; return }
    deferredValue = false
    if (switched) {
      actionEpoch += 1
      loadedKey = key
      sending = false; cancelling = false; adding = false; failure = ''
      closeSuggestions()
      dismissedIdentity = ''
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

  export function focus() { editor?.commands.focus('end') }
  export function getText() { return editor ? serializeDocToText(editor.state.doc) : value }
  export function insertText(text: string): boolean {
    if (!editor || disabled) return false
    editor.commands.focus()
    return insertComposerText(editor, text)
  }
  export function insertQuote(text: string): boolean {
    if (!editor || disabled) return false
    const inserted = appendComposerQuote(editor, text)
    if (inserted) editor.commands.focus('end')
    return inserted
  }

  async function runAction(kind: 'submit' | 'cancel' | 'attachment', operation: () => void | Promise<void>) {
    if ((kind === 'submit' && sending) || (kind === 'cancel' && cancelling) || (kind === 'attachment' && adding)) return
    const epoch = actionEpoch
    const key = draftKey
    if (kind === 'submit') sending = true
    else if (kind === 'cancel') cancelling = true
    else adding = true
    failure = ''
    try { await operation() } catch {
      if (alive && epoch === actionEpoch && key === draftKey) failure = kind === 'submit' ? 'Could not send. Your draft is preserved.'
        : kind === 'cancel' ? 'Could not cancel the current turn.' : 'Could not add the attachment.'
    } finally {
      if (alive && epoch === actionEpoch && key === draftKey) {
        if (kind === 'submit') sending = false
        else if (kind === 'cancel') cancelling = false
        else adding = false
      }
    }
  }

  function submit() {
    if (disabled || sendDisabled || busy || sending || adding || !editor || editor.view.composing) return
    const text = serializeDocToText(editor.state.doc).trim()
    if (!text && attachments.length === 0) return
      // The host owns submit/restore draft transitions. Never erase newer typing here.
    void runAction('submit', () => onsubmit(text))
  }

  function receiveFiles(files: readonly File[]): boolean {
    if (disabled || adding) return true
    if (!onPasteFiles) { failure = 'Attachments are not supported by this session.'; return true }
    const receive = onPasteFiles
    void runAction('attachment', () => receive(files))
    return true
  }

  function handleKey(event: KeyboardEvent): boolean {
    if (!editor || disabled || isImeCompositionKey(event) || editor.view.composing) return false
    if (query) {
      if (event.key === 'Escape') { closeSuggestions(); return true }
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        if (suggestions.length) selectedSuggestion = (selectedSuggestion + (event.key === 'ArrowDown' ? 1 : -1) + suggestions.length) % suggestions.length
        return true
      }
      if ((event.key === 'Enter' && !event.shiftKey) || event.key === 'Tab') {
        const suggestion = suggestions[selectedSuggestion]
        if (suggestion && !searching) selectReference(suggestion)
        return true
      }
    }
    const action = decideComposerKey(event, { composing: editor.view.composing, inCodeBlock: false, inList: false }, {
      submit: submitShortcut, newline: submitShortcut === 'enter' ? 'shift+enter' : 'enter',
    })
    if (action === 'submit') { submit(); return true }
    if (action === 'newline') { editor.commands.setHardBreak(); return true }
    return false
  }

  function closeSuggestions() {
    dismissedIdentity = queryIdentity
    query = null; suggestions = []; searching = false; queryIdentity = ''
    lookup.cancel()
  }

  async function refreshSuggestions() {
    if (!editor || disabled || !referenceSearch || editor.view.composing) return
    const next = findReferenceQuery(editor.state)
    const identity = next ? `${draftKey}:${next.from}:${next.to}:${next.text}` : ''
    if (!next) { closeSuggestions(); return }
    if (identity === queryIdentity || identity === dismissedIdentity) return
    query = next; queryIdentity = identity; suggestions = []; selectedSuggestion = 0; searching = true; searchFailed = false
    try {
      const found = await lookup.search(referenceSearch, next.text)
      if (alive && found && queryIdentity === identity) { suggestions = found.slice(0, 20); searching = false }
    } catch { if (alive && queryIdentity === identity) { searching = false; searchFailed = true } }
  }

  function selectReference(reference: ComposerReference) {
    if (!editor || !query || disabled) return
    const range = { from: query.from, to: query.to }
    closeSuggestions()
    editor.chain().focus().insertContentAt(range, [{ type: 'reference', attrs: reference }, { type: 'text', text: ' ' }]).run()
  }

  function openReferenceSearch() {
    if (!editor || disabled || !referenceSearch) return
    const before = editor.state.selection.$from.nodeBefore
    const text = before?.isText ? before.text ?? '' : ''
    insertText(canStartMentionAt(text, text.length) ? '@' : ' @')
  }
</script>

<div class="agent-composer relative rounded-2xl border bg-background shadow-sm transition-colors focus-within:border-primary/40 focus-within:ring-2 focus-within:ring-primary/10" class:opacity-60={disabled}>
  {#if query}
    <div class="absolute inset-x-0 bottom-full z-20 mb-2 max-h-60 overflow-y-auto rounded-xl border bg-popover p-1.5 shadow-lg" role="listbox" aria-label={tr('Files')}>
      {#if searching || searchFailed || !suggestions.length}
        <p class="px-3 py-2 text-xs text-muted-foreground" role="status">{tr(searching ? 'Searching…' : searchFailed ? 'File search failed' : 'No matching files')}</p>
      {:else}
        {#each suggestions as suggestion, index (suggestion.id)}
          <button type="button" role="option" aria-selected={selectedSuggestion === index}
            class="block w-full rounded-lg px-3 py-2 text-left text-xs hover:bg-muted" class:bg-muted={selectedSuggestion === index}
            onmousedown={(event) => event.preventDefault()} onclick={() => selectReference(suggestion)}>
            <span class="block truncate font-medium">{suggestion.label}</span><span class="block truncate text-[10px] text-muted-foreground">{suggestion.uri}</span>
          </button>
        {/each}
      {/if}
    </div>
  {/if}
  <div bind:this={editorHost} class="min-h-24 px-4 pt-3"></div>
  {#if attachments.length}
    <div class="flex flex-wrap gap-1.5 px-3 pb-2">
      {#each attachments as attachment (attachment.id)}
        <span class="inline-flex max-w-full items-center gap-1 rounded-lg border bg-muted/40 py-1 pl-2 pr-1 text-xs" title={attachment.detail ?? attachment.name}>
          <Paperclip class="size-3 shrink-0" /><span class="truncate">{attachment.name}</span>
          {#if onRemoveAttachment}<button type="button" class="rounded p-1 hover:bg-muted" aria-label={`${tr('Remove attachment')}: ${attachment.name}`} disabled={disabled || adding}
            onclick={() => void runAction('attachment', () => onRemoveAttachment?.(attachment.id))}><X class="size-3" /></button>{/if}
        </span>
      {/each}
    </div>
  {/if}
  {#if failure}<p class="m-0 px-4 pb-2 text-xs text-destructive" role="alert">{tr(failure)}</p>{/if}
  <footer class="flex min-h-11 items-center gap-1.5 px-2.5 pb-2">
    {#if onAddAttachments}<Button variant="ghost" size="icon" class="size-8 rounded-lg" title={tr('Add attachment')} aria-label={tr('Add attachment')} disabled={disabled || adding}
      onclick={() => void runAction('attachment', () => onAddAttachments?.())}><Paperclip class="size-4" /></Button>{/if}
    {#if referenceSearch}<Button variant="ghost" size="icon" class="size-8 rounded-lg" title={tr('Mention a file')} aria-label={tr('Mention a file')} {disabled}
      onclick={openReferenceSearch}><AtSign class="size-4" /></Button>{/if}
    <slot name="footer" />
    <span class="ml-1 min-w-0 flex-1 text-[10px] leading-4 text-muted-foreground">{hint}</span>
    {#if busy && oncancel}
      <Button variant="secondary" size="icon" class="size-8 shrink-0 rounded-xl" aria-label={tr('Cancel current turn')} title={tr('Cancel current turn')} disabled={disabled || cancelling}
        onclick={() => void runAction('cancel', () => oncancel?.())}>{#if cancelling}<LoaderCircle class="size-4 animate-spin" />{:else}<Square class="size-3.5 fill-current" />{/if}</Button>
    {:else}
      <Button size="icon" class="size-8 shrink-0 rounded-xl" aria-label={tr('Send message')} title={tr('Send message')} disabled={!canSend} onclick={submit}>
        {#if sending}<LoaderCircle class="size-4 animate-spin" />{:else}<ArrowUp class="size-4" />{/if}
      </Button>
    {/if}
  </footer>
</div>

<style>
  .agent-composer :global(.ramble-composer-editor) { min-height: 5.25rem; max-height: 16rem; overflow-y: auto; outline: none; font-size: 0.875rem; line-height: 1.65; overflow-wrap: anywhere; white-space: pre-wrap; padding-bottom: 0.65rem; }
  .agent-composer :global(.ramble-composer-editor p) { margin: 0; }
  .agent-composer :global(.ramble-composer-empty::before) { color: var(--muted-foreground); content: attr(data-placeholder); float: left; height: 0; pointer-events: none; }
  .agent-composer :global(.ramble-composer-quote-marker) { color: transparent; border-left: 2px solid var(--muted-foreground); margin-left: 0.1em; }
  .agent-composer :global(.ramble-composer-inactive-selection) { background: color-mix(in oklch, var(--primary) 18%, transparent); }
  .agent-composer :global(.ramble-composer-reference) { display: inline; border-radius: 0.35rem; background: var(--muted); padding: 0.1rem 0.3rem; color: var(--foreground); font-size: 0.85em; white-space: normal; }
  .agent-composer :global(.ProseMirror-selectednode) { outline: 1px solid var(--primary); }
</style>
