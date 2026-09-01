import { describe, expect, it } from 'vitest'

import type { ApplicationEvent } from '../generated/feedback'
import { APPLICATION_EVENTS_STREAM, applicationEventRevision } from './applicationEvents'

describe('application event contracts', () => {
  it('uses a stable stream identity and decimal revisions', () => {
    const event: ApplicationEvent = {
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '9007199254740993',
      resources: [{ kind: 'navigation' }],
    }

    expect(APPLICATION_EVENTS_STREAM.id).toBe('rambledesk://application-events')
    expect(applicationEventRevision(event)).toBe(9_007_199_254_740_993n)
  })
})
