import { describe, expect, it } from 'vitest'

import { acpAdapterErrorMessage } from './adapter'

describe('ACP Workbench adapter errors', () => {
  it('labels a missing native command as unavailable', () => {
    expect(acpAdapterErrorMessage('command read_acp_workbench not found')).toContain('尚未接入')
  })

  it('preserves normal request-not-found errors', () => {
    expect(acpAdapterErrorMessage({
      code: 'REQUEST_NOT_FOUND',
      message: 'the waiting Feedback Request was not found',
    })).toBe('the waiting Feedback Request was not found')
  })

  it('explains a missing npx runtime in user language', () => {
    expect(acpAdapterErrorMessage({
      code: 'ACP_RUNTIME_MISSING',
      message: '/private/runtime/npx was missing',
    })).toContain('安装 Node.js')
    expect(acpAdapterErrorMessage({
      code: 'ACP_RUNTIME_MISSING',
      message: '/private/runtime/npx was missing',
    })).not.toContain('/private/runtime')
  })

  it('distinguishes a protocol handshake failure from installation failure', () => {
    expect(acpAdapterErrorMessage({
      code: 'ACP_PROTOCOL_VIOLATION',
      message: 'initialize response omitted capabilities',
    })).toContain('协议握手失败')
  })
})
