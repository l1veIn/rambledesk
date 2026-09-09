import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'

import { createCookingSession } from './cookingSession'

describe('cooking session', () => {
  it('tracks which requests are cooking', () => {
    const session = createCookingSession()
    session.setCooking('request-1', true)
    expect(session.isCooking('request-1')).toBe(true)
    expect(session.isCooking('request-2')).toBe(false)
    expect(session.isCooking(null)).toBe(false)
    expect(get(session).cookingRequestIds.has('request-1')).toBe(true)
  })

  it('stops cooking a request without touching the others', () => {
    const session = createCookingSession()
    session.setCooking('request-1', true)
    session.setCooking('request-2', true)
    session.setCooking('request-1', false)
    expect(session.isCooking('request-1')).toBe(false)
    expect(session.isCooking('request-2')).toBe(true)
  })

  it('keeps the same state when the preview is set to its current value', () => {
    const session = createCookingSession()
    const preview = { requestId: 'request-1', savedRevision: 1, markdown: 'cooked', original: 'raw', model: 'test-model' }
    session.setPreview(preview)
    const state = get(session)
    session.setPreview(preview)
    expect(get(session)).toBe(state)
    expect(session.preview()).toEqual(preview)
  })

  it('discards the preview when cleared', () => {
    const session = createCookingSession()
    session.setPreview({ requestId: 'request-1', savedRevision: 1, markdown: 'cooked', original: 'raw', model: 'test-model' })
    session.setPreview(null)
    expect(session.preview()).toBeNull()
  })
})
