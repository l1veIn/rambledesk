<!-- Per-reply changed-files card. Idea and layout adapted from Codeg
     src/components/message/reply-artifacts.tsx at 3ebdfed (Apache-2.0).
     RambleDesk reads durable ACP tool content and opens a workspace diff tab
     instead of an editor file preview. -->
<script lang="ts">
  import { ChevronRight, ExternalLink, FileDiff, FilePlus, FileX } from '@lucide/svelte'
  import { useWorkbenchCapabilities } from '$lib/capabilities/capabilityContext'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  import { directoryOf, displayPath, fileNameOf, type ChangedFile } from './changed-files'

  export let files: readonly ChangedFile[]
  export let cwd = ''
  export let onOpenDiff: (file: ChangedFile) => void = () => {}

  const capabilities = useWorkbenchCapabilities()
  let open = true
  let revealError = ''

  $: revealAvailable = capabilities.serverPaths.status.availability !== 'unavailable'
  $: entries = files.map((file) => {
    const path = displayPath(file.path, cwd)
    return { file, path, name: fileNameOf(path), directory: directoryOf(path) }
  })
  $: additions = files.reduce((total, file) => total + file.additions, 0)
  $: deletions = files.reduce((total, file) => total + file.deletions, 0)

  function reveal(file: ChangedFile) {
    revealError = ''
    void capabilities.serverPaths.implementation
      .reveal(file.path)
      .catch(() => { revealError = chatText($locale, 'Could not reveal the file.') })
  }
</script>

{#if files.length}
  <section class="mt-3 overflow-hidden rounded-lg border bg-card/40" data-turn-changed-files>
    <button
      type="button"
      class="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-accent/40"
      aria-expanded={open}
      onclick={() => (open = !open)}
    >
      <FileDiff class="size-4 shrink-0 text-muted-foreground" />
      <span class="text-xs font-medium">{chatText($locale, 'Changed files')}</span>
      <span class="rounded-md border bg-muted/40 px-1.5 py-0.5 text-[10px] tabular-nums text-muted-foreground">
        {files.length} {chatText($locale, 'files')}
      </span>
      <span class="inline-flex items-center gap-1.5 rounded-md border bg-muted/40 px-1.5 py-0.5 font-mono text-[10px]">
        <span class="text-emerald-600 dark:text-emerald-400">+{additions}</span>
        <span class="text-red-600 dark:text-red-400">−{deletions}</span>
      </span>
      <ChevronRight class={`ms-auto size-4 shrink-0 text-muted-foreground transition-transform ${open ? 'rotate-90' : ''}`} />
    </button>
    {#if open}
      <div class="@container max-h-80 overflow-y-auto border-t p-2">
        <div class="grid gap-2 @md:grid-cols-2">
          {#each entries as entry (entry.file.id)}
            {#if entry.file.kind === 'removed'}
              <!-- A removed file is gone from disk: nothing to open or reveal. -->
              <div class="flex items-center gap-2 overflow-hidden rounded-md border border-destructive/30 bg-destructive/5 px-2.5 py-2" title={entry.path}>
                <FileX class="size-4 shrink-0 text-destructive" />
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="truncate text-xs font-medium text-destructive">{entry.name}</span>
                  {#if entry.directory}<span class="truncate text-[10px] text-muted-foreground">{entry.directory}</span>{/if}
                </span>
                <span class="shrink-0 rounded-md border border-destructive/30 bg-destructive/10 px-1.5 py-0.5 font-mono text-[10px] text-destructive">{chatText($locale, 'Removed')}</span>
              </div>
            {:else}
              <div class={`flex items-stretch overflow-hidden rounded-md border transition-colors ${entry.file.kind === 'added' ? 'border-emerald-600/30 bg-emerald-500/5 hover:border-emerald-600/50 dark:border-emerald-400/30' : 'bg-muted/20 hover:bg-accent/40'}`}>
                <button
                  type="button"
                  class="flex min-w-0 flex-1 items-center gap-2 px-2.5 py-2 text-left"
                  title={`${entry.path} · ${chatText($locale, 'View changes')}`}
                  aria-label={`${chatText($locale, 'View changes')}: ${entry.path}`}
                  onclick={() => onOpenDiff(entry.file)}
                >
                  {#if entry.file.kind === 'added'}<FilePlus class="size-4 shrink-0 text-muted-foreground" />
                  {:else}<FileDiff class="size-4 shrink-0 text-muted-foreground" />{/if}
                  <span class="flex min-w-0 flex-1 flex-col">
                    <span class="truncate text-xs font-medium">{entry.name}</span>
                    {#if entry.directory}<span class="truncate text-[10px] text-muted-foreground">{entry.directory}</span>{/if}
                  </span>
                  {#if entry.file.additions || entry.file.deletions}
                    <span class="shrink-0 font-mono text-[10px]">
                      <span class="text-emerald-600 dark:text-emerald-400">+{entry.file.additions}</span>
                      <span class="text-red-600 dark:text-red-400">−{entry.file.deletions}</span>
                    </span>
                  {/if}
                </button>
                {#if revealAvailable}
                  <button
                    type="button"
                    class="flex w-9 shrink-0 items-center justify-center border-l text-muted-foreground hover:bg-accent/60 hover:text-foreground"
                    aria-label={`${chatText($locale, 'Reveal in file manager')}: ${entry.path}`}
                    title={chatText($locale, 'Reveal in file manager')}
                    onclick={() => reveal(entry.file)}
                  >
                    <ExternalLink class="size-3.5" />
                  </button>
                {/if}
              </div>
            {/if}
          {/each}
        </div>
      </div>
    {/if}
    {#if revealError}<p role="alert" class="m-0 border-t px-3 py-1.5 text-[10px] text-destructive">{revealError}</p>{/if}
  </section>
{/if}
