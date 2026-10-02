// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { VisualFeedbackAnnotation, VisualFeedbackData } from '../../generated/feedback'
import { drawVisualAnnotations, exportVisualPng } from './visualExport'
const data: VisualFeedbackData = { title: 'Sketch', source_version: 'v1', width: 960, height: 600, image_file_name: null, background_color: '#fff8ed' }
const base = { id: 'mark_one', color: '#e5484d', stroke_width: 4, text: '', body: '' }
afterEach(() => vi.restoreAllMocks())
describe('visual PNG composition', () => {
  it('projects known field attachments out of PNG text while keeping unknown links and literal code', () => {
    const fillText = vi.fn(), ctx = { fillText } as unknown as CanvasRenderingContext2D
    const text = 'Move this\n\n[spec.pdf](attachment://known_file)\n\n[Other](attachment://unknown_file)\n`[Code](attachment://known_file)`'
    const annotation: VisualFeedbackAnnotation = { ...base, kind: 'text', points: [{ x: 20, y: 10 }], text }
    drawVisualAnnotations(ctx, [annotation], [{ attachment_id: 'known_file' }])
    const rendered = fillText.mock.calls.map(([line]) => line).join('\n')
    expect(rendered).not.toContain('[spec.pdf]'); expect(rendered).toContain('[Other](attachment://unknown_file)')
    expect(rendered).toContain('`[Code](attachment://known_file)`'); expect(rendered).toContain('Move this')
    expect(annotation.text).toBe(text)
  })
  it('exports at original resolution with the requested background and typed annotation geometry', async () => {
    const calls: string[] = [], ctx = { fillStyle: '', strokeStyle: '', lineWidth: 0, font: '', textBaseline: '',
      fillRect: vi.fn(() => calls.push('background')), strokeRect: vi.fn(() => calls.push('rectangle')), fillText: vi.fn(() => calls.push('text')),
      beginPath: vi.fn(), moveTo: vi.fn(), lineTo: vi.fn(), stroke: vi.fn(() => calls.push('stroke')) } as unknown as CanvasRenderingContext2D
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(ctx)
    const bytes = new ArrayBuffer(8)
    const encode = vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function(this: HTMLCanvasElement, callback, type) {
      expect(this.width).toBe(960); expect(this.height).toBe(600); expect(type).toBe('image/png')
      callback({ arrayBuffer: async () => bytes } as Blob)
    })
    const annotations: VisualFeedbackAnnotation[] = [{ ...base, kind: 'rectangle', points: [{ x: 200, y: 100 }, { x: 20, y: 10 }] },
      { ...base, id: 'text_one', kind: 'text', points: [{ x: 20, y: 10 }], stroke_width: 24, text: 'First\nSecond' }]
    expect(await exportVisualPng(data, annotations, null)).toBe(bytes)
    expect(calls).toEqual(['background', 'rectangle', 'text', 'text'])
    expect(ctx.strokeRect).toHaveBeenCalledWith(20, 10, 180, 90)
    expect(ctx.fillText).toHaveBeenNthCalledWith(2, 'Second', 20, 40)
    expect(encode).toHaveBeenCalledOnce()
  })
  it('draws arrow heads and freehand segments and refuses to silently replace an unavailable image', async () => {
    const ctx = { beginPath: vi.fn(), moveTo: vi.fn(), lineTo: vi.fn(), stroke: vi.fn() } as unknown as CanvasRenderingContext2D
    drawVisualAnnotations(ctx, [{ ...base, kind: 'arrow', points: [{ x: 20, y: 30 }, { x: 100, y: 100 }] },
      { ...base, id: 'pen_one', kind: 'freehand', points: [{ x: 1, y: 2 }, { x: 10, y: 20 }, { x: 20, y: 15 }] }])
    expect(ctx.stroke).toHaveBeenCalledTimes(3); expect(ctx.lineTo).toHaveBeenCalledWith(20, 15)
    await expect(exportVisualPng({ ...data, image_file_name: 'original.png' }, [], null)).rejects.toThrow('unavailable')
  })
  it('reports oversized PNGs before reading or uploading their bytes without changing annotations', async () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ fillRect: vi.fn() } as unknown as CanvasRenderingContext2D)
    const arrayBuffer = vi.fn()
    vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation((callback) => {
      callback({ size: 20 * 1024 * 1024 + 1, arrayBuffer } as unknown as Blob)
    })
    const annotations: VisualFeedbackAnnotation[] = []
    await expect(exportVisualPng(data, annotations, null)).rejects.toThrow('create a new request with a smaller image or canvas')
    expect(arrayBuffer).not.toHaveBeenCalled()
    expect(annotations).toEqual([])
  })
})
