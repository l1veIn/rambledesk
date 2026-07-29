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
  <title>RambleDesk · M0</title>
</svelte:head>

<main>
  <section class="hero">
    <p class="eyebrow">RAMBLEDESK · M0 FOUNDATION</p>
    <h1>Agent 呼叫你做真实体验反馈。</h1>
    <p class="lede">
      当前版本只验证桌面壳、workspace 边界和本地 MCP。反馈工作流将在 M1 接入。
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
      <strong>{health?.storage === 'not_initialized' ? '等待 M1' : health?.storage ?? '—'}</strong>
      <small>业务数据尚未启用</small>
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
