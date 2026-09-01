import { describe, expect, it, vi } from 'vitest'

import { createTauriPublishedFeedbackAction } from './publishedFeedbackAction'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

describe('Tauri published feedback action', () => {
  it('reveals the durable package through its native capability boundary', async () => {
    const invoke = vi.fn(async () => undefined)
    const action = createTauriPublishedFeedbackAction({
      invoke: invoke as TauriCapabilityApi['invoke'],
    })

    await action.run('request-1')

    expect(action.label).toBe('Open feedback package')
    expect(invoke).toHaveBeenCalledWith('reveal_feedback_package', {
      input: { request_id: 'request-1' },
    })
  })
})
