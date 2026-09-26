<script lang="ts">
  import type { Editor, EditorOptions } from '@tiptap/core'
  import { onDestroy } from 'svelte'
  import TiptapInput from '../editor/TiptapInput.svelte'
  import { fieldDocumentText, fieldTextDocument, fieldTextExtensions, fieldTextPosition } from '../editor/fieldTextDocument'
  import { rebaseFieldTextEdit, syncFieldText } from '../editor/fieldTextEditing'
  import InputToolbar from './InputToolbar.svelte'
  import FieldAttachments from './FieldAttachments.svelte'
  import { fieldAttachmentText, mapFieldAttachmentTextRanges, replaceFieldAttachmentText } from './fieldAttachmentText'
  import { unavailableInputToolsState, useInputTools } from './inputToolsContext'
  import SpeechOriginBadge from '../speech/SpeechOriginBadge.svelte'
  import { fieldSpeechSegmentsForTarget } from '../speech/fieldSpeechSegments'
  import { FieldSpeechDecorations, setFieldSpeechSegments } from '../speech/fieldSpeechDecorations'
  import type { InputTarget } from '../domain/inputTarget'
  import { unavailableVoiceInputState, useVoiceInput } from '../speech/voiceInputContext'

  export let value: string
  export let label: string
  export let voiceLabel: string
  export let target: InputTarget | null
  export let maxLength: number
  export let disabled = false
  export let placeholder = ''
  export let tone: 'default' | 'suggestion' = 'default'
  export let editorClass = 'px-3 py-2 leading-6'
  export let focusOnMount = false
  export let onChange: (value: string) => void

  const voice = useVoiceInput()
  const voiceState = voice?.state ?? unavailableVoiceInputState
  const toolsState = useInputTools()?.state ?? unavailableInputToolsState
  let editor: Editor | null = null
  let compositionTimer: ReturnType<typeof setTimeout> | undefined
  let speechSignature = ''
  const options: Partial<EditorOptions> = {
    extensions: [...fieldTextExtensions(), FieldSpeechDecorations],
    content: fieldTextDocument(fieldAttachmentText(value, $toolsState.attachments).text),
    editorProps: {
      clipboardTextSerializer: (slice) => slice.content.textBetween(0, slice.content.size, '\n', '\n'),
      handleClick: () => { selectTarget(); return false },
      handleDOMEvents: {
        compositionend: () => {
          clearTimeout(compositionTimer)
          compositionTimer = setTimeout(syncText, 0)
          return false
        },
      },
    },
    onUpdate: ({ editor: source, transaction }) => {
      if (disabled) return
      const before = fieldDocumentText(transaction.before.toJSON())
      const edited = fieldDocumentText(source.getJSON())
      const latest = fieldAttachmentText(value, $toolsState.attachments).text
      const merged = rebaseFieldTextEdit(before, edited, latest)
      const next = replaceFieldAttachmentText(value, merged, $toolsState.attachments, maxLength)
      if (next !== value) { value = next; onChange(next) }
      syncText()
    },
  }

  // Echoed keystrokes are no-ops; speech/tidy changes patch only the changed span.
  $: if (editor) { value; $toolsState.attachments; syncText() }
  $: if (editor) { value; target; $voiceState; $toolsState.attachments; syncSpeech() }
  onDestroy(() => clearTimeout(compositionTimer))

  function selectTarget() {
    if (!disabled && !$voiceState.disabled && target) voice?.selectTarget(target)
  }

  function syncText() {
    if (!editor || editor.isDestroyed || editor.view.composing) return
    syncFieldText(editor, fieldAttachmentText(value, $toolsState.attachments).text)
    syncSpeech()
  }

  function syncSpeech() {
    if (!editor || editor.isDestroyed || editor.view.composing) return
    const snapshot = $voiceState.draftSnapshot
    const segments = snapshot && target?.requestId === $voiceState.requestId
      ? fieldSpeechSegmentsForTarget(snapshot, target).filter((segment) =>
        [...value].slice(segment.start, segment.end).join('') === segment.text) : []
    const ranges = mapFieldAttachmentTextRanges(value, $toolsState.attachments, segments).map((segment) => ({
      segmentId: segment.segmentId,
      from: fieldTextPosition(editor!.state.doc, segment.start),
      to: fieldTextPosition(editor!.state.doc, segment.end),
      state: segment.state === 'cleaned' ? 'cleaned' as const : 'pending' as const,
    }))
    const signature = JSON.stringify(ranges)
    if (speechSignature !== signature) { speechSignature = signature; setFieldSpeechSegments(editor, ranges) }
  }
</script>

<div class="grid gap-1.5 text-xs">
  <div class="flex flex-wrap items-center gap-2">
    <span class="mr-auto">{label}</span>
    <SpeechOriginBadge {target} />
  </div>
  <div class={`overflow-hidden rounded-lg border focus-within:border-ring ${tone === 'suggestion' ? 'border-emerald-600/25 bg-emerald-500/5' : 'bg-background'}`}>
    <div class={`border-b px-2 py-1.5 ${tone === 'suggestion' ? 'border-emerald-600/15' : 'bg-muted/30'}`}>
      <InputToolbar {target} {disabled} label={voiceLabel} />
    </div>
    <div class="field-editor" class:opacity-70={disabled}>
      <TiptapInput bind:editor {options} {disabled} {label} {placeholder} {focusOnMount}
        onFocus={selectTarget} attributes={$$restProps} contentClass={`field-prose ${editorClass}`} />
    </div>
    <FieldAttachments {value} {disabled} {onChange} />
  </div>
</div>

<style>
  .field-editor :global(.field-prose) { min-height: 90px; font-size: 14px; line-height: 1.78; }
  .field-editor :global(.field-prose p) { margin: 0; }
</style>
