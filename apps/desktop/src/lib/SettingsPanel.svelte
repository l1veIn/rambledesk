<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification'
  import {
    BellRing,
    Check,
    CheckCircle2,
    ChevronDown,
    Clipboard,
    Download,
    Languages,
    LoaderCircle,
    MonitorCog,
    Play,
    PlugZap,
    RefreshCw,
    ShieldCheck,
    TerminalSquare,
    Volume2,
  } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import * as Alert from '$lib/components/ui/alert'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import * as Dialog from '$lib/components/ui/dialog'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import * as Select from '$lib/components/ui/select'
  import * as Tabs from '$lib/components/ui/tabs'
  import { t } from '$lib/i18n'
  import { playNotificationSound } from '$lib/notifications'
  import {
    locale,
    notificationPopupEnabled,
    notificationSound,
    notificationSoundEnabled,
    notificationVolume,
    setLocale,
    setNotificationPopupEnabled,
    setNotificationSound,
    setNotificationSoundEnabled,
    setNotificationVolume,
    setThemePreference,
    themePreference,
    type NotificationSound,
    type ThemePreference,
  } from '$lib/preferences'

  type Section = 'general' | 'notifications' | 'adapters'

  export let mcpConfiguration = ''
  export let initialSection: Section = 'general'
  export let onClose: () => void = () => {}

  type McpHostView = {
    id: string
    name: string
    iconSvg: string
    installed: boolean
    configured: boolean
    configPath: string
    restartRequired: boolean
  }

  type McpInstallResult = {
    hostId: string
    action: 'created' | 'updated' | 'unchanged'
    configPath: string
    restartRequired: boolean
  }

  let dialogOpen = true
  let closeDelivered = false
  let activeSection: Section = initialSection
  let hosts: McpHostView[] = []
  let selectedIds = new Set<string>()
  let loadingHosts = true
  let installing = false
  let installMessage = ''
  let installError = ''
  let installingPi = false
  let piInstallMessage = ''
  let piInstallError = ''
  let copyState: 'idle' | 'copied' | 'error' = 'idle'
  let genericAdapterOpen = true
  let configurationOpen = false
  let notificationPermissionError = ''
  const isTauri = '__TAURI_INTERNALS__' in window

  $: installedHosts = hosts.filter((host) => host.installed)
  $: selectedCount = selectedIds.size
  $: if (!dialogOpen && !closeDelivered) {
    closeDelivered = true
    onClose()
  }

  onMount(() => {
    if (isTauri) void refreshHosts()
    else loadingHosts = false
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function refreshHosts() {
    loadingHosts = true
    installError = ''
    try {
      hosts = await invoke<McpHostView[]>('detect_generic_mcp_hosts')
      selectedIds = new Set(
        hosts
          .filter((host) => host.installed && !host.configured)
          .map((host) => host.id),
      )
    } catch (cause) {
      installError = messageFrom(cause)
    } finally {
      loadingHosts = false
    }
  }

  function toggleHost(host: McpHostView) {
    if (!host.installed || installing) return
    const next = new Set(selectedIds)
    if (next.has(host.id)) next.delete(host.id)
    else next.add(host.id)
    selectedIds = next
  }

  async function installSelected() {
    if (selectedIds.size === 0 || installing) return
    installing = true
    installError = ''
    installMessage = ''
    try {
      const results = await invoke<McpInstallResult[]>('install_generic_mcp_hosts', {
        hostIds: [...selectedIds],
      })
      const changed = results.filter((result) => result.action !== 'unchanged').length
      installMessage = tr('已为 {count} 个工具写入通用 MCP 适配器配置；重启这些工具后生效。', {
        count: changed,
      })
      await refreshHosts()
    } catch (cause) {
      installError = messageFrom(cause)
    } finally {
      installing = false
    }
  }

  async function copyConfiguration() {
    try {
      await navigator.clipboard.writeText(mcpConfiguration)
      copyState = 'copied'
    } catch {
      copyState = 'error'
    }
  }

  async function installPiPackage() {
    if (installingPi) return
    installingPi = true
    piInstallError = ''
    piInstallMessage = ''
    try {
      const output = await invoke<string>('install_pi_package', {
        checkoutRoot: null,
      })
      piInstallMessage =
        tr('已安装 Pi 原生适配器，重启 Pi 会话后生效。') +
        (output.trim() ? `\n${output.trim()}` : '')
    } catch (cause) {
      piInstallError = messageFrom(cause)
    } finally {
      installingPi = false
    }
  }

  async function togglePopupNotifications(enabled: boolean) {
    notificationPermissionError = ''
    if (!enabled) {
      setNotificationPopupEnabled(false)
      return
    }
    if (!isTauri) {
      notificationPermissionError = tr('系统弹窗通知只在桌面应用中可用。')
      return
    }
    try {
      const permission = (await isPermissionGranted()) ? 'granted' : await requestPermission()
      if (permission === 'granted') {
        setNotificationPopupEnabled(true)
      } else {
        setNotificationPopupEnabled(false)
        notificationPermissionError = tr('操作系统没有授予弹窗通知权限。')
      }
    } catch (cause) {
      setNotificationPopupEnabled(false)
      notificationPermissionError = messageFrom(cause)
    }
  }

  function soundLabel(sound: NotificationSound) {
    if (sound === 'soft') return tr('柔和提示')
    if (sound === 'alert') return tr('醒目提示')
    return tr('清脆双音')
  }

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return String(cause)
  }
</script>

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content
    class="h-[min(680px,calc(100vh-5rem))] w-[min(940px,calc(100vw-3rem))] max-w-none gap-0 overflow-hidden p-0 sm:max-w-none"
    aria-describedby="settings-description"
  >
    <Dialog.Header class="sr-only">
      <Dialog.Title>{tr('设置')}</Dialog.Title>
      <Dialog.Description id="settings-description">
        {tr('管理界面偏好和宿主适配器。')}
      </Dialog.Description>
    </Dialog.Header>

    <Tabs.Root
      bind:value={activeSection}
      orientation="vertical"
      class="grid h-full min-h-0 grid-cols-[184px_minmax(0,1fr)] gap-0"
    >
      <aside class="flex min-h-0 flex-col border-r bg-muted/35 p-3">
        <div class="flex h-12 items-center gap-2 px-2">
          <span class="grid size-7 place-items-center rounded-md bg-primary text-xs font-bold text-primary-foreground">
            R
          </span>
          <div class="min-w-0">
            <strong class="block text-xs font-semibold">RambleDesk</strong>
            <span class="block text-[10px] text-muted-foreground">{tr('设置')}</span>
          </div>
        </div>

        <Tabs.List
          variant="line"
          class="mt-3 flex w-full flex-col items-stretch gap-1 bg-transparent p-0"
        >
          <Tabs.Trigger value="general" class="h-9 w-full justify-start px-2.5">
            <MonitorCog data-icon="inline-start" />
            {tr('通用')}
          </Tabs.Trigger>
          <Tabs.Trigger value="notifications" class="h-9 w-full justify-start px-2.5">
            <BellRing data-icon="inline-start" />
            {tr('通知')}
          </Tabs.Trigger>
          <Tabs.Trigger value="adapters" class="h-9 w-full justify-start px-2.5">
            <PlugZap data-icon="inline-start" />
            <span class="flex-1 text-left">{tr('适配器')}</span>
            {#if installedHosts.length > 0}
              <Badge variant="secondary" class="h-5 px-1.5 text-[9px]">
                {installedHosts.length}
              </Badge>
            {/if}
          </Tabs.Trigger>
        </Tabs.List>

        <div class="mt-auto flex gap-2 border-t pt-3 text-[10px] leading-4 text-muted-foreground">
          <ShieldCheck class="mt-0.5 size-3.5 shrink-0" />
          <span>{tr('适配器配置只写入当前用户目录，并保留其他适配器。')}</span>
        </div>
      </aside>

      <div class="flex min-h-0 min-w-0 flex-col">
        <header class="flex h-16 shrink-0 items-center border-b px-6">
          <div>
            <p class="m-0 text-[10px] font-medium uppercase text-muted-foreground">
              {activeSection === 'general'
                ? tr('偏好设置')
                : activeSection === 'notifications'
                  ? tr('提醒方式')
                  : tr('宿主适配')}
            </p>
            <h2 class="m-0 mt-0.5 text-base font-semibold">
              {activeSection === 'general'
                ? tr('通用')
                : activeSection === 'notifications'
                  ? tr('通知')
                  : tr('适配器')}
            </h2>
          </div>
        </header>

        <ScrollArea class="min-h-0 flex-1">
          <Tabs.Content value="general" class="m-0 space-y-8 p-6 outline-none">
            <section class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <Languages class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('语言')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('选择 RambleDesk 的界面语言。')}
                  </p>
                </div>
              </div>
              <Select.Root
                type="single"
                value={$locale}
                onValueChange={(value: string) => setLocale(value as 'zh-CN' | 'en')}
              >
                <Select.Trigger class="w-full">
                  {$locale === 'zh-CN' ? '简体中文' : 'English'}
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="zh-CN" label="简体中文" />
                  <Select.Item value="en" label="English" />
                </Select.Content>
              </Select.Root>
            </section>

            <section class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <MonitorCog class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('外观')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('选择界面明暗模式，也可以跟随操作系统。')}
                  </p>
                </div>
              </div>
              <Select.Root
                type="single"
                value={$themePreference}
                onValueChange={(value: string) => setThemePreference(value as ThemePreference)}
              >
                <Select.Trigger class="w-full">
                  {$themePreference === 'system'
                    ? tr('跟随系统')
                    : $themePreference === 'light'
                      ? tr('浅色')
                      : tr('深色')}
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="system" label={tr('跟随系统')} />
                  <Select.Item value="light" label={tr('浅色')} />
                  <Select.Item value="dark" label={tr('深色')} />
                </Select.Content>
              </Select.Root>
            </section>
          </Tabs.Content>

          <Tabs.Content value="notifications" class="m-0 space-y-8 p-6 outline-none">
            <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
              <div class="flex gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <BellRing class="size-4" />
                </span>
                <div>
                  <h3 class="m-0 text-sm font-medium">{tr('系统弹窗')}</h3>
                  <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                    {tr('新请求到达时使用 Windows、macOS 或 Linux 的系统消息通知。')}
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={$notificationPopupEnabled}
                aria-label={tr('系统弹窗')}
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
                <p class="col-span-2 m-0 text-xs text-destructive">{notificationPermissionError}</p>
              {/if}
            </section>

            <section class="border-b pb-8">
              <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8">
                <div class="flex gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <Volume2 class="size-4" />
                  </span>
                  <div>
                    <h3 class="m-0 text-sm font-medium">{tr('声音提醒')}</h3>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {tr('声音与系统弹窗相互独立；即使弹窗权限关闭也可以响铃。')}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={$notificationSoundEnabled}
                  aria-label={tr('声音提醒')}
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
                      <strong class="block text-xs font-medium">{tr('提示音')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('选择新请求到达时播放的声音，并可立即试听。')}
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
                          <Select.Item value="chime" label={tr('清脆双音')} />
                          <Select.Item value="soft" label={tr('柔和提示')} />
                          <Select.Item value="alert" label={tr('醒目提示')} />
                        </Select.Content>
                      </Select.Root>
                      <Button
                        variant="outline"
                        size="icon"
                        aria-label={tr('试听提示音')}
                        title={tr('试听提示音')}
                        onclick={() => void playNotificationSound($notificationSound, $notificationVolume)}
                      >
                        <Play />
                      </Button>
                    </div>
                  </div>

                  <div class="grid grid-cols-[minmax(0,1fr)_240px] items-center gap-6">
                    <div>
                      <strong class="block text-xs font-medium">{tr('音量')}</strong>
                      <span class="mt-0.5 block text-[10px] text-muted-foreground">
                        {tr('调整提示音音量。')}
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
                        aria-label={tr('音量')}
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
          </Tabs.Content>

          <Tabs.Content value="adapters" class="m-0 space-y-8 p-6 outline-none">
            <section class="border-b pb-8">
              <div class="flex items-start gap-3">
                <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                  <PlugZap class="size-4" />
                </span>
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <h3 class="m-0 text-sm font-medium">{tr('Pi 原生适配器')}</h3>
                    <Badge variant="secondary">{tr('原生等待')}</Badge>
                  </div>
                  <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                    {tr('Pi package 通过本地 JSON API 请求、查询、等待和取消；等待发生在 Pi 工具调用内。')}
                  </p>
                </div>
                <Button disabled={installingPi || !isTauri} onclick={installPiPackage}>
                  {#if installingPi}
                    <LoaderCircle class="animate-spin" data-icon="inline-start" />
                    {tr('正在安装…')}
                  {:else}
                    <Download data-icon="inline-start" />
                    {tr('安装')}
                  {/if}
                </Button>
              </div>
              {#if piInstallMessage}
                <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                  <CheckCircle2 />
                  <Alert.Title>{tr('安装完成')}</Alert.Title>
                  <Alert.Description class="whitespace-pre-wrap">{piInstallMessage}</Alert.Description>
                </Alert.Root>
              {/if}
              {#if piInstallError}
                <Alert.Root variant="destructive" class="mt-4">
                  <Alert.Title>{tr('安装失败')}</Alert.Title>
                  <Alert.Description>{piInstallError}</Alert.Description>
                </Alert.Root>
              {/if}
            </section>

            <section>
              <Collapsible.Root bind:open={genericAdapterOpen}>
                <div class="flex items-start gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
                    <TerminalSquare class="size-4" />
                  </span>
                  <div class="min-w-0 flex-1">
                    <div class="flex flex-wrap items-center gap-2">
                      <h3 class="m-0 text-sm font-medium">{tr('通用 MCP 适配器')}</h3>
                      <Badge variant="outline">{tr('手动继续')}</Badge>
                    </div>
                    <p class="m-0 mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
                      {tr('为支持 MCP 的宿主提供反馈工具；提交或取消后由 Resume Prompt 引导用户继续宿主会话。')}
                    </p>
                  </div>
                  <div class="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      disabled={loadingHosts || installing || !isTauri}
                      aria-label={tr('重新检测')}
                      title={tr('重新检测')}
                      onclick={refreshHosts}
                    >
                      <RefreshCw class={loadingHosts ? 'animate-spin' : ''} />
                    </Button>
                    <Collapsible.Trigger>
                      {#snippet child({ props })}
                        <Button
                          {...props}
                          variant="ghost"
                          size="icon-sm"
                          aria-label={genericAdapterOpen ? tr('收起') : tr('展开')}
                        >
                          <ChevronDown
                            class={[
                              'transition-transform',
                              genericAdapterOpen ? 'rotate-180' : '',
                            ]}
                          />
                        </Button>
                      {/snippet}
                    </Collapsible.Trigger>
                  </div>
                </div>

                <Collapsible.Content class="pt-4">
                  {#if loadingHosts}
                    <div class="flex h-24 items-center justify-center gap-2 text-xs text-muted-foreground">
                      <LoaderCircle class="size-4 animate-spin" />
                      {tr('正在检测 Coding 工具…')}
                    </div>
                  {:else if hosts.length === 0}
                    <p class="m-0 border-y py-5 text-center text-xs text-muted-foreground">
                      {isTauri ? tr('没有检测到支持的宿主') : tr('请在桌面应用中管理适配器')}
                    </p>
                  {:else}
                    <div class="divide-y border-y">
                      {#each hosts as host (host.id)}
                        <label
                          class={[
                            'flex min-h-12 items-center gap-3 px-2 py-2 text-xs transition-colors',
                            host.installed
                              ? 'cursor-pointer hover:bg-muted/60'
                              : 'cursor-not-allowed opacity-50',
                          ]}
                        >
                          <input
                            type="checkbox"
                            class="size-3.5 accent-primary"
                            checked={selectedIds.has(host.id)}
                            disabled={!host.installed || installing}
                            onchange={() => toggleHost(host)}
                          />
                          <span class="grid size-5 shrink-0 place-items-center [&_svg]:size-4">
                            {@html host.iconSvg}
                          </span>
                          <span class="min-w-0 flex-1">
                            <strong class="block truncate font-medium">{host.name}</strong>
                            <span class="block truncate text-[10px] text-muted-foreground" title={host.configPath}>
                              {host.configPath}
                            </span>
                          </span>
                          <Badge variant={host.configured ? 'secondary' : 'outline'}>
                            {host.configured
                              ? tr('已配置')
                              : host.installed
                                ? tr('已检测')
                                : tr('未检测到')}
                          </Badge>
                        </label>
                      {/each}
                    </div>
                  {/if}

                  <div class="mt-3 flex items-center justify-between gap-4">
                    <p class="m-0 text-[10px] leading-4 text-muted-foreground">
                      {tr('只更新 RambleDesk 的 MCP 条目；不会覆盖宿主中的其他配置。')}
                    </p>
                    <Button
                      disabled={selectedCount === 0 || installing || !isTauri}
                      onclick={installSelected}
                    >
                      {#if installing}
                        <LoaderCircle class="animate-spin" data-icon="inline-start" />
                      {:else}
                        <PlugZap data-icon="inline-start" />
                      {/if}
                      {selectedCount > 0
                        ? tr('配置所选（{count}）', { count: selectedCount })
                        : tr('选择宿主')}
                    </Button>
                  </div>

                  {#if installMessage}
                    <Alert.Root class="mt-4 border-success/30 bg-success/5 text-success">
                      <CheckCircle2 />
                      <Alert.Title>{tr('配置完成')}</Alert.Title>
                      <Alert.Description>{installMessage}</Alert.Description>
                    </Alert.Root>
                  {/if}
                  {#if installError}
                    <Alert.Root variant="destructive" class="mt-4">
                      <Alert.Title>{tr('配置失败')}</Alert.Title>
                      <Alert.Description>{installError}</Alert.Description>
                    </Alert.Root>
                  {/if}
                </Collapsible.Content>
              </Collapsible.Root>
            </section>

            <Collapsible.Root bind:open={configurationOpen} class="border-t pt-5">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <strong class="block text-xs font-medium">{tr('通用 MCP 配置')}</strong>
                  <span class="block text-[10px] text-muted-foreground">
                    {tr('仅用于手动配置和故障排查。')}
                  </span>
                </div>
                <Collapsible.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} variant="ghost" size="sm">
                      {configurationOpen ? tr('收起') : tr('查看')}
                      <ChevronDown
                        data-icon="inline-end"
                        class={['transition-transform', configurationOpen ? 'rotate-180' : '']}
                      />
                    </Button>
                  {/snippet}
                </Collapsible.Trigger>
              </div>
              <Collapsible.Content class="pt-3">
                <Alert.Root>
                  <ShieldCheck />
                  <Alert.Title>{tr('本机凭证')}</Alert.Title>
                  <Alert.Description>
                    {tr('配置中包含仅限本机使用的访问令牌，请勿发送给他人。')}
                  </Alert.Description>
                </Alert.Root>
                <pre class="mt-3 max-h-44 overflow-auto rounded-md border bg-muted/45 p-3 text-[10px] leading-4">{mcpConfiguration}</pre>
                <div class="mt-2 flex items-center justify-end gap-2">
                  {#if copyState === 'error'}
                    <span class="text-[10px] text-destructive">{tr('无法访问剪贴板，请手动复制')}</span>
                  {/if}
                  <Button variant="outline" size="sm" onclick={copyConfiguration}>
                    {#if copyState === 'copied'}
                      <Check data-icon="inline-start" />
                      {tr('已复制')}
                    {:else}
                      <Clipboard data-icon="inline-start" />
                      {tr('复制配置')}
                    {/if}
                  </Button>
                </div>
              </Collapsible.Content>
            </Collapsible.Root>
          </Tabs.Content>
        </ScrollArea>
      </div>
    </Tabs.Root>
  </Dialog.Content>
</Dialog.Root>
