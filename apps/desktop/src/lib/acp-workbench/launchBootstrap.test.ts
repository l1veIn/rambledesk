import { describe, expect, it } from 'vitest'

import { launchBootstrapDocumentJson, launchBootstrapMarkdown } from './launchBootstrap'

describe('Launch Ramble bootstrap', () => {
  it('starts the managed loop without relying on a slash command or installed skill', () => {
    expect(launchBootstrapMarkdown).not.toContain('/ramble')
    expect(launchBootstrapMarkdown).not.toContain('ramble skill')
    expect(launchBootstrapMarkdown).toContain('request_feedback')
    expect(launchBootstrapMarkdown).toContain('goal, relevant context and materials')
    expect(launchBootstrapMarkdown).toContain('End this turn immediately after request_feedback')

    const document = JSON.parse(launchBootstrapDocumentJson) as { type: string; content: unknown[] }
    expect(document.type).toBe('doc')
    expect(JSON.stringify(document.content)).not.toContain('/ramble')
    expect(JSON.stringify(document.content)).toContain('request_feedback')
  })
})
