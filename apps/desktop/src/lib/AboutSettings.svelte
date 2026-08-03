<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { Download, ExternalLink, GitBranch, LoaderCircle, RefreshCw, RotateCw, ShieldCheck } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import rambelleSticker from '../assets/rambelle-states/idle.png'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    checkForUpdates,
    downloadAndInstallUpdate,
    restartAfterUpdate,
    updateState,
  } from '$lib/updater'

  export let installBlocked = false

  let version = '0.0.1'
  const isTauri = '__TAURI_INTERNALS__' in window
  const projectUrl = 'https://github.com/l1veIn/rambledesk'

  onMount(async () => {
    if (isTauri) version = await getVersion().catch(() => version)
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function openProject() {
    if (isTauri) {
      await openUrl(projectUrl)
      return
    }
    window.open(projectUrl, '_blank', 'noopener,noreferrer')
  }

  $: progress =
    $updateState.total > 0
      ? Math.min(100, Math.round(($updateState.downloaded / $updateState.total) * 100))
      : 0
</script>

<div class="space-y-6">
  <section class="relative overflow-hidden rounded-xl border bg-gradient-to-br from-primary/8 via-background to-info/8 p-6">
    <div class="relative z-10 grid grid-cols-[minmax(0,1fr)_150px] items-center gap-6">
      <div>
        <div class="flex flex-wrap items-center gap-2">
          <h3 class="m-0 text-xl font-semibold tracking-tight">RambleDesk</h3>
          <Badge variant="secondary">v{version}</Badge>
          <Badge variant="outline">Windows</Badge>
        </div>
        <p class="m-0 mt-3 max-w-xl text-sm leading-6 text-muted-foreground">
          {tr('让 Agent 在关键节点停下来，向人类请求可恢复、可归档的结构化反馈。')}
        </p>
        <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">
          {tr('反馈草稿、附件与反馈包保存在你的设备上；Agent 只会收到你明确提交或取消的结果。')}
        </p>
        <Button variant="link" class="mt-3 h-auto gap-1.5 p-0 text-xs" onclick={() => void openProject()}>
          <GitBranch data-icon="inline-start" />
          {tr('查看 GitHub 仓库')}
          <ExternalLink class="size-3" />
        </Button>
      </div>
      <img
        src={rambelleSticker}
        alt={tr('Rambelle 挥手贴纸')}
        class="mx-auto h-36 w-36 object-contain drop-shadow-[0_16px_30px_rgba(59,130,246,0.2)]"
      />
    </div>
  </section>

  <section class="rounded-xl border p-5">
    <div class="flex items-start justify-between gap-6">
      <div>
        <h3 class="m-0 text-sm font-medium">{tr('软件更新')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('RambleDesk 会在启动后静默检查更新，你也可以随时手动检查。')}
        </p>
      </div>
      <Button
        variant="outline"
        disabled={!isTauri || $updateState.status === 'checking' || $updateState.status === 'downloading'}
        onclick={() => void checkForUpdates()}
      >
        {#if $updateState.status === 'checking'}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
          {tr('正在检查…')}
        {:else}
          <RefreshCw data-icon="inline-start" />
          {tr('检查更新')}
        {/if}
      </Button>
    </div>

    <div class="mt-4 rounded-lg border bg-muted/25 p-4" aria-live="polite">
      {#if $updateState.status === 'idle'}
        <p class="m-0 text-xs text-muted-foreground">{tr('尚未检查更新。')}</p>
      {:else if $updateState.status === 'checking'}
        <p class="m-0 text-xs text-muted-foreground">{tr('正在连接更新服务器…')}</p>
      {:else if $updateState.status === 'up-to-date'}
        <div class="flex items-center gap-2 text-xs text-success">
          <ShieldCheck class="size-4" />
          {tr('当前已是最新版本。')}
        </div>
      {:else if $updateState.status === 'available'}
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div>
            <strong class="block text-xs">{tr('发现新版本 v{version}', { version: $updateState.version })}</strong>
            {#if $updateState.message}
              <p class="m-0 mt-1 line-clamp-3 text-[10px] leading-4 text-muted-foreground">{$updateState.message}</p>
            {/if}
          </div>
          <Button
            disabled={installBlocked}
            title={installBlocked ? tr('请先完成或取消当前反馈，再安装更新。') : ''}
            onclick={() => void downloadAndInstallUpdate()}
          >
            <Download data-icon="inline-start" />
            {tr('下载并安装')}
          </Button>
        </div>
      {:else if $updateState.status === 'downloading'}
        <div>
          <div class="flex items-center justify-between gap-3 text-xs">
            <span>{tr('正在下载 v{version}…', { version: $updateState.version })}</span>
            {#if $updateState.total > 0}<span>{progress}%</span>{/if}
          </div>
          <div class="mt-3 h-2 overflow-hidden rounded-full bg-muted">
            <div
              class={['h-full bg-primary transition-[width]', $updateState.total <= 0 ? 'animate-pulse' : '']}
              style={`width: ${$updateState.total > 0 ? progress : 35}%`}
            ></div>
          </div>
        </div>
      {:else if $updateState.status === 'ready'}
        <div class="flex flex-wrap items-center justify-between gap-3">
          <strong class="text-xs">{tr('v{version} 已安装，重启后生效。', { version: $updateState.version })}</strong>
          <Button
            disabled={installBlocked}
            title={installBlocked ? tr('请先完成或取消当前反馈，再重启。') : ''}
            onclick={() => void restartAfterUpdate()}
          >
            <RotateCw data-icon="inline-start" />
            {tr('立即重启')}
          </Button>
        </div>
      {:else if $updateState.status === 'error'}
        <div>
          <strong class="block text-xs text-destructive">{tr('检查或安装更新失败')}</strong>
          <p class="m-0 mt-1 break-all text-[10px] leading-4 text-muted-foreground">{$updateState.message}</p>
        </div>
      {/if}
    </div>

    {#if installBlocked && ($updateState.status === 'available' || $updateState.status === 'ready')}
      <p class="m-0 mt-3 text-[10px] leading-4 text-warning-foreground dark:text-warning">
        {tr('当前存在进行中的反馈或未保存内容。为避免丢失数据，更新重启暂时不可用。')}
      </p>
    {/if}
  </section>

  <p class="m-0 text-center text-[10px] text-muted-foreground">
    © 2026 RambleDesk · MIT · {tr('第三方组件声明见 THIRD_PARTY_NOTICES.md')}
  </p>
</div>
