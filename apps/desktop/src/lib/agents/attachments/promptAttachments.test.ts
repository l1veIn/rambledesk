import { describe, expect, it } from 'vitest'
import type { AgentPromptCapabilities } from '$lib/generated/feedback'
import { SessionPromptDrafts } from '../managedSessionUi'
import { attachmentAccept, canAttachFiles, MAX_IMAGE_BYTES, MAX_TEXT_BYTES, readPromptFiles, validatePromptAttachments, type PromptAttachment } from './promptAttachments'

const capabilities: AgentPromptCapabilities = { image: true, audio: false, embedded_context: true, resource_links: true }
const textFile = (text = 'const x = 1', name = 'source.ts') => new File([text], name, { type: 'text/plain' })
const pngFile = () => new File([new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]).buffer], 'image.png', { type: 'image/png' })

describe('typed prompt files', () => {
  it('reads explicit images as base64 and UTF-8 files as embedded resources without fake project paths', async () => {
    const [image, resource] = await readPromptFiles([pngFile(), textFile('你好', '文件 a.ts')], capabilities)
    expect(image.content).toEqual({ type: 'image', mime_type: 'image/png', data: 'iVBORw0KGgo=' })
    expect(resource.name).toBe('文件 a.ts')
    expect(resource.content).toMatchObject({ type: 'resource', mime_type: 'text/plain', text: '你好' })
    if (resource.content.type !== 'resource') throw new Error('Expected resource')
    expect(resource.content.uri).toMatch(/^ramble-attachment:\/\/[a-f0-9-]{36}\/%E6%96%87%E4%BB%B6%20a.ts$/)
    expect(resource.content.uri).not.toContain('file:///')
    expect(() => validatePromptAttachments('Read this', [image, resource], capabilities)).not.toThrow()
  })

  it('checks real formats, negotiated capabilities and per-file bounds before returning any additions', async () => {
    await expect(readPromptFiles([pngFile()], { ...capabilities, image: false })).rejects.toThrow('does not accept image')
    await expect(readPromptFiles([textFile()], { ...capabilities, embedded_context: false })).rejects.toThrow('does not accept text-file')
    await expect(readPromptFiles([new File(['not png'], 'wrong.png', { type: 'image/png' })], capabilities)).rejects.toThrow('Choose a PNG')
    await expect(readPromptFiles([textFile('\u0000binary')], capabilities)).rejects.toThrow('binary data')
    await expect(readPromptFiles([new File([new Uint8Array([0xff, 0xfe]).buffer], 'legacy.txt')], capabilities)).rejects.toThrow('UTF-8')
    await expect(readPromptFiles([new File([new Uint8Array(MAX_IMAGE_BYTES + 1).buffer], 'huge.png')], capabilities)).rejects.toThrow('1.5 MiB')
    await expect(readPromptFiles([textFile('a'.repeat(MAX_TEXT_BYTES + 1))], capabilities)).rejects.toThrow('256 KiB')
    expect(canAttachFiles({ ...capabilities, image: false, embedded_context: false })).toBe(false)
    expect(attachmentAccept({ ...capabilities, embedded_context: false })).not.toContain('text/')
  })

  it('applies aggregate text, block and encoded-content bounds without mutating the draft', async () => {
    const [text] = await readPromptFiles([textFile('x'.repeat(MAX_TEXT_BYTES))], capabilities)
    expect(() => validatePromptAttachments('extra', [text], capabilities)).toThrow('together')
    expect(() => validatePromptAttachments('text', Array(16).fill(text), capabilities)).toThrow('16 content blocks')
    const image: PromptAttachment = { id: 'large', name: 'large.png', detail: '', content: { type: 'image', mime_type: 'image/png', data: 'a'.repeat(2 * 1024 * 1024) } }
    expect(() => validatePromptAttachments('', [image, image], capabilities)).toThrow('4 MiB')
    expect(text.content.type).toBe('resource')
  })
})

describe('attachment draft ownership', () => {
  it('keeps unsent files across session switches and restores text plus attachments after an untouched failure', async () => {
    const drafts = new SessionPromptDrafts()
    const attachments = await readPromptFiles([textFile()], capabilities)
    drafts.write('one', 'Review file')
    drafts.writeAttachments('one', attachments)
    drafts.write('two', 'Another task')
    expect(drafts.readAttachments('one')).toEqual(attachments)
    expect(drafts.readAttachments('two')).toEqual([])
    const submission = drafts.beginSubmission('one', 'Review file')
    expect(drafts.readAttachments('one')).toEqual([])
    expect(submission.attachments).toEqual(attachments)
    expect(drafts.restoreSubmission(submission)).toBe(true)
    expect(drafts.read('one')).toBe('Review file')
    expect(drafts.readAttachments('one')).toEqual(attachments)
    expect(drafts.read('two')).toBe('Another task')
  })

  it('protects newer attachment-only drafts and ignores late file reads into deleted sessions', async () => {
    const drafts = new SessionPromptDrafts()
    const attachments = await readPromptFiles([textFile()], capabilities)
    drafts.writeAttachments('one', attachments)
    const submission = drafts.beginSubmission('one', '')
    const next = await readPromptFiles([pngFile()], capabilities)
    drafts.writeAttachments('one', next)
    expect(drafts.restoreSubmission(submission)).toBe(false)
    expect(drafts.readAttachments('one')).toEqual(next)
    drafts.writeAttachments('one', [])
    expect(drafts.restoreSubmission(submission)).toBe(false)
    drafts.forgetSession('one')
    drafts.writeAttachments('one', attachments)
    expect(drafts.readAttachments('one')).toEqual([])
  })
})
