import { describe, expect, it } from 'vitest'

import type { CaptureAnnotation } from '../screenCapture'
import {
  cloneAnnotations,
  commitAnnotations,
  counterAnnotation,
  createDraftAnnotation,
  deleteAnnotation,
  emptyAnnotationHistory,
  nextCounterNumber,
  pushUndoSnapshot,
  redoAnnotations,
  replaceAnnotations,
  shouldCommitDraft,
  textAnnotation,
  undoAnnotations,
  updateAnnotationAppearance,
} from './annotationModel'

const style = { color: '#ff0000', strokeWidth: 4 }
const point = { x: 10, y: 10 }
const second = { x: 40, y: 40 }

describe('annotation model', () => {
  it('creates a draft per tool with the current style', () => {
    expect(createDraftAnnotation('arrow', point, second, style, 'id')).toMatchObject({
      id: 'id',
      type: 'arrow',
      start: point,
      end: second,
      color: '#ff0000',
      strokeWidth: 4,
    })
    expect(createDraftAnnotation('pen', point, second, style, 'id')).toMatchObject({
      type: 'pen',
      points: [point, second],
    })
    expect(createDraftAnnotation('rectangle', point, second, style, 'id')).toMatchObject({
      type: 'rectangle',
      rect: { x: 10, y: 10, width: 30, height: 30 },
    })
    expect(createDraftAnnotation('mosaic', point, second, style, 'id')).toMatchObject({
      type: 'mosaic',
      pixelSize: 12,
    })
    expect(createDraftAnnotation('select', point, second, style, 'id')).toBeNull()
  })

  it('numbers counters after the highest existing counter', () => {
    expect(nextCounterNumber([])).toBe(1)
    expect(nextCounterNumber([counterAnnotation(point, 3, style)])).toBe(4)
  })

  it('commits, undoes, and redoes annotation lists', () => {
    const first = textAnnotation(point, 'one', style, 'a')
    const committed = commitAnnotations(emptyAnnotationHistory, [first])
    expect(committed.annotations).toEqual([first])
    expect(committed.undoStack).toHaveLength(1)
    expect(committed.redoStack).toHaveLength(0)

    const undone = undoAnnotations(committed)
    expect(undone.annotations).toEqual([])
    expect(undone.redoStack).toEqual([[first]])

    const redone = redoAnnotations(undone)
    expect(redone.annotations).toEqual([first])
    expect(redone.undoStack).toEqual([[]])
  })

  it('leaves the history untouched when there is nothing to undo or redo', () => {
    expect(undoAnnotations(emptyAnnotationHistory)).toBe(emptyAnnotationHistory)
    expect(redoAnnotations(emptyAnnotationHistory)).toBe(emptyAnnotationHistory)
  })

  it('replaces a live gesture list without pushing an undo entry', () => {
    const committed = commitAnnotations(emptyAnnotationHistory, [
      textAnnotation(point, 'one', style, 'a'),
    ])
    const moved = replaceAnnotations(committed, [
      textAnnotation(second, 'one', style, 'a'),
    ])
    expect(moved.undoStack).toBe(committed.undoStack)
    expect(moved.annotations[0]).toMatchObject({ point: second })
  })

  it('pushes a gesture snapshot as one undo entry', () => {
    const snapshot = [textAnnotation(point, 'before', style, 'a')]
    const pushed = pushUndoSnapshot(emptyAnnotationHistory, snapshot)
    expect(pushed.undoStack).toEqual([snapshot])
    expect(pushed.redoStack).toEqual([])
  })

  it('deletes an annotation and updates the selected appearance', () => {
    const a = textAnnotation(point, 'one', style, 'a')
    const b = textAnnotation(second, 'two', style, 'b')
    const history = commitAnnotations(emptyAnnotationHistory, [a, b])

    expect(deleteAnnotation(history, 'a').annotations).toEqual([b])
    expect(updateAnnotationAppearance(history, 'b', { color: '#00ff00' }).annotations[1]).toMatchObject({
      color: '#00ff00',
      text: 'two',
    })
  })

  it('only commits drafts with a visible size', () => {
    const sized = createDraftAnnotation('rectangle', point, second, style, 'id')
    const flat = createDraftAnnotation('rectangle', point, point, style, 'id')
    expect(shouldCommitDraft(sized, 3)).toBe(true)
    expect(shouldCommitDraft(flat, 3)).toBe(false)
    expect(shouldCommitDraft(null, 3)).toBe(false)
  })

  it('clones annotation lists by value', () => {
    const annotations: CaptureAnnotation[] = [textAnnotation(point, 'one', style, 'a')]
    const clone = cloneAnnotations(annotations)
    expect(clone).toEqual(annotations)
    expect(clone).not.toBe(annotations)
    expect(clone[0]).not.toBe(annotations[0])
  })
})
