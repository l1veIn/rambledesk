import { describe, expect, it } from 'vitest'
import type { VisualFeedbackAnnotation, VisualFeedbackData } from '../../generated/feedback'
import { canvasPoint, createVisualHistory, emptyVisualState, validVisualAnnotations, visualFeedbackState } from './visualModel'
import { visualFeedbackDefinition } from '../definitions/visual_feedback/definition'
import { visualAnnotationField } from '../definitions/visual_feedback/fields'
import { workbenchFieldTarget } from '../../domain/inputTarget'

const visualData: VisualFeedbackData = { title: 'Sketch', source_version: 'snapshot-1', width: 960, height: 600, image_file_name: null }
const arrowMark: VisualFeedbackAnnotation = { id: 'mark_one', kind: 'arrow', points: [{ x: 20, y: 30 }, { x: 200, y: 180 }], color: '#e5484d', stroke_width: 4, text: '', body: '' }
describe('visual feedback draft contract', () => {
  it('allows an absent state to prepare notes-only feedback without accepting a foreign state', () => {
    const spec = { type: 'visual_feedback', version: 1, data: visualData }
    expect(visualFeedbackDefinition.complete(spec, null)).toBe(true)
    expect(visualFeedbackDefinition.hasInput(spec, null)).toBe(false)
    expect(visualFeedbackDefinition.complete({ ...spec, data: {} }, null)).toBe(false)
    expect(visualFeedbackDefinition.complete(spec, { type: 'questions', answers: [] })).toBe(false)
  })
  it('keeps original pixel coordinates independent of zoom and clips pointer positions', () => {
    const rect = { left: 10, top: 20, width: 480, height: 300 }
    expect(canvasPoint(110, 120, rect, visualData)).toEqual({ x: 200, y: 200 })
    expect(canvasPoint(-100, 1000, rect, visualData)).toEqual({ x: 0, y: 600 })
  })
  it('preserves empty text drafts but requires visible text at submission', () => {
    const text = { ...arrowMark, kind: 'text' as const, points: [{ x: 10, y: 10 }], text: '' }
    const state = { ...emptyVisualState(), annotations: [text] }
    const spec = { type: 'visual_feedback', version: 1, data: visualData }
    expect(visualFeedbackState(state)).toEqual(state)
    expect(visualFeedbackDefinition.complete(spec, state)).toBe(false)
    expect(visualFeedbackDefinition.complete(spec, { ...state, annotations: [{ ...text, text: 'Move this' }] })).toBe(true)
  })
  it.each([['\u0085', false], ['\ufeff', true]] as const)('uses Rust Unicode whitespace visibility for text %j and preserves optional comment whitespace', (value, visible) => {
    const annotation = { ...arrowMark, kind: 'text' as const, points: [{ x: 10, y: 10 }], text: value, body: value }
    const state = { ...emptyVisualState(), annotations: [annotation] }, spec = { type: 'visual_feedback', version: 1, data: visualData }
    expect(visualFeedbackState(state)).toEqual(state)
    expect(visualFeedbackDefinition.hasInput(spec, state)).toBe(true)
    expect(visualFeedbackDefinition.complete(spec, state)).toBe(visible)
    expect(visualFeedbackDefinition.complete(spec, { ...state, annotations: [{ ...annotation, text: 'Guide' }] })).toBe(true)
  })
  it('rejects degenerate shapes, invalid coordinates, duplicate IDs, foreign fields and oversized evidence', () => {
    expect(validVisualAnnotations([arrowMark], 960, 600)).toBe(true)
    expect(validVisualAnnotations([{ ...arrowMark, points: [{ x: 1.5, y: 2 }, { x: 20, y: 10 }] }])).toBe(false)
    expect(validVisualAnnotations([{ ...arrowMark, points: [{ x: 1, y: 2 }, { x: 1, y: 2 }] }])).toBe(false)
    expect(validVisualAnnotations([{ ...arrowMark, kind: 'rectangle', points: [{ x: 1, y: 2 }, { x: 1, y: 20 }] }])).toBe(false)
    expect(validVisualAnnotations([arrowMark, arrowMark])).toBe(false)
    expect(validVisualAnnotations([{ ...arrowMark, unknown: true }])).toBe(false)
    expect(validVisualAnnotations([{ ...arrowMark, points: [{ x: 961, y: 2 }, { x: 20, y: 10 }] }], 960, 600)).toBe(false)
    const strokes = Array.from({ length: 10 }, (_, index) => ({ ...arrowMark, id: `pen_${index}`, kind: 'freehand', points: Array.from({ length: 2048 }, () => ({ x: 1, y: 2 })) }))
    expect(validVisualAnnotations(strokes)).toBe(false)
  })
  it('undoes edits without losing recovered annotations and discards redo after a new edit', () => {
    const history = createVisualHistory([arrowMark]), edited = { ...arrowMark, body: 'Move it' }
    history.commit([edited]); history.recover([edited])
    expect(history.undo()).toEqual([arrowMark]); expect(history.redo()).toEqual([edited])
    history.undo(); history.commit([{ ...arrowMark, color: '#000000' }]); expect(history.redo()).toBeNull()
    history.recover([edited]); expect(history.canUndo).toBe(false)
  })
  it('routes body/text edits to one stable annotation and invalidates the generated image', () => {
    const spec = { type: 'visual_feedback', version: 1, data: visualData }
    const state = { ...emptyVisualState(), annotations: [arrowMark], composite_attachment_id: 'generated_png' }
    const target = workbenchFieldTarget({ requestId: 'request', requestTitle: 'Sketch' },
      { workbenchType: 'visual_feedback', version: 1, field: 'body', entityId: arrowMark.id, sourceVersion: visualData.source_version, label: 'Comment' })
    expect(visualAnnotationField.accepts(target)).toBe(true)
    const field = visualAnnotationField.read({ spec, state, target })
    expect(field.replace('Move it')).toMatchObject({ composite_attachment_id: null, annotations: [{ body: 'Move it' }] })
    expect(() => visualAnnotationField.read({ spec: { ...spec, data: { ...visualData, source_version: 'new' } }, state, target })).toThrow('changed')
    expect(visualAnnotationField.removeAttachment({ ...emptyVisualState(), composite_attachment_id: 'generated_png' }, 'generated_png', (text) => text)).toEqual(emptyVisualState())
  })
})
