<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'

  import type { HealthSnapshot } from './lib/generated/health'
  import { statusLabel } from './lib/status'

  let health: HealthSnapshot | null = null
  let endpoint = '尚未连接桌面桥'
  let error = ''

  onMount(async () => {
    try {
      ;[health, endpoint] = await Promise.all([
        invoke<HealthSnapshot>('get_health'),
        invoke<string>('get_mcp_endpoint'),
      ])
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause)
    }
  })
</script>

<svelte:head>
  <title>RambleDesk · M1</title>
</svelte:head>

<main>
  <section class="hero">
    <p class="eyebrow">RAMBLEDESK · M1 REQUEST KERNEL</p>
    <h1>Agent 呼叫你做真实体验反馈。</h1>
    <p class="lede">
      请求会先持久化，再由 Agent 轮询状态；断线或重启不会让反馈任务消失。
    </p>
  </section>

  <section class="status-grid" aria-label="运行状态">
    <article>
      <span>Core</span>
      <strong>{statusLabel(health)}</strong>
      <small>{health?.serviceVersion ?? '—'}</small>
    </article>
    <article>
      <span>Storage</span>
      <strong>{health?.storage === 'ready' ? 'SQLite 已就绪' : health?.storage ?? '—'}</strong>
      <small>persistent request source of truth</small>
    </article>
    <article>
      <span>MCP endpoint</span>
      <strong class="endpoint">{endpoint}</strong>
      <small>loopback · bearer token required</small>
    </article>
  </section>

  {#if error}
    <p class="error">桌面桥不可用：{error}</p>
  {/if}
</main>
