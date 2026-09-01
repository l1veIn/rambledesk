import { describe, expect, it } from 'vitest'

import { publishedFeedbackDownload } from './publishedFeedbackDownload'

describe('publishedFeedbackDownload', () => {
  it('exports only the safe published manifest and Markdown projection', () => {
    const download = publishedFeedbackDownload('request/one', {
      manifest: {
        schema_version: 1,
        request_id: 'request/one',
        title: 'Request one',
        host_id: 'codex',
        host_session_id: 'session-a',
        source_hint: null,
        submitted_at: '2026-09-01T00:00:00Z',
        source_revision: 1,
        draft_revision: 1,
        feedback_markdown: 'feedback.md',
        feedback_sha256: 'sha256',
        attachments: [{
          id: 'image',
          file_name: 'image.png',
          media_type: 'image/png',
          byte_size: 42,
          sha256: 'attachment-sha256',
          path: 'attachments/image.png',
        }],
      },
      markdown: '# Published feedback',
    })

    expect(download.fileName).toBe('request_one.rambledesk-feedback.json')
    expect(download.contents).toContain('# Published feedback')
    expect(download.contents).toContain('attachments/image.png')
    expect(download.contents).not.toMatch(/directory_path|markdown_path|manifest_path|file:\/\//u)
  })
})
