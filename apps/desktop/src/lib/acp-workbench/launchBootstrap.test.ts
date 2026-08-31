import { describe, expect, it } from 'vitest'

import { launchBootstrapDocumentJson, launchBootstrapMarkdown } from './launchBootstrap'

describe('Launch Ramble bootstrap', () => {
  it('asks the Agent to request the human intent before starting work', () => {
    expect(launchBootstrapMarkdown).toContain('request_feedback')
    expect(launchBootstrapMarkdown).toContain('ask the human what they want to work on')
    expect(launchBootstrapMarkdown).toContain('Do not guess their intent or start work')

    const document = JSON.parse(launchBootstrapDocumentJson) as { type: string; content: unknown[] }
    expect(document.type).toBe('doc')
    expect(document.content).toHaveLength(2)
  })
})
