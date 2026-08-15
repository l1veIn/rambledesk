<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { CheckCircle2, LoaderCircle, RefreshCw, Settings2, ShieldAlert } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { toast } from '$lib/components/ui/sonner'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  type MacPermissionStatus = 'granted' | 'denied' | 'not_determined' | 'unknown'
  type MacPermission = { id: string; status: MacPermissionStatus }

  let permissions: MacPermission[] = []
  let loading = true
  let busy = false

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

  function title(id: string) {
    return id === 'screen_capture' ? tr('屏幕与系统音频录制') : tr('麦克风')
  }

  function description(id: string) {
    return id === 'screen_capture'
      ? tr('屏幕录制权限用于截图和滚动截图。')
      : tr('麦克风权限用于语音 Ramble 的本地转录。')
  }

  function statusBadge(status: MacPermissionStatus) {
    if (status === 'granted') return { label: tr('已授权'), variant: 'secondary' as const }
    if (status === 'denied') return { label: tr('已拒绝'), variant: 'destructive' as const }
    if (status === 'unknown') return { label: tr('未知'), variant: 'outline' as const }
    return { label: tr('未授权'), variant: 'outline' as const }
  }

  function updateStatus(id: string, status: MacPermissionStatus) {
    permissions = permissions.map((permission) =>
      permission.id === id ? { ...permission, status } : permission,
    )
  }

  async function load() {
    loading = true
    try {
      permissions = await invoke<MacPermission[]>('list_macos_permissions')
    } catch (cause) {
      toast.error(tr('无法读取权限状态'), { description: messageFrom(cause) })
    } finally {
      loading = false
    }
  }

  async function request(id: string) {
    if (busy) return
    busy = true
    try {
      const next = await invoke<MacPermission>('request_macos_permission', { permission: id })
      updateStatus(id, next.status)
      if (next.status === 'granted') {
        toast.success(tr('权限已开启'))
      } else {
        toast.error(tr('权限未开启，请打开系统设置手动允许。'))
      }
    } catch (cause) {
      toast.error(tr('无法请求权限'), { description: messageFrom(cause) })
    } finally {
      busy = false
    }
  }

  async function openSettings(id: string) {
    try {
      await invoke('open_macos_privacy_settings', { permission: id })
      toast.info(tr('请在系统设置中允许 RambleDesk，然后回来刷新状态。'))
    } catch (cause) {
      toast.error(tr('无法打开系统设置'), { description: messageFrom(cause) })
    }
  }

  onMount(() => {
    void load()
  })
</script>

<div class="space-y-3">
  {#if loading}
    <div class="flex items-center gap-2 py-6 text-xs text-muted-foreground">
      <LoaderCircle class="size-4 animate-spin" />
      {tr('正在读取权限状态…')}
    </div>
  {:else if permissions.length === 0}
    <p class="m-0 py-6 text-xs text-muted-foreground">{tr('当前平台无需额外权限。')}</p>
  {:else}
    {#each permissions as permission (permission.id)}
      <div class="flex flex-wrap items-center justify-between gap-4 rounded-lg border bg-muted/20 px-4 py-3">
        <div class="flex min-w-0 gap-3">
          <ShieldAlert class="mt-0.5 size-5 shrink-0 text-muted-foreground" />
          <div class="min-w-0">
            <strong class="block text-xs">{title(permission.id)}</strong>
            <p class="mb-0 mt-1 text-[10px] leading-4 text-muted-foreground">
              {description(permission.id)}
            </p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <Badge variant={statusBadge(permission.status).variant}>
            {#if permission.status === 'granted'}<CheckCircle2 data-icon="inline-start" />{/if}
            {statusBadge(permission.status).label}
          </Badge>
          {#if permission.status !== 'granted'}
            {#if permission.status === 'not_determined'}
              <Button size="sm" disabled={busy} onclick={() => void request(permission.id)}>
                {#if busy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{/if}
                {tr('去授权')}
              </Button>
              <Button size="sm" variant="outline" onclick={() => void openSettings(permission.id)}>
                <Settings2 data-icon="inline-start" />
                {tr('打开系统设置')}
              </Button>
            {:else}
              <Button size="sm" variant="outline" onclick={() => void openSettings(permission.id)}>
                <Settings2 data-icon="inline-start" />
                {tr('打开系统设置')}
              </Button>
            {/if}
          {/if}
        </div>
      </div>
    {/each}
    <div class="flex items-center justify-between gap-3 rounded-md border bg-muted/20 px-3 py-2">
      <p class="m-0 text-[10px] leading-4 text-muted-foreground">
        {tr('授权后如果仍无法使用，请重启 RambleDesk。')}
      </p>
      <Button variant="ghost" size="sm" disabled={loading} onclick={() => void load()}>
        <RefreshCw data-icon="inline-start" />
        {tr('刷新权限状态')}
      </Button>
    </div>
  {/if}
</div>
