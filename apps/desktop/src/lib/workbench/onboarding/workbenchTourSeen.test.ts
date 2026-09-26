import { describe, expect, it } from 'vitest'
import { createWorkbenchTourSeenStore } from './workbenchTourSeen'

function memoryStorage() {
  const values = new Map<string, string>()
  return {
    values,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value) },
  }
}

describe('workbench tour completion', () => {
  it('only persists explicit completion and keeps types independent across reloads', () => {
    const storage = memoryStorage()
    storage.values.set('rambledesk.onboarding.completed', 'true')
    const firstSession = createWorkbenchTourSeenStore(() => storage)
    expect(firstSession.hasSeen('ramble', 1)).toBe(false)
    expect(storage.values.size).toBe(1)
    firstSession.markSeen('ramble', 1)
    const nextSession = createWorkbenchTourSeenStore(() => storage)
    expect(nextSession.hasSeen('ramble', 1)).toBe(true)
    expect(nextSession.hasSeen('document_review', 1)).toBe(false)
    nextSession.markSeen('document_review', 1)
    expect(nextSession.hasSeen('ramble', 1)).toBe(true)
    expect(storage.values.get('rambledesk.onboarding.completed')).toBe('true')
  })

  it('allows a new guide version to run without repeating the completed version', () => {
    const storage = memoryStorage()
    const seen = createWorkbenchTourSeenStore(() => storage)
    seen.markSeen('ramble', 1)
    expect(seen.hasSeen('ramble', 1)).toBe(true)
    expect(seen.hasSeen('ramble', 2)).toBe(false)
    seen.markSeen('ramble', 2)
    expect(createWorkbenchTourSeenStore(() => storage).hasSeen('ramble', 2)).toBe(true)
  })

  it.each(['{', 'null', 'true', '[]', '1', '{"version":"1"}', '{"version":0.5}', '{"version":0}'])('ignores invalid saved data: %s', (saved) => {
    const storage = memoryStorage()
    storage.values.set('rambledesk.workbench-tour.ramble', saved)
    expect(createWorkbenchTourSeenStore(() => storage).hasSeen('ramble', 1)).toBe(false)
  })

  it('falls back to session memory when storage access itself is blocked', () => {
    const seen = createWorkbenchTourSeenStore(() => { throw new Error('Storage blocked') })
    expect(seen.hasSeen('ramble', 1)).toBe(false)
    expect(() => seen.markSeen('ramble', 1)).not.toThrow()
    expect(seen.hasSeen('ramble', 1)).toBe(true)
    expect(seen.hasSeen('document_review', 1)).toBe(false)
  })

  it('remembers explicit completion when writing is rejected or storage is absent', () => {
    for (const storage of [null, { getItem: () => null, setItem: () => { throw new Error('Quota exceeded') } }]) {
      const seen = createWorkbenchTourSeenStore(() => storage)
      seen.markSeen('document_review', 1)
      expect(seen.hasSeen('document_review', 1)).toBe(true)
      expect(seen.hasSeen('document_review', 2)).toBe(false)
    }
  })
})
