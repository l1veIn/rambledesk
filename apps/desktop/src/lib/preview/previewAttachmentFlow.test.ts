import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from '../capabilities/unavailableCapabilities'
import { PreviewApplicationTransport } from './previewApplicationTransport'
import { previewFixtures } from './previewFixtures'

const png = () => Uint8Array.from(readFileSync(new URL('../../../../../playground/workbenches/materials/visual-background.png', import.meta.url))).buffer
const requestId = previewFixtures.requests[0]!.request_id
const otherId = previewFixtures.requests[1]!.request_id

describe('binary attachment preview flow', () => {
  it('loads the unique image example as request bytes and preserves the blank default', async () => {
    const image = new PreviewApplicationTransport(UNAVAILABLE_CAPABILITY_MANIFEST, { workspace: 'visual_feedback-image' })
    const workspace = await image.call('getFeedbackWorkspace', { request_id: requestId })
    expect(workspace.workbench?.data).toMatchObject({ width: 640, height: 400, image_file_name: 'visual-background.png' })
    const attachment = workspace.request_attachments[0]!
    expect(attachment).toMatchObject({ media_type: 'image/png', byte_size: png().byteLength })
    const input = { request_id: requestId, attachment_id: attachment.attachment_id }
    const bytes = await image.call('readRequestAttachment', input)
    expect(new Uint8Array(bytes)).toEqual(new Uint8Array(png()))
    new Uint8Array(bytes).fill(0)
    expect(new Uint8Array(await image.call('readRequestAttachment', input))).toEqual(new Uint8Array(png()))
    await expect(image.call('readRequestAttachment', { ...input, request_id: otherId })).rejects.toThrow()
    const blank = new PreviewApplicationTransport(UNAVAILABLE_CAPABILITY_MANIFEST, { workspace: 'visual_feedback' })
    expect((await blank.call('getFeedbackWorkspace', { request_id: requestId })).workbench?.data).toMatchObject({ image_file_name: null })
  })

  it('retains generated PNG bytes, prevents foreign reads and does not reuse removed IDs', async () => {
    const preview = new PreviewApplicationTransport(UNAVAILABLE_CAPABILITY_MANIFEST, { workspace: 'visual_feedback' })
    const bytes = png()
    const first = await preview.call('addFeedbackAttachment', {
      request_id: requestId, file_name: 'visual-feedback.png', contents: bytes, expected_revision: 0,
    })
    const attachment = first.attachments[0]!
    expect(attachment).toMatchObject({ media_type: 'image/png', byte_size: bytes.byteLength })
    new Uint8Array(bytes).fill(0)
    const input = { request_id: requestId, attachment_id: attachment.attachment_id }
    const read = await preview.call('readFeedbackAttachment', input)
    expect(new Uint8Array(read)).toEqual(new Uint8Array(png()))
    new Uint8Array(read).fill(0)
    expect(new Uint8Array(await preview.call('readFeedbackAttachment', input))).toEqual(new Uint8Array(png()))
    await expect(preview.call('readFeedbackAttachment', { ...input, request_id: otherId })).rejects.toThrow()
    await preview.call('removeFeedbackAttachment', { ...input, expected_revision: 0 })
    await expect(preview.call('readFeedbackAttachment', input)).rejects.toThrow()
    const next = await preview.call('addFeedbackAttachment', {
      request_id: requestId, file_name: 'visual-feedback.png', contents: png(), expected_revision: 0,
    })
    expect(next.attachments[0]!.attachment_id).not.toBe(attachment.attachment_id)
    await preview.call('submitFeedback', { request_id: requestId, expected_revision: 0 })
    const published = await preview.call('readPublishedFeedback', { request_id: requestId })
    expect(published?.manifest.attachments).toMatchObject([{ id: next.attachments[0]!.attachment_id, media_type: 'image/png' }])
    await preview.call('deleteFeedbackRequest', { request_id: requestId })
    await expect(preview.call('readFeedbackAttachment', { request_id: requestId, attachment_id: next.attachments[0]!.attachment_id })).rejects.toThrow()
  })
})
