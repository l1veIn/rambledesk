<script lang="ts">
  import { Keyboard } from '@lucide/svelte'
  import { onDestroy, onMount } from 'svelte'

  import { Button } from '$lib/components/ui/button'
  import { toast } from '$lib/components/ui/sonner'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    comboFromEvent,
    comboParts,
    isAcceptableCombo,
    refreshShortcutSettings,
    resetShortcuts,
    setLocalCaptureActive,
    setShortcutCaptureActive,
    shortcutSettings,
    updateShortcut,
    type ShortcutAction,
  } from '$lib/shortcutSettings'

  const isTauri = '__TAURI_INTERNALS__' in window

  let capturing: ShortcutAction | null = null
  let draft = ''
  let draftError = ''
  let saving = false
  let resetting = false

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return String(cause)
  }

  const actions: { id: ShortcutAction; icon: typeof Keyboard; title: string; description: string }[] = [
    {
      id: 'rambleToggle',
      icon: Keyboard,
      title: tr('Voice toggle'),
      description: tr(
        'Start or stop a voice Ramble from anywhere with a global shortcut.',
      ),
    },
    {
      id: 'screenCapture',
      icon: Keyboard,
      title: tr('Screen capture'),
      description: tr(
        'Open the screenshot editor on the current monitor from anywhere with a global shortcut.',
      ),
    },
  ]

  function combo(action: ShortcutAction): string {
    return $shortcutSettings[action] ?? ''
  }

  function beginCapture(action: ShortcutAction) {
    if (!isTauri || capturing) return
    capturing = action
    draft = ''
    draftError = ''
    setLocalCaptureActive(true)
    void setShortcutCaptureActive(true).catch(() => {})
  }

  function cancelCapture() {
    capturing = null
    draft = ''
    draftError = ''
    setLocalCaptureActive(false)
    void setShortcutCaptureActive(false).catch(() => {})
  }

  async function confirmCapture() {
    if (!capturing || saving || !draft) return
    const action = capturing
    if (!isAcceptableCombo(draft)) {
      draftError = tr(
        'A shortcut needs at least one modifier (Ctrl / Cmd / Alt / Shift), or a function key F1–F24.',
      )
      return
    }
    saving = true
    draftError = ''
    try {
      await setShortcutCaptureActive(false)
      await updateShortcut(action, draft)
      capturing = null
      draft = ''
      setLocalCaptureActive(false)
      toast.success(tr('Shortcut saved and active.'))
    } catch (cause) {
      draftError = messageFrom(cause)
    } finally {
      saving = false
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (!capturing) return
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopImmediatePropagation()
      cancelCapture()
      return
    }
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) {
      // Avoid stray focus changes while the user is holding modifiers.
      event.preventDefault()
      return
    }
    event.preventDefault()
    event.stopImmediatePropagation()
    if (event.repeat) return
    const next = comboFromEvent(event)
    if (next) {
      draft = next
      draftError = ''
    } else {
      draftError = tr('This key cannot be used as a global shortcut.')
    }
  }

  async function restoreDefaults() {
    if (resetting || !isTauri || capturing) return
    resetting = true
    try {
      await resetShortcuts()
      toast.success(tr('Shortcuts restored to defaults.'))
    } catch (cause) {
      toast.error(tr('Could not restore shortcut defaults.'), {
        description: messageFrom(cause),
      })
    } finally {
      resetting = false
    }
  }

  onMount(() => {
    if (!isTauri) return
    void refreshShortcutSettings()
    window.addEventListener('keydown', onKeydown, { capture: true })
    return () => {
      window.removeEventListener('keydown', onKeydown, { capture: true })
      if (capturing) {
        setLocalCaptureActive(false)
        void setShortcutCaptureActive(false)
      }
    }
  })

  function chips(comboValue: string) {
    return comboParts(comboValue)
  }
</script>

{#if !isTauri}
  <p class="m-0 text-xs text-muted-foreground">
    {tr('Global shortcuts are available only in the desktop app.')}
  </p>
{:else}
  <section class="flex items-center justify-end">
    <Button
      variant="outline"
      size="sm"
      disabled={resetting || !!capturing}
      onclick={() => void restoreDefaults()}
    >
      {tr('Restore defaults')}
    </Button>
  </section>

  {#each actions as action (action.id)}
    <section class="border-b pb-8">
      <div class="flex items-start justify-between gap-8">
        <div class="flex gap-3">
          <span
            class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground"
          >
            <action.icon class="size-4" />
          </span>
          <div>
            <h3 class="m-0 text-sm font-medium">{action.title}</h3>
            <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{action.description}</p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          {#each chips(combo(action.id)) as part (part)}
            <kbd
              class="rounded-md border bg-muted/40 px-1.5 py-0.5 font-mono text-[10px] text-foreground"
            >{part}</kbd>
          {/each}
          {#if capturing !== action.id}
            <Button variant="outline" size="sm" onclick={() => beginCapture(action.id)}>
              {tr('Change…')}
            </Button>
          {/if}
        </div>
      </div>

      {#if capturing === action.id}
        <div class="ml-11 mt-4 rounded-md border bg-muted/20 p-4">
          <p class="m-0 text-xs text-muted-foreground">
            {tr('Press the new shortcut combination.')}
          </p>
          <div class="mt-3 flex flex-wrap items-center gap-2">
            {#if draft}
              {#each chips(draft) as part (part)}
                <kbd
                  class="rounded-md border bg-background px-2 py-1 font-mono text-xs text-foreground"
                >{part}</kbd>
              {/each}
            {:else}
              <span class="px-1 text-xs text-muted-foreground">…</span>
            {/if}
            <span class="flex-1"></span>
            <Button
              size="sm"
              disabled={!draft || saving}
              onclick={() => void confirmCapture()}
            >
              {tr('Apply')}
            </Button>
            <Button variant="outline" size="sm" onclick={cancelCapture}>
              {tr('Cancel')}
            </Button>
          </div>
          {#if draftError}
            <p class="m-0 mt-2 text-xs text-destructive">{draftError}</p>
          {/if}
        </div>
      {/if}
    </section>
  {/each}
{/if}
