<script lang="ts">
  import { AlertTriangle, RefreshCw, RotateCw, Settings } from '@lucide/svelte'
  import AboutSettings from '$lib/AboutSettings.svelte'
  import { Button } from '$lib/components/ui/button'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import { locale } from '$lib/preferences'

  export let capabilities: WorkbenchCapabilities
  export let message: string
  export let timedOut = false
  export let settingsOpen = false
  export let onRetry: () => void
  export let onReload: () => void = () => window.location.reload()

  $: zh = $locale === 'zh-CN'
</script>

<section class="min-h-0 min-w-0 flex-1 overflow-y-auto p-6" data-startup-recovery>
  <div class="mx-auto max-w-3xl space-y-5">
    <div class="rounded-xl border border-destructive/30 bg-destructive/5 p-5" role="alert">
      <div class="flex items-start gap-3">
        <AlertTriangle class="mt-0.5 size-5 shrink-0 text-destructive" />
        <div class="min-w-0 flex-1 space-y-3">
          <h1 class="text-lg font-semibold">
            {timedOut
              ? (zh ? '加载超时' : 'Loading timed out')
              : (zh ? '无法加载 RambleDesk' : 'RambleDesk could not load')}
          </h1>
          <p class="text-sm leading-6 text-muted-foreground">
            {timedOut
              ? (zh ? '应用未能及时读取会话数据。你可以重试，或打开设置导出诊断包。' : 'The app did not finish reading session data in time. Retry, or open settings to export a diagnostic package.')
              : (zh ? '启动时发生了错误。请保留下面的错误信息；你可以重试，或打开设置导出诊断包。' : 'An error occurred during startup. Keep the error details below; you can retry or open settings to export a diagnostic package.')}
          </p>
          <pre class="max-h-48 select-text overflow-auto whitespace-pre-wrap break-words rounded-md border bg-background p-3 text-xs" aria-label={zh ? '错误详情' : 'Error details'}>{message}</pre>
          <div class="flex flex-wrap gap-2">
            <Button onclick={onRetry}><RefreshCw class="size-4" />{zh ? '重试加载' : 'Retry loading'}</Button>
            <Button variant="outline" onclick={onReload}><RotateCw class="size-4" />{zh ? '重新加载应用' : 'Reload app'}</Button>
            <Button variant="outline" onclick={() => settingsOpen = !settingsOpen}>
              <Settings class="size-4" />{zh ? '设置与诊断' : 'Settings and diagnostics'}
            </Button>
          </div>
        </div>
      </div>
    </div>
    {#if settingsOpen}
      <AboutSettings
        softwareUpdates={capabilities.softwareUpdates}
        diagnostics={capabilities.diagnostics}
        serverPaths={capabilities.serverPaths}
        externalLinks={capabilities.externalLinks}
        windowControls={capabilities.windowControls}
      />
    {/if}
  </div>
</section>
