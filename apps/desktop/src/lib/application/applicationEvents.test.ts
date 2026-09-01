import { describe, expect, it } from 'vitest'

import type { ApplicationEvent } from '../generated/feedback'
import {
  APPLICATION_EVENT_CREDENTIAL_PROTOCOL_PREFIX,
  APPLICATION_EVENT_PROTOCOL,
  APPLICATION_EVENTS_STREAM,
  REVISION_HEADER,
  RUNTIME_GENERATION_HEADER,
  applicationEventRevision,
  isRuntimeGenerationStaleError,
} from './applicationEvents'

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

  it('defines transport-only metadata and stale generation errors', () => {
    expect(RUNTIME_GENERATION_HEADER).toBe('X-RambleDesk-Runtime-Generation')
    expect(REVISION_HEADER).toBe('X-RambleDesk-Revision')
    expect(APPLICATION_EVENT_PROTOCOL).toBe('rambledesk-events')
    expect(APPLICATION_EVENT_CREDENTIAL_PROTOCOL_PREFIX).toBe('rambledesk-session.')
    expect(
      isRuntimeGenerationStaleError({
        code: 'RUNTIME_GENERATION_STALE',
        message: 'refetch',
        retryable: false,
      }),
    ).toBe(true)
  })
})
