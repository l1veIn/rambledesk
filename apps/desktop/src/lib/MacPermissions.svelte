<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { CheckCircle2, LoaderCircle, RefreshCw, Rocket, Settings2, ShieldAlert } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { toast } from '$lib/components/ui/sonner'
  import { t } from '$lib/i18n'
  import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification'
  import { locale, setNotificationPopupEnabled } from '$lib/preferences'

  type MacPermissionStatus = 'granted' | 'denied' | 'not_determined' | 'unknown'
  type MacPermission = { id: string; status: MacPermissionStatus; restart_required: boolean }

  export let restartRequired = false

  let permissions: MacPermission[] = []
  let loading = true
  let busy = false
  let restarting = false
  let screenRestartRequired = false
  let loadGeneration = 0
  const isTauri = '__TAURI_INTERNALS__' in window

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
    if (id === 'screen_capture') return tr('Screen & System Audio Recording')
    if (id === 'notifications') return tr('System notifications')
    return tr('Microphone')
  }

  function description(id: string) {
    if (id === 'screen_capture') {
      return tr('Screen Recording is used for screenshots and scrolling captures.')
    }
    if (id === 'notifications') {
      return tr(
        'System notifications tell you when a new request arrives. Allow banners in System Settings if the prompt is dismissed.',
      )
    }
    return tr('The microphone is used for on-device transcription of voice Rambles.')
  }

  async function notificationPermission(): Promise<MacPermission> {
    if (!isTauri) return { id: 'notifications', status: 'unknown', restart_required: false }
    try {
      const granted = await isPermissionGranted()
      return { id: 'notifications', status: granted ? 'granted' : 'not_determined', restart_required: false }
    } catch {
      return { id: 'notifications', status: 'unknown', restart_required: false }
    }
  }

  function statusBadge(permission: MacPermission) {
    if (permission.restart_required) {
      return { label: tr('Granted — restart required'), variant: 'outline' as const }
    }
    if (permission.status === 'granted') return { label: tr('Granted'), variant: 'secondary' as const }
    if (permission.status === 'denied') return { label: tr('Denied'), variant: 'destructive' as const }
    if (permission.status === 'unknown') return { label: tr('Unknown'), variant: 'outline' as const }
    return { label: tr('Not granted'), variant: 'outline' as const }
  }

  function syncRestartRequired(next: MacPermission[]) {
    const screenCapture = next.find((permission) => permission.id === 'screen_capture')
    if (screenCapture?.status === 'granted' && !screenCapture.restart_required) {
      screenRestartRequired = false
    } else if (screenCapture?.restart_required) {
      screenRestartRequired = true
    }
    permissions = next.map((permission) =>
      permission.id === 'screen_capture' && screenRestartRequired
        ? { ...permission, status: 'granted', restart_required: true }
        : permission,
    )
    restartRequired = permissions.some((permission) => permission.restart_required)
  }

  async function load(showLoading = permissions.length === 0) {
    if (busy || restarting) return
    const generation = ++loadGeneration
    if (showLoading) loading = true
    if (!isTauri) {
      permissions = []
      loading = false
      restartRequired = false
      return
    }
    try {
      const next = await invoke<MacPermission[]>('list_macos_permissions')
      const withNotifications = [...next, await notificationPermission()]
      if (generation === loadGeneration) syncRestartRequired(withNotifications)
    } catch (cause) {
      if (showLoading && generation === loadGeneration) {
        toast.error(tr('Could not read permission status'), { description: messageFrom(cause) })
      }
    } finally {
      if (generation === loadGeneration) loading = false
    }
  }

  async function request(id: string) {
    if (busy) return
    loadGeneration += 1
    busy = true
    try {
      if (id === 'notifications') {
        const permission = (await isPermissionGranted()) ? 'granted' : await requestPermission()
        const next: MacPermission = {
          id: 'notifications',
          status: permission === 'granted' ? 'granted' : 'denied',
          restart_required: false,
        }
        syncRestartRequired(permissions.map((item) => (item.id === id ? next : item)))
        if (next.status === 'granted') {
          setNotificationPopupEnabled(true)
          toast.success(tr('Permission granted'))
        } else {
          toast.error(tr('Permission was not granted. Open System Settings and allow it manually.'), {
            description: tr(
              'The operating system did not grant notification permission. Open System Settings → Notifications → RambleDesk and allow banners.',
            ),
          })
        }
        return
      }
      const next = await invoke<MacPermission>('request_macos_permission', { permission: id })
      syncRestartRequired(permissions.map((permission) => (permission.id === id ? next : permission)))
      if (next.restart_required) {
        toast.success(tr('Permission granted. Restart RambleDesk to enable screen capture.'))
      } else if (next.status === 'granted') {
        toast.success(tr('Permission granted'))
      } else {
        toast.error(tr('Permission was not granted. Open System Settings and allow it manually.'), {
          description: tr(
            'If System Settings already shows RambleDesk as allowed, turn that switch off and on, then come back and grant access again.',
          ),
        })
      }
    } catch (cause) {
      toast.error(tr('Could not request permission'), { description: messageFrom(cause) })
    } finally {
      busy = false
    }
  }

  async function openSettings(id: string) {
    try {
      await invoke('open_macos_privacy_settings', { permission: id })
      toast.info(tr('Allow RambleDesk in System Settings, then come back and refresh the status.'))
    } catch (cause) {
      toast.error(tr('Could not open System Settings'), { description: messageFrom(cause) })
    }
  }

  async function restartApp() {
    if (restarting) return
    restarting = true
    try {
      await invoke('restart_application')
    } catch (cause) {
      restarting = false
      toast.error(tr('Could not restart RambleDesk'), { description: messageFrom(cause) })
    }
  }

  onMount(() => {
    if (!isTauri) {
      loading = false
      return
    }

    let disposed = false
    let unlistenFocus: (() => void) | undefined

    const refreshWhenVisible = () => {
      if (document.visibilityState === 'visible') void load(false)
    }

    void load(true)
    document.addEventListener('visibilitychange', refreshWhenVisible)
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) void load(false)
      })
      .then((unlisten) => {
        if (disposed) unlisten()
        else unlistenFocus = unlisten
      })
      .catch(() => {
        // The visibility listener still refreshes status if window event setup is unavailable.
      })

    return () => {
      disposed = true
      unlistenFocus?.()
      document.removeEventListener('visibilitychange', refreshWhenVisible)
    }
  })
</script>

<div class="space-y-3">
  {#if loading}
    <div class="flex items-center gap-2 py-6 text-xs text-muted-foreground">
      <LoaderCircle class="size-4 animate-spin" />
      {tr('Reading permission status…')}
    </div>
  {:else if permissions.length === 0}
    <p class="m-0 py-6 text-xs text-muted-foreground">{tr('No extra permissions are needed on this platform.')}</p>
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
          <Badge variant={statusBadge(permission).variant}>
            {#if permission.status === 'granted' && !permission.restart_required}<CheckCircle2 data-icon="inline-start" />{/if}
            {statusBadge(permission).label}
          </Badge>
          {#if permission.restart_required}
            <Button size="sm" disabled={restarting} onclick={() => void restartApp()}>
              {#if restarting}<LoaderCircle class="animate-spin" data-icon="inline-start" />{/if}
              <Rocket data-icon="inline-start" />
              {tr('Restart now')}
            </Button>
          {:else if permission.status !== 'granted'}
            {#if permission.status === 'not_determined'}
              <Button size="sm" disabled={busy} onclick={() => void request(permission.id)}>
                {#if busy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{/if}
                {tr('Grant access')}
              </Button>
              <Button size="sm" variant="outline" onclick={() => void openSettings(permission.id)}>
                <Settings2 data-icon="inline-start" />
                {tr('Open System Settings')}
              </Button>
            {:else}
              <Button size="sm" variant="outline" onclick={() => void openSettings(permission.id)}>
                <Settings2 data-icon="inline-start" />
                {tr('Open System Settings')}
              </Button>
            {/if}
          {/if}
        </div>
      </div>
    {/each}
    <div class="flex items-center justify-between gap-3 rounded-md border bg-muted/20 px-3 py-2">
      <p class="m-0 text-[10px] leading-4 text-muted-foreground">
        {#if screenRestartRequired}
          {tr('Screen capture is allowed, but this process cannot use it until RambleDesk restarts.')}
        {:else if permissions.every((permission) => permission.status === 'granted')}
          {tr('These permissions are ready.')}
        {:else}
          {tr('Allow the system prompt from Grant access. After that, restart RambleDesk before capturing.')}
        {/if}
      </p>
      <Button variant="ghost" size="sm" disabled={loading} onclick={() => void load(true)}>
        <RefreshCw data-icon="inline-start" />
        {tr('Refresh permission status')}
      </Button>
    </div>
  {/if}
</div>
