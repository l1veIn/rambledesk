<script lang="ts">
  import { ArchiveRestore, LoaderCircle, RefreshCw, X } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { MissingSessionViewDescriptor } from './sessionViewRecovery'

  export let missing: MissingSessionViewDescriptor
  export let label: string
  export let busy = false
  export let onRetry: () => Promise<void> | void = () => {}
  export let onClose: () => Promise<void> | void = () => {}
  export let onOpenArchive: () => void = () => {}

  const tr = (source: string) => t($locale, source)

  $: archived = missing.reason === 'archived'
  $: unresolved = missing.reason === 'unresolved'
  $: description = archived
    ? tr('This session is archived. Its saved requests remain available in Archived content.')
    : missing.reason === 'unavailable'
      ? tr('This session no longer exists or is unavailable. The local tab can be closed safely.')
      : unresolved
        ? tr('Checking whether this session is still available…')
        : tr('RambleDesk could not verify this session. Retry when the data source is available.')
</script>

<section
  class="grid h-full min-h-0 place-items-center overflow-auto bg-background px-6 py-10"
  aria-labelledby="missing-session-title"
  aria-busy={unresolved || busy}
>
  <div class="min-w-0 w-full max-w-lg overflow-hidden rounded-2xl border border-border/70 bg-card/70 p-6 shadow-sm">
    <div class="flex items-start gap-4">
      <div class="grid size-11 shrink-0 place-items-center rounded-xl bg-muted text-muted-foreground">
        {#if unresolved || busy}
          <LoaderCircle class="size-5 animate-spin" aria-hidden="true" />
        {:else if archived}
          <ArchiveRestore class="size-5" aria-hidden="true" />
        {:else}
          <RefreshCw class="size-5" aria-hidden="true" />
        {/if}
      </div>
      <div class="min-w-0 flex-1">
        <p class="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
          {tr('Session recovery')}
        </p>
        <h2
          id="missing-session-title"
          class="mt-1 truncate text-lg font-semibold text-foreground"
          title={label}
        >
          {label}
        </h2>
        <p class="mt-2 text-sm leading-6 text-muted-foreground">{description}</p>
        <p class="mt-3 break-all font-mono text-xs text-muted-foreground/80">
          {missing.session.hostId} · {missing.session.hostSessionId}
        </p>
      </div>
    </div>

    <div class="mt-6 flex flex-wrap justify-end gap-2">
      {#if archived}
        <Button variant="outline" onclick={onOpenArchive}>
          <ArchiveRestore class="size-4" aria-hidden="true" />
          {tr('View archived content')}
        </Button>
      {:else if !unresolved}
        <Button variant="outline" disabled={busy} onclick={() => void onRetry()}>
          <RefreshCw class={['size-4', busy && 'animate-spin']} aria-hidden="true" />
          {tr('Retry')}
        </Button>
      {/if}
      <Button onclick={() => void onClose()}>
        <X class="size-4" aria-hidden="true" />
        {tr('Close session tab')}
      </Button>
    </div>
  </div>
</section>
