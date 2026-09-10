<script lang="ts">
  import { BellRing, LoaderCircle, Play, Trash2, Upload, Volume2 } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import * as Select from '$lib/components/ui/select'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import { t } from '$lib/i18n'
  import {
    decodeCustomSoundBytes,
    discardCustomSoundCache,
    MAX_CUSTOM_SOUND_SECONDS,
    playNotificationSound,
  } from '$lib/notifications'
  import {
    customNotificationSound,
    locale,
    notificationPopupEnabled,
    notificationSound,
    notificationSoundEnabled,
    notificationVolume,
    setCustomNotificationSound,
    setNotificationPopupEnabled,
    setNotificationSound,
    setNotificationSoundEnabled,
    setNotificationVolume,
    type CustomNotificationSound,
    type NotificationSound,
  } from '$lib/preferences'
  import rambellePermission from '../../assets/rambelle-states/state-permission.webp'

  export let capabilities: WorkbenchCapabilities

  let notificationPermissionError = ''
  let customSoundBusy = false
  let customSoundError = ''

  const isWindows = capabilities.windowControls.implementation.platform() === 'Windows'

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

  async function togglePopupNotifications(enabled: boolean) {
    notificationPermissionError = ''
    if (!enabled) {
      setNotificationPopupEnabled(false)
      return
    }
    if (isWindows) {
      setNotificationPopupEnabled(false)
      notificationPermissionError = tr(
        'Current unsigned Windows builds cannot show system banners. RambleDesk will not try to send them. Watch the inbox badge and use sound alerts instead.',
      )
      return
    }
    if (capabilities.notifications.status.availability === 'unavailable') {
      notificationPermissionError = tr('System notifications are available only in the desktop app.')
      return
    }
    try {
      const currentPermission = await capabilities.notifications.implementation.permission()
      const permission = currentPermission === 'granted'
        ? 'granted'
        : await capabilities.notifications.implementation.requestPermission()
      if (permission === 'granted') {
        setNotificationPopupEnabled(true)
      } else {
        setNotificationPopupEnabled(false)
        notificationPermissionError = tr(
          'The operating system did not grant notification permission. Open System Settings → Notifications → RambleDesk and allow banners.',
        )
      }
    } catch (cause) {
      setNotificationPopupEnabled(false)
      notificationPermissionError = messageFrom(cause)
    }
  }

  function soundLabel(sound: NotificationSound) {
    if (sound === 'soft') return tr('Soft chime')
    if (sound === 'alert') return tr('Attention alert')
    if (sound === 'hakimi') return tr('Hakimi FM')
    if (sound === 'custom') return tr('Custom audio')
    return tr('Bright chime')
  }

  async function chooseCustomSound() {
    if (customSoundBusy || capabilities.notifications.status.availability === 'unavailable') return
    customSoundBusy = true
    customSoundError = ''
    try {
      const selected = await capabilities.serverPaths.implementation.chooseFile({
        extensions: ['mp3', 'wav', 'ogg', 'm4a', 'aac'],
      })
      if (!selected) return
      const imported = await capabilities.notifications.implementation.importSound(selected)
      let duration = 0
      try {
        const decoded = await decodeCustomSoundBytes(imported.id, [...imported.bytes])
        duration = decoded.duration
      } catch {
        await capabilities.notifications.implementation.removeSound(imported.id).catch(() => {})
        customSoundError = tr('Could not decode this audio file. Try a different one.')
        return
      }
      if (duration > MAX_CUSTOM_SOUND_SECONDS) {
        await capabilities.notifications.implementation.removeSound(imported.id).catch(() => {})
        customSoundError = tr('Audio exceeds the 10-second limit. Trim it and try again.')
        return
      }
      // Cleanup of the previous sound happens only after validation succeeded.
      await capabilities.notifications.implementation.commitSound(imported.id).catch(() => {})
      const next: CustomNotificationSound = { id: imported.id, name: imported.name }
      setCustomNotificationSound(next)
      setNotificationSound('custom')
    } catch (cause) {
      customSoundError = messageFrom(cause)
    } finally {
      customSoundBusy = false
    }
  }

  async function removeCustomSound() {
    const current = $customNotificationSound
    if (!current) return
    discardCustomSoundCache()
    if (capabilities.notifications.status.availability !== 'unavailable') {
      await capabilities.notifications.implementation.removeSound(current.id).catch(() => {})
    }
    setCustomNotificationSound(null)
    setNotificationSound('chime')
    customSoundError = ''
  }
</script>

            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <BellRing class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('System notifications')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {#if isWindows}
                      {tr('Current unsigned Windows builds cannot show system banners. RambleDesk will not try to send them. Watch the inbox badge and use sound alerts instead.')}
                    {:else}
                      {tr('On macOS, allow RambleDesk in System Settings → Notifications. Banners may stay hidden while this window is focused; check Notification Center if a request arrives while you are already here.')}
                    {/if}
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={$notificationPopupEnabled}
                aria-label={tr('System notifications')}
                disabled={isWindows}
                class={[
                  'relative h-[22px] w-10 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                  $notificationPopupEnabled ? 'bg-primary' : 'bg-input',
                ]}
                onclick={() => void togglePopupNotifications(!$notificationPopupEnabled)}
              >
                <span
                  class={[
                    'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform',
                    $notificationPopupEnabled ? 'translate-x-5' : 'translate-x-0',
                  ]}
                ></span>
              </button>
              {#if notificationPermissionError}
                <div class="col-span-2 flex items-center gap-3 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2">
                  <img src={rambellePermission} alt="" class="size-14 shrink-0 object-contain" aria-hidden="true" />
                  <p class="m-0 text-xs text-destructive">{notificationPermissionError}</p>
                </div>
              {/if}
            </section>

            <section class="border-b pb-8">
              <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8">
                <div class="flex gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <Volume2 class="size-4" />
                  </span>
                  <div>
                    <h3 class="m-0 text-sm font-medium">{tr('Sound alerts')}</h3>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('Sound is independent of system notifications and can play even when popup permission is disabled.')}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={$notificationSoundEnabled}
                  aria-label={tr('Sound alerts')}
                  class={[
                    'relative h-[22px] w-10 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                    $notificationSoundEnabled ? 'bg-primary' : 'bg-input',
                  ]}
                  onclick={() => setNotificationSoundEnabled(!$notificationSoundEnabled)}
                >
                  <span
                    class={[
                      'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform',
                      $notificationSoundEnabled ? 'translate-x-5' : 'translate-x-0',
                    ]}
                  ></span>
                </button>
              </div>

              {#if $notificationSoundEnabled}
                <div class="ml-11 mt-5 grid gap-5 rounded-md border bg-muted/20 p-4">
                  <div class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('Alert sound')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('Choose the sound played for new requests and preview it immediately.')}
                      </span>
                    </div>
                    <div class="flex items-center gap-2">
                      <Select.Root
                        type="single"
                        value={$notificationSound}
                        onValueChange={(value: string) => setNotificationSound(value as NotificationSound)}
                      >
                        <Select.Trigger class="min-w-0 flex-1">
                          {soundLabel($notificationSound)}
                        </Select.Trigger>
                        <Select.Content>
                          <Select.Item value="chime" label={tr('Bright chime')} />
                          <Select.Item value="soft" label={tr('Soft chime')} />
                          <Select.Item value="alert" label={tr('Attention alert')} />
                          <Select.Item value="hakimi" label={tr('Hakimi FM')} />
                          <Select.Item value="custom" label={tr('Custom audio')} />
                        </Select.Content>
                      </Select.Root>
                      <Button
                        variant="outline"
                        size="icon"
                        aria-label={tr('Preview alert sound')}
                        title={tr('Preview alert sound')}
                        onclick={() =>
                          void playNotificationSound(
                            $notificationSound,
                            $notificationVolume,
                            $notificationSound === 'custom' ? $customNotificationSound : null,
                            capabilities.notifications.implementation.readCustomSound,
                          )}
                      >
                        <Play />
                      </Button>
                    </div>
                  </div>

                  {#if $notificationSound === 'custom'}
                    <div class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-6">
                      <div>
                        <strong class="block text-xs font-medium">{tr('Custom audio')}</strong>
                        <span class="mt-0.5 block text-[10px] text-muted-foreground">
                          {tr('Choose an audio file as the alert sound (up to 10 seconds and 5 MiB).')}
                        </span>
                      </div>
                      <div class="flex items-center gap-2">
                        <Button
                          variant="outline"
                          size="sm"
                          class="min-w-0 flex-1"
                          disabled={capabilities.serverPaths.status.availability === 'unavailable' || customSoundBusy}
                          onclick={() => void chooseCustomSound()}
                        >
                          {#if customSoundBusy}
                            <LoaderCircle class="animate-spin" data-icon="inline-start" />
                          {:else}
                            <Upload data-icon="inline-start" />
                          {/if}
                          {tr('Choose audio…')}
                        </Button>
                        {#if $customNotificationSound}
                          <Button
                            variant="outline"
                            size="icon"
                            aria-label={tr('Remove custom audio')}
                            title={tr('Remove custom audio')}
                            onclick={() => void removeCustomSound()}
                          >
                            <Trash2 />
                          </Button>
                        {/if}
                      </div>
                    </div>
                    {#if $customNotificationSound}
                      <p class="m-0 break-all text-[10px] text-muted-foreground">
                        {tr('Current alert sound: {name}', { name: $customNotificationSound.name })}
                      </p>
                    {/if}
                    {#if customSoundError}
                      <p class="m-0 text-xs text-destructive">{customSoundError}</p>
                    {/if}
                    {#if capabilities.notifications.status.availability === 'unavailable'}
                      <p class="m-0 text-[10px] text-muted-foreground">
                        {tr('Custom alert sounds are available only in the desktop app.')}
                      </p>
                    {/if}
                  {/if}

                  <div class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('Volume')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('Adjust alert sound volume.')}
                      </span>
                    </div>
                    <div class="flex items-center gap-3">
                      <input
                        type="range"
                        min="0"
                        max="100"
                        step="5"
                        value={$notificationVolume}
                        class="min-w-0 flex-1 accent-primary"
                        aria-label={tr('Volume')}
                        oninput={(event) =>
                          setNotificationVolume(Number((event.currentTarget as HTMLInputElement).value))}
                      />
                      <span class="w-9 text-right text-[10px] tabular-nums text-muted-foreground">
                        {$notificationVolume}%
                      </span>
                    </div>
                  </div>
                </div>
              {/if}
            </section>
