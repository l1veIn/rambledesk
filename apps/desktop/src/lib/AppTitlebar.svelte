<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { Bell, BellOff, Cog, Minus, X } from '@lucide/svelte'
  import appIcon from '../assets/rambledesk-app-icon.png'
  import { t } from './i18n'
  import { locale } from './preferences'

  export let projectName = 'Vault Zero'
  export let pendingCount = 0
  export let notificationText = '通知'
  export let notificationEnabled = false
  export let notificationDisabled = false
  export let onSettings: () => void = () => {}
  export let onNotifications: () => void = () => {}
  export let onWindowError: (message: string) => void = () => {}

  const isTauri = '__TAURI_INTERNALS__' in window
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent)

  async function runWindowAction(action: 'minimize' | 'close') {
    if (!isTauri) return
    try {
      const appWindow = getCurrentWindow()
      if (action === 'minimize') await appWindow.minimize()
      else await appWindow.close()
    } catch (cause) {
      onWindowError(cause instanceof Error ? cause.message : String(cause))
    }
  }

  async function startDragging(event: PointerEvent) {
    if (!isTauri || event.button !== 0) return
    const target = event.target
    if (target instanceof Element) {
      const button = target.closest('button')
      if (button && !button.matches('.titlebar-brand, .titlebar-drag')) return
    }
    try {
      await getCurrentWindow().startDragging()
    } catch (cause) {
      onWindowError(cause instanceof Error ? cause.message : String(cause))
    }
  }

</script>

<header
  class:mac-titlebar={isMac}
  class="app-titlebar"
>
  {#if isMac}
    <div class="window-controls mac-controls" aria-label={t($locale, '窗口控制')}>
      <button class="traffic close" aria-label={t($locale, '关闭窗口')} onclick={() => runWindowAction('close')}></button>
      <button class="traffic minimize" aria-label={t($locale, '最小化窗口')} onclick={() => runWindowAction('minimize')}></button>
    </div>
  {/if}

  <button
    class="titlebar-brand"
    aria-label={t($locale, '拖动窗口')}
    title={t($locale, '拖动窗口')}
    onpointerdown={(event) => void startDragging(event)}
  >
    <img class="app-mark" src={appIcon} alt="" draggable="false" />
    <strong>RambleDesk</strong>
    <span class="titlebar-divider"></span>
    <span class="project-name">{projectName}</span>
  </button>

  <button
    class="titlebar-drag"
    aria-label={t($locale, '拖动窗口')}
    title={t($locale, '拖动窗口')}
    onpointerdown={(event) => void startDragging(event)}
  ></button>

  <div class="titlebar-status">
    {#if pendingCount > 0}
      <span class="pending-pill">{pendingCount} {t($locale, '待处理')}</span>
    {/if}
    <button
      class:enabled={notificationEnabled}
      class="titlebar-action"
      disabled={notificationDisabled}
      onclick={onNotifications}
      title={notificationText}
      aria-label={notificationText}
    >
      {#if notificationEnabled}<Bell size={16} strokeWidth={1.65} />{:else}<BellOff size={16} strokeWidth={1.65} />{/if}
    </button>
    <button class="titlebar-action" onclick={onSettings} title={t($locale, '设置')} aria-label={t($locale, '设置')}>
      <Cog size={17} strokeWidth={1.75} />
    </button>
  </div>

  {#if !isMac}
    <div class="window-controls" aria-label={t($locale, '窗口控制')}>
      <button aria-label={t($locale, '最小化窗口')} onclick={() => runWindowAction('minimize')}>
        <Minus size={16} strokeWidth={1.5} />
      </button>
      <button class="close-control" aria-label={t($locale, '关闭窗口')} onclick={() => runWindowAction('close')}>
        <X size={16} strokeWidth={1.5} />
      </button>
    </div>
  {/if}
</header>

<style>
  .app-titlebar {
    position: relative;
    display: flex;
    align-items: stretch;
    height: 46px;
    border-bottom: 1px solid var(--line-soft, #d7e0eb);
    border-radius: 15px 15px 0 0;
    background: var(--glass, rgb(247 249 252 / 94%));
    backdrop-filter: blur(18px);
    user-select: none;
    z-index: 30;
  }

  .app-titlebar.mac-titlebar {
    padding-left: 78px;
  }

  .titlebar-brand,
  .titlebar-status,
  .window-controls {
    display: flex;
    align-items: center;
  }

  .titlebar-brand {
    min-width: 0;
    gap: 10px;
    padding: 0 14px;
    color: var(--ink, #193250);
    border: 0;
    background: transparent;
    text-align: left;
    cursor: grab;
  }

  .titlebar-brand strong {
    font-family: "Segoe UI Variable", "Segoe UI", sans-serif;
    font-size: 14px;
    font-weight: 680;
    letter-spacing: -0.015em;
  }

  .app-mark {
    width: 27px;
    height: 27px;
    flex: 0 0 auto;
    border-radius: 7px;
    object-fit: contain;
    pointer-events: none;
    filter: drop-shadow(0 2px 5px rgb(48 83 118 / 14%));
  }

  .titlebar-divider {
    width: 1px;
    height: 18px;
    background: var(--line-soft, #d8e0e9);
  }

  .project-name {
    overflow: hidden;
    max-width: 230px;
    color: var(--ink-soft, #61738a);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .titlebar-drag {
    min-width: 24px;
    flex: 1;
    border: 0;
    background: transparent;
    cursor: grab;
  }

  .titlebar-brand:active,
  .titlebar-drag:active { cursor: grabbing; }

  .titlebar-brand:focus-visible,
  .titlebar-drag:focus-visible { outline: 2px solid var(--blue); outline-offset: -3px; }

  .titlebar-status {
    flex: 0 0 auto;
    gap: 8px;
    padding: 0 12px 0 10px;
  }

  .pending-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border-radius: 999px;
    color: var(--ink-soft, #64758a);
    background: var(--surface-tint, #eef2f7);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.025em;
  }

  .pending-pill {
    color: #a7600a;
    background: #fff4df;
  }

  .titlebar-action,
  .window-controls button {
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    cursor: pointer;
  }

  .titlebar-action {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    color: var(--ink-soft, #6b7d92);
  }

  .titlebar-action:hover:not(:disabled),
  .titlebar-action.enabled {
    color: #3376bb;
    background: var(--blue-soft, #eaf2fb);
  }

  .titlebar-action:disabled {
    cursor: default;
    opacity: 0.42;
  }

  .titlebar-action :global(svg),
  .window-controls :global(svg) {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.5;
  }

  .window-controls {
    align-self: stretch;
    margin-left: 8px;
  }

  .window-controls > button {
    width: 46px;
    height: 100%;
    color: var(--ink, #45586f);
  }

  .window-controls > button:hover {
    background: var(--surface-tint, #e6edf5);
  }

  .window-controls > button.close-control:hover {
    color: #fff;
    background: #d63b43;
  }

  .mac-controls {
    position: absolute;
    left: 15px;
    gap: 8px;
    height: 100%;
    margin: 0;
  }

  .mac-controls > button.traffic {
    width: 12px;
    height: 12px;
    border: 1px solid rgb(0 0 0 / 12%);
    border-radius: 50%;
  }

  .traffic.close { background: #ff5f57; }
  .traffic.minimize { background: #febc2e; }

  @media (max-width: 900px) {
    .project-name,
    .pending-pill {
      display: none;
    }
  }
</style>
