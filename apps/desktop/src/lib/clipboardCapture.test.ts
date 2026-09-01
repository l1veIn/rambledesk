import { describe, expect, it } from 'vitest'

import { clipboardCaptureLabel } from './clipboardCapture'

describe('clipboard capture routing', () => {
  it('marks truncated clipboard context in its visible label', () => {
    expect(clipboardCaptureLabel(0, true)).toContain('内容过长，已截断')
  })
})
