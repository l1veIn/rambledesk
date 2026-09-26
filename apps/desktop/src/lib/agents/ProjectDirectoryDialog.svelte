<script lang="ts">
  import { ArrowUp, ChevronDown, ChevronRight, Folder, HardDrive, House, LoaderCircle } from '@lucide/svelte'
  import { onDestroy } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import { isProjectDirectoryError } from '$lib/application/contracts'
  import type { ProjectDirectoryListing } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'

  export let transport: ApplicationTransport
  export let value: string
  export let disabled = false
  export let onSelect: (path: string) => void

  let open = false
  let listing: ProjectDirectoryListing | null = null
  let pathInput = ''
  let loading = false
  let error = ''
  let generation = 0
  let title: HTMLElement | null = null
  $: projectName = value.replace(/[\\/]+$/u, '').split(/[\\/]/u).pop() || value
  $: canSelect = !disabled && !loading && !error && listing !== null && pathInput.trim() === listing.path
  $: if (disabled && open) changeOpen(false)
  const zh: Record<string, string> = {
    'Project directory': '项目目录', 'Choose a project': '选择项目',
    'Choose a project folder': '选择项目文件夹',
    'Browse folders on the device running RambleDesk.': '浏览运行 RambleDesk 的设备上的文件夹。',
    'Folder path': '文件夹路径', 'Go': '前往', 'Home directory': '主目录',
    'Parent directory': '上一级', 'Locations': '位置', 'Folders': '文件夹',
    'Loading folders…': '正在读取文件夹…', 'No subfolders. You can select this folder.': '没有子文件夹，可以直接选择当前文件夹。',
    'Select a location or enter a folder path.': '选择一个位置，或输入文件夹路径。',
    'Cancel': '取消', 'Use this folder': '使用此文件夹', 'Selected folder': '当前文件夹',
    'Retry': '重试', 'The folder was not found.': '找不到此文件夹。',
    'You do not have permission to open this folder.': '没有权限打开此文件夹。',
    'Enter an absolute path to a folder.': '请输入文件夹的绝对路径。',
    'Could not read this folder. Try again or choose another location.': '无法读取此文件夹，请重试或选择其他位置。',
  }
  const tr = (text: string) => $locale === 'zh-CN' ? zh[text] ?? text : text

  function changeOpen(next: boolean) {
    open = next && !disabled
    generation += 1
    if (open) {
      listing = null
      pathInput = value.trim()
      void browse(pathInput || null)
    }
  }

  async function browse(path: string | null) {
    const attempt = ++generation
    loading = true
    error = ''
    if (path !== null) pathInput = path
    try {
      const result = await transport.call('browseProjectDirectories', { path })
      if (!open || attempt !== generation) return
      listing = result
      pathInput = result.path
    } catch (cause) {
      if (!open || attempt !== generation) return
      const code = isProjectDirectoryError(cause) ? cause.code : null
      error = code === 'DIRECTORY_NOT_FOUND' ? 'The folder was not found.'
        : code === 'DIRECTORY_ACCESS_DENIED' ? 'You do not have permission to open this folder.'
        : code === 'INVALID_DIRECTORY_PATH' ? 'Enter an absolute path to a folder.'
        : 'Could not read this folder. Try again or choose another location.'
    } finally {
      if (open && attempt === generation) loading = false
    }
  }

  function select() {
    if (!canSelect || !listing) return
    const path = listing.path
    changeOpen(false)
    onSelect(path)
  }
  onDestroy(() => { generation += 1 })
</script>

<Dialog.Root {open} onOpenChange={changeOpen}>
  <Dialog.Trigger>
    {#snippet child({ props })}
      <Button {...props} variant="ghost" size="sm" class="min-w-0 max-w-[70%] justify-start gap-2 px-2 text-xs" {disabled}
        title={value || tr('Choose a project')} aria-label={`${tr('Project directory')}: ${value || tr('Choose a project')}`}>
        <Folder class="size-4 shrink-0" />
        <span class="truncate">{projectName || tr('Choose a project')}</span><ChevronDown class="size-3 shrink-0 text-muted-foreground" />
      </Button>
    {/snippet}
  </Dialog.Trigger>
  <Dialog.Content class="flex max-h-[85dvh] w-[calc(100vw-2rem)] max-w-2xl flex-col gap-4 sm:max-w-2xl" showCloseButton={false}
    onOpenAutoFocus={(event) => { event.preventDefault(); title?.focus({ preventScroll: true }) }}>
    <Dialog.Header>
      <Dialog.Title bind:ref={title} tabindex={-1} class="outline-none">{tr('Choose a project folder')}</Dialog.Title>
      <Dialog.Description>{tr('Browse folders on the device running RambleDesk.')}</Dialog.Description>
    </Dialog.Header>
    <form class="flex min-w-0 items-center gap-2" onsubmit={(event) => { event.preventDefault(); void browse(pathInput.trim() || null) }}>
      <Button type="button" variant="outline" size="icon" class="size-9 shrink-0" title={tr('Parent directory')} aria-label={tr('Parent directory')}
        disabled={!listing?.parent_path || loading} onclick={() => void browse(listing!.parent_path)}><ArrowUp class="size-4" /></Button>
      <input bind:value={pathInput} disabled={loading} aria-label={tr('Folder path')} autocomplete="off" spellcheck="false"
        class="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 font-mono text-xs outline-none focus:ring-2 focus:ring-ring" />
      <Button type="submit" variant="outline" size="sm" class="h-9" disabled={loading}>{tr('Go')}</Button>
    </form>
    <div class="flex min-h-0 flex-1 overflow-hidden rounded-xl border">
      <nav aria-label={tr('Locations')} class="w-28 shrink-0 space-y-1 overflow-y-auto border-r bg-muted/20 p-2 sm:w-36">
        <button type="button" class="flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-xs hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
          onclick={() => void browse(listing?.home_path ?? null)}><House class="size-4 shrink-0" /><span>{tr('Home directory')}</span></button>
        {#each listing?.roots ?? [] as root (root.path)}
          <button type="button" class="flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-xs hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
            title={root.path} onclick={() => void browse(root.path)}><HardDrive class="size-4 shrink-0" /><span class="truncate">{root.name}</span></button>
        {/each}
      </nav>
      <div class="h-[min(20rem,40dvh)] min-w-0 flex-1 overflow-y-auto p-2" aria-label={tr('Folders')} aria-busy={loading}>
        {#if loading}
          <p class="m-0 flex items-center justify-center gap-2 p-6 text-xs text-muted-foreground" role="status"><LoaderCircle class="size-4 animate-spin" />{tr('Loading folders…')}</p>
        {:else if error}
          <div class="space-y-3 p-4"><p class="m-0 text-sm text-destructive" role="alert">{tr(error)}</p><Button variant="outline" size="sm" onclick={() => void browse(pathInput.trim() || null)}>{tr('Retry')}</Button></div>
        {:else if listing}
          {#each listing.directories as directory (directory.path)}
            <button type="button" class="flex w-full items-center gap-2 rounded-md px-3 py-2.5 text-left text-sm hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              title={directory.path} onclick={() => void browse(directory.path)}><Folder class="size-4 shrink-0 text-muted-foreground" /><span class="min-w-0 flex-1 truncate">{directory.name}</span><ChevronRight class="size-3.5 shrink-0 text-muted-foreground" /></button>
          {:else}<p class="m-0 p-6 text-center text-xs leading-5 text-muted-foreground" role="status">{tr('No subfolders. You can select this folder.')}</p>{/each}
        {:else}<p class="m-0 p-6 text-xs text-muted-foreground">{tr('Select a location or enter a folder path.')}</p>{/if}
      </div>
    </div>
    <Dialog.Footer class="min-w-0 flex-row items-center gap-2">
      <span class="mr-auto min-w-0 flex-1 truncate text-left font-mono text-[11px] text-muted-foreground" title={listing?.path} aria-label={tr('Selected folder')}>{listing?.path ?? ''}</span>
      <Button variant="outline" onclick={() => changeOpen(false)}>{tr('Cancel')}</Button>
      <Button disabled={!canSelect} onclick={select}>{tr('Use this folder')}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
