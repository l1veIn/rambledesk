import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import StartupRecoveryPanel from './StartupRecoveryPanel.svelte'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

describe('startup recovery rendering', () => {
  it('keeps an error and recovery actions visible without a workspace or backend', () => {
    const { body } = render(StartupRecoveryPanel, { props: {
      capabilities: createUnavailableWorkbenchCapabilities(),
      message: 'Database schema 20 is newer than supported schema 10.', onRetry: vi.fn(),
    } })
    expect(body).toContain('role="alert"')
    expect(body).toContain('RambleDesk could not load')
    expect(body).toContain('Database schema 20 is newer than supported schema 10.')
    expect(body).toContain('Retry loading')
    expect(body).toContain('Reload app')
    expect(body).toContain('Settings and diagnostics')
    expect(body).not.toContain('aria-busy="true"')
  })

  it('distinguishes timeout from data errors and escapes backend details', () => {
    const { body } = render(StartupRecoveryPanel, { props: {
      capabilities: createUnavailableWorkbenchCapabilities(), message: '<script>secret</script>', timedOut: true,
      onRetry: vi.fn(),
    } })
    expect(body).toContain('Loading timed out')
    expect(body).not.toContain('<script>secret</script>')
  })
})
