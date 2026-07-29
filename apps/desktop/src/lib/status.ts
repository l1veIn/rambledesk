import type { HealthSnapshot } from './generated/health'

export function statusLabel(health: HealthSnapshot | null): string {
  if (!health) return '正在连接桌面核心…'
  if (health.status === 'ready') return 'MCP 核心已就绪'
  return '状态未知'
}
