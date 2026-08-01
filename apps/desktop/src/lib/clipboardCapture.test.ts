import { describe, expect, it } from 'vitest'

import {
  clipboardCaptureLabel,
  eventBelongsToRamble,
  type ClipboardCaptureEvent,
} from './clipboardCapture'

const event: ClipboardCaptureEvent = {
  type: 'text',
  request_id: 'request-a',
  ramble_session_id: 'ramble-a',
  text: 'copied context',
  captured_at_ms: 0,
  truncated: false,
}

describe('clipboard capture routing', () => {
  it('accepts only events from the active request and Ramble', () => {
    expect(eventBelongsToRamble(event, 'request-a', 'ramble-a')).toBe(true)
    expect(eventBelongsToRamble(event, 'request-b', 'ramble-a')).toBe(false)
    expect(eventBelongsToRamble(event, 'request-a', 'ramble-b')).toBe(false)
  })

  it('marks truncated clipboard context in its visible label', () => {
    expect(clipboardCaptureLabel(0, true)).toContain('内容过长，已截断')
  })
})
