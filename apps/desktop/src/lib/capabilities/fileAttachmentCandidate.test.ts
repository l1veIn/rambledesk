import { describe, expect, it } from 'vitest'

import { fileAttachmentCandidate } from './fileAttachmentCandidate'

describe('file Attachment Candidate projection', () => {
  it.each(['file-input', 'image-paste'] as const)(
    'preserves browser File metadata with the %s source',
    async (source) => {
      const candidate = fileAttachmentCandidate({
        name: 'screen.png',
        type: 'image/png',
        size: 3,
        arrayBuffer: async () => new Uint8Array([1, 2, 3]).buffer,
      }, source)

      expect(candidate).toMatchObject({
        source,
        fileName: 'screen.png',
        mediaType: 'image/png',
        byteLength: 3,
      })
      await expect(candidate.readBytes()).resolves.toEqual(
        new Uint8Array([1, 2, 3]).buffer,
      )
      await expect(candidate.dispose()).resolves.toBeUndefined()
    },
  )
})
