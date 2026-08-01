<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import {
    Check,
    CheckCircle2,
    ChevronRight,
    Clipboard,
    Languages,
    LoaderCircle,
    MonitorCog,
    PlugZap,
    RefreshCw,
    Settings2,
    ShieldCheck,
    TerminalSquare,
    X,
  } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { t } from './i18n'
  import {
    locale,
    setLocale,
    setThemePreference,
    themePreference,
    type ThemePreference,
  } from './preferences'

  type Section = 'general' | 'mcp'

  export let mcpConfiguration = ''
  export let initialSection: Section = 'general'
  export let onClose: () => void = () => {}

  type McpClientView = {
    id: string
    name: string
    installed: boolean
    configured: boolean
    configPath: string
    restartRequired: boolean
  }
  type McpInstallResult = {
    clientId: string
    action: 'created' | 'updated' | 'unchanged'
    configPath: string
    restartRequired: boolean
  }

  let activeSection: Section = initialSection
  let clients: McpClientView[] = []
  let selectedIds = new Set<string>()
  let loadingClients = true
  let installing = false
  let installMessage = ''
  let installError = ''
  let copyState: 'idle' | 'copied' | 'error' = 'idle'
  const isTauri = '__TAURI_INTERNALS__' in window

  $: installedClients = clients.filter((client) => client.installed)
  $: selectedCount = selectedIds.size

  onMount(() => {
    if (!isTauri) {
      loadingClients = false
      return
    }
    void refreshClients()
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function refreshClients() {
    loadingClients = true
    installError = ''
    try {
      clients = await invoke<McpClientView[]>('detect_mcp_clients')
      selectedIds = new Set(
        clients
          .filter((client) => client.installed && !client.configured)
          .map((client) => client.id),
      )
    } catch (cause) {
      installError = messageFrom(cause)
    } finally {
      loadingClients = false
    }
  }

  function toggleClient(client: McpClientView) {
    if (!client.installed || installing) return
    const next = new Set(selectedIds)
    if (next.has(client.id)) next.delete(client.id)
    else next.add(client.id)
    selectedIds = next
  }

  async function installSelected() {
    if (selectedIds.size === 0 || installing) return
    installing = true
    installError = ''
    installMessage = ''
    try {
      const results = await invoke<McpInstallResult[]>('install_mcp_clients', {
        clientIds: [...selectedIds],
      })
      const changed = results.filter((result) => result.action !== 'unchanged').length
      installMessage = tr('已为 {count} 个工具写入 RambleDesk MCP；重启这些工具后生效。', {
        count: changed,
      })
      await refreshClients()
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

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return String(cause)
  }
</script>

<div class="settings-backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && onClose()}>
  <div class="settings-shell" role="dialog" aria-modal="true" aria-labelledby="settings-title">
    <aside class="settings-sidebar">
      <div class="settings-brand">
        <span class="settings-mark"><Settings2 size={18} strokeWidth={1.8} /></span>
        <div>
          <p>RAMBLEDESK</p>
          <strong id="settings-title">{tr('设置')}</strong>
        </div>
      </div>

      <nav aria-label={tr('设置分类')}>
        <button class:active={activeSection === 'general'} onclick={() => (activeSection = 'general')}>
          <MonitorCog size={17} strokeWidth={1.8} />
          <span>{tr('通用')}</span>
          <ChevronRight size={15} />
        </button>
        <button class:active={activeSection === 'mcp'} onclick={() => (activeSection = 'mcp')}>
          <PlugZap size={17} strokeWidth={1.8} />
          <span>MCP</span>
          {#if installedClients.length > 0}<em>{installedClients.length}</em>{/if}
          <ChevronRight size={15} />
        </button>
      </nav>

      <div class="settings-safety">
        <ShieldCheck size={16} strokeWidth={1.8} />
        <span>{tr('配置只写入当前用户目录，并保留其他 MCP 服务。')}</span>
      </div>
    </aside>

    <div class="settings-content">
      <header class="settings-header">
        <div>
          <p class="eyebrow">{activeSection === 'general' ? tr('偏好设置') : tr('本地集成')}</p>
          <h2>{activeSection === 'general' ? tr('通用') : tr('MCP 接入')}</h2>
        </div>
        <button class="settings-close" aria-label={tr('关闭设置')} onclick={onClose}><X size={18} /></button>
      </header>

      <div class="settings-scroll">
        {#if activeSection === 'general'}
          <section class="settings-section">
            <div class="section-heading">
              <span><Languages size={18} strokeWidth={1.8} /></span>
              <div><h3>{tr('语言')}</h3><p>{tr('选择 RambleDesk 的界面语言。')}</p></div>
            </div>
            <div class="choice-grid two-columns">
              <button class:active={$locale === 'zh-CN'} onclick={() => setLocale('zh-CN')}>
                <strong>简体中文</strong><small>Chinese (Simplified)</small>
                {#if $locale === 'zh-CN'}<CheckCircle2 size={17} />{/if}
              </button>
              <button class:active={$locale === 'en'} onclick={() => setLocale('en')}>
                <strong>English</strong><small>English</small>
                {#if $locale === 'en'}<CheckCircle2 size={17} />{/if}
              </button>
            </div>
          </section>

          <section class="settings-section">
            <div class="section-heading">
              <span><MonitorCog size={18} strokeWidth={1.8} /></span>
              <div><h3>{tr('外观')}</h3><p>{tr('选择界面明暗模式，也可以跟随操作系统。')}</p></div>
            </div>
            <div class="choice-grid three-columns">
              {#each [
                { id: 'system', label: tr('跟随系统'), detail: tr('自动适配') },
                { id: 'light', label: tr('浅色'), detail: tr('明亮清晰') },
                { id: 'dark', label: tr('深色'), detail: tr('低光舒适') },
              ] as choice}
                <button
                  class:active={$themePreference === choice.id}
                  onclick={() => setThemePreference(choice.id as ThemePreference)}
                >
                  <strong>{choice.label}</strong><small>{choice.detail}</small>
                  {#if $themePreference === choice.id}<CheckCircle2 size={17} />{/if}
                </button>
              {/each}
            </div>
          </section>
        {:else}
          <section class="settings-section mcp-intro">
            <div class="section-heading">
              <span><TerminalSquare size={18} strokeWidth={1.8} /></span>
              <div>
                <h3>{tr('Coding 工具')}</h3>
                <p>{tr('自动检测本机工具，并将 RambleDesk 的本地 HTTP MCP 安全合并到对应配置。')}</p>
              </div>
              <button class="refresh-clients" disabled={loadingClients || installing} onclick={refreshClients}>
                <RefreshCw size={15} class={loadingClients ? 'spinning' : ''} />{tr('重新检测')}
              </button>
            </div>

            {#if loadingClients}
              <div class="settings-loading"><LoaderCircle class="spinning" size={20} />{tr('正在检测 Coding 工具…')}</div>
            {:else}
              <div class="client-list">
                {#each clients as client}
                  <button
                    class="client-row"
                    class:selected={selectedIds.has(client.id)}
                    class:unavailable={!client.installed}
                    disabled={!client.installed || installing}
                    onclick={() => toggleClient(client)}
                  >
                    <span class="client-check">
                      {#if selectedIds.has(client.id)}<Check size={14} strokeWidth={2.2} />{/if}
                    </span>
                    <span class="client-icon"><TerminalSquare size={19} strokeWidth={1.7} /></span>
                    <span class="client-copy">
                      <strong>{client.name}</strong>
                      <small title={client.configPath}>{client.configPath}</small>
                    </span>
                    <span class:configured={client.configured} class="client-status">
                      {client.configured ? tr('已接入') : client.installed ? tr('已检测') : tr('未检测到')}
                    </span>
                  </button>
                {/each}
              </div>
            {/if}

            {#if installMessage}<p class="settings-success"><CheckCircle2 size={16} />{installMessage}</p>{/if}
            {#if installError}<p class="settings-error">{installError}</p>{/if}

            <div class="install-bar">
              <div>
                <strong>{tr('一键接入 RambleDesk')}</strong>
                <span>{tr('只更新 rambledesk 条目，不覆盖其他配置。')}</span>
              </div>
              <button class="install-button" disabled={selectedCount === 0 || installing} onclick={installSelected}>
                {#if installing}<LoaderCircle class="spinning" size={16} />{:else}<PlugZap size={16} />{/if}
                {selectedCount > 0 ? tr('接入所选（{count}）', { count: selectedCount }) : tr('选择工具')}
              </button>
            </div>
          </section>

          <details class="manual-config">
            <summary>{tr('手动配置与故障排查')}<ChevronRight size={15} /></summary>
            <p>{tr('配置中包含仅限本机使用的访问令牌，请勿发送给他人。')}</p>
            <pre>{mcpConfiguration}</pre>
            <button onclick={copyConfiguration}>
              {#if copyState === 'copied'}<Check size={15} />{tr('已复制')}{:else}<Clipboard size={15} />{tr('复制 MCP 配置')}{/if}
            </button>
            {#if copyState === 'error'}<small class="settings-error">{tr('无法访问剪贴板，请手动复制')}</small>{/if}
          </details>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .settings-backdrop {
    position: fixed;
    inset: 46px 0 0;
    z-index: 60;
    display: grid;
    place-items: center;
    padding: 28px;
    background: rgb(9 18 31 / 48%);
    backdrop-filter: blur(10px);
  }

  .settings-shell {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    width: min(920px, calc(100vw - 64px));
    height: min(660px, calc(100vh - 110px));
    overflow: hidden;
    border: 1px solid var(--line, #cbd8e6);
    border-radius: 18px;
    color: var(--ink, #18304b);
    background: var(--surface, #f8fbff);
    box-shadow: 0 28px 80px rgb(9 22 39 / 28%);
  }

  .settings-sidebar {
    display: flex;
    min-width: 0;
    flex-direction: column;
    padding: 22px 14px 16px;
    border-right: 1px solid var(--line-soft, #dbe4ee);
    background: var(--surface-tint, #eef4fa);
  }

  .settings-brand { display: flex; align-items: center; gap: 11px; padding: 0 8px 24px; }
  .settings-mark { display: grid; width: 34px; height: 34px; place-items: center; border-radius: 10px; color: #3e82c7; background: var(--blue-soft, #e8f2fd); }
  .settings-brand p, .eyebrow { margin: 0 0 3px; color: #4d88c8; font-size: 9px; font-weight: 750; letter-spacing: .16em; }
  .settings-brand strong { font-size: 16px; }
  nav { display: grid; gap: 5px; }
  nav button { display: grid; grid-template-columns: 22px 1fr auto auto; align-items: center; gap: 8px; width: 100%; padding: 10px 11px; border: 0; border-radius: 10px; color: var(--ink-soft, #66788c); background: transparent; text-align: left; cursor: pointer; }
  nav button:hover, nav button.active { color: #3376bb; background: var(--blue-soft, #e7f1fc); }
  nav button span { font-size: 12px; font-weight: 650; }
  nav button em { min-width: 20px; padding: 2px 5px; border-radius: 999px; background: var(--surface, #fff); font-size: 9px; font-style: normal; text-align: center; }
  .settings-safety { display: flex; align-items: flex-start; gap: 8px; margin-top: auto; padding: 12px 10px; color: var(--ink-muted, #7d8b9b); font-size: 10px; line-height: 1.5; }
  .settings-safety :global(svg) { flex: 0 0 auto; color: #2aa59f; }

  .settings-content { display: grid; min-width: 0; min-height: 0; grid-template-rows: auto 1fr; }
  .settings-header { display: flex; align-items: center; justify-content: space-between; padding: 24px 28px 18px; border-bottom: 1px solid var(--line-soft, #dbe4ee); }
  .settings-header h2 { margin: 0; font-size: 22px; letter-spacing: -.025em; }
  .settings-close { display: grid; width: 34px; height: 34px; place-items: center; border: 0; border-radius: 9px; color: var(--ink-soft, #6d7e90); background: transparent; cursor: pointer; }
  .settings-close:hover { color: var(--ink, #18304b); background: var(--surface-tint, #edf3f8); }
  .settings-scroll { min-height: 0; overflow-y: auto; padding: 26px 30px 34px 28px; scrollbar-width: thin; scrollbar-color: rgb(94 132 171 / 45%) transparent; }
  .settings-scroll::-webkit-scrollbar { width: 8px; }
  .settings-scroll::-webkit-scrollbar-track { background: transparent; }
  .settings-scroll::-webkit-scrollbar-thumb { border: 2px solid transparent; border-radius: 99px; background: rgb(94 132 171 / 42%); background-clip: padding-box; }

  .settings-section { padding: 20px; border: 1px solid var(--line-soft, #dbe4ee); border-radius: 14px; background: var(--surface-raised, #fff); box-shadow: 0 8px 24px rgb(41 71 105 / 5%); }
  .settings-section + .settings-section { margin-top: 18px; }
  .section-heading { display: flex; align-items: center; gap: 11px; margin-bottom: 18px; }
  .section-heading > span { display: grid; width: 34px; height: 34px; flex: 0 0 auto; place-items: center; border-radius: 10px; color: #3f83c6; background: var(--blue-soft, #eaf3fc); }
  .section-heading h3 { margin: 0 0 3px; font-size: 14px; }
  .section-heading p { margin: 0; color: var(--ink-muted, #8090a2); font-size: 10px; line-height: 1.45; }
  .choice-grid { display: grid; gap: 10px; }
  .two-columns { grid-template-columns: repeat(2, 1fr); }
  .three-columns { grid-template-columns: repeat(3, 1fr); }
  .choice-grid button { position: relative; display: grid; gap: 4px; padding: 14px; border: 1px solid var(--line-soft, #dbe4ee); border-radius: 11px; color: var(--ink, #18304b); background: var(--surface, #f8fbff); text-align: left; cursor: pointer; }
  .choice-grid button:hover { border-color: #8eb7de; }
  .choice-grid button.active { border-color: #4a8bca; background: var(--blue-soft, #e9f3fd); box-shadow: inset 0 0 0 1px rgb(74 139 202 / 20%); }
  .choice-grid strong { font-size: 12px; }
  .choice-grid small { color: var(--ink-muted, #8090a2); font-size: 9px; }
  .choice-grid :global(svg) { position: absolute; top: 12px; right: 12px; color: #2e8d88; }

  .mcp-intro { padding-bottom: 16px; }
  .mcp-intro .section-heading > div { min-width: 0; flex: 1; }
  .refresh-clients { display: flex; align-items: center; gap: 6px; border: 0; color: #3479bc; background: transparent; font-size: 10px; font-weight: 650; cursor: pointer; }
  .settings-loading { display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 180px; color: var(--ink-muted, #8090a2); font-size: 11px; }
  .client-list { display: grid; gap: 8px; }
  .client-row { display: grid; grid-template-columns: 20px 36px minmax(0, 1fr) auto; align-items: center; gap: 10px; width: 100%; padding: 11px 12px; border: 1px solid var(--line-soft, #dbe4ee); border-radius: 11px; color: var(--ink, #18304b); background: var(--surface, #f8fbff); text-align: left; cursor: pointer; }
  .client-row:hover:not(:disabled), .client-row.selected { border-color: #75a8d8; background: var(--blue-soft, #eaf3fc); }
  .client-row.unavailable { cursor: default; opacity: .46; }
  .client-check { display: grid; width: 18px; height: 18px; place-items: center; border: 1px solid #9cafc2; border-radius: 5px; color: #fff; }
  .client-row.selected .client-check { border-color: #4387c8; background: #4387c8; }
  .client-icon { display: grid; width: 34px; height: 34px; place-items: center; border-radius: 9px; color: #4b7eae; background: var(--surface-tint, #edf3f8); }
  .client-copy { min-width: 0; }
  .client-copy strong, .client-copy small { display: block; }
  .client-copy strong { margin-bottom: 3px; font-size: 12px; }
  .client-copy small { overflow: hidden; color: var(--ink-muted, #8090a2); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .client-status { padding: 4px 7px; border-radius: 999px; color: var(--ink-muted, #78899b); background: var(--surface-tint, #edf3f8); font-size: 9px; font-weight: 650; }
  .client-status.configured { color: #168a83; background: var(--cyan-soft, #e4f6f3); }
  .install-bar { display: flex; align-items: center; justify-content: space-between; gap: 18px; margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--line-soft, #dbe4ee); }
  .install-bar strong, .install-bar span { display: block; }
  .install-bar strong { margin-bottom: 3px; font-size: 11px; }
  .install-bar span { color: var(--ink-muted, #8090a2); font-size: 9px; }
  .install-button { display: flex; align-items: center; gap: 7px; padding: 10px 14px; border: 0; border-radius: 9px; color: #fff; background: linear-gradient(135deg, #438bca, #29aaa2); font-size: 10px; font-weight: 700; cursor: pointer; }
  .install-button:disabled { cursor: default; filter: grayscale(.45); opacity: .5; }
  .settings-success, .settings-error { margin: 12px 0 0; font-size: 10px; line-height: 1.5; }
  .settings-success { display: flex; align-items: center; gap: 7px; color: #19857f; }
  .settings-error { color: #c3565c; }

  .manual-config { margin-top: 18px; padding: 0 18px; border: 1px solid var(--line-soft, #dbe4ee); border-radius: 12px; background: var(--surface-raised, #fff); }
  .manual-config summary { display: flex; align-items: center; gap: 8px; padding: 15px 0; font-size: 11px; font-weight: 650; cursor: pointer; list-style: none; }
  .manual-config[open] summary :global(svg) { transform: rotate(90deg); }
  .manual-config p { color: var(--ink-muted, #8090a2); font-size: 9px; }
  .manual-config pre { max-height: 170px; overflow: auto; padding: 14px; border-radius: 9px; color: var(--code-ink, #c9daf0); background: var(--code-bg, #132236); font: 9px/1.55 "Cascadia Code", monospace; white-space: pre-wrap; word-break: break-all; }
  .manual-config > button { display: flex; align-items: center; gap: 6px; margin: 0 0 16px auto; border: 0; color: #3479bc; background: transparent; font-size: 10px; font-weight: 650; cursor: pointer; }
  :global(.spinning) { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  @media (max-width: 760px) {
    .settings-backdrop { padding: 12px; }
    .settings-shell { grid-template-columns: 74px minmax(0, 1fr); width: calc(100vw - 24px); }
    .settings-brand > div, nav button span, nav button em, nav button > :global(svg:last-child), .settings-safety span { display: none; }
    .settings-brand { justify-content: center; padding-inline: 0; }
    nav button { display: grid; grid-template-columns: 1fr; place-items: center; }
    .settings-scroll { padding: 20px; }
    .three-columns { grid-template-columns: 1fr; }
  }
</style>
