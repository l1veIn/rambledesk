import { describe, expect, it } from 'vitest'

import { statusLabel } from './status'

describe('statusLabel', () => {
  it('describes the generated ready state', () => {
    expect(
      statusLabel({
        serviceName: 'rambledesk',
        serviceVersion: '0.1.0',
        status: 'ready',
        storage: 'not_initialized',
      }),
    ).toBe('MCP 核心已就绪')
  })

  it('describes the loading state', () => {
    expect(statusLabel(null)).toBe('正在连接桌面核心…')
  })
})
