import { describe, expect, it } from 'vitest'

import {
  clampCaptureRectangle,
  getAnnotationBounds,
  hitTestAnnotation,
  normalizeCaptureRectangle,
  resizeAnnotation,
  resizeCaptureRectangle,
  translateAnnotation,
  type CaptureAnnotation,
} from './screenCapture'

describe('advanced screen capture geometry', () => {
  it('normalizes reverse-direction region selections', () => {
    expect(normalizeCaptureRectangle({ x: 90, y: 70 }, { x: 20, y: 10 })).toEqual({
      x: 20,
      y: 10,
      width: 70,
      height: 60,
    })
  })

  it('keeps moved and resized capture regions inside the source image', () => {
    expect(clampCaptureRectangle({ x: 95, y: -5, width: 30, height: 40 }, 100, 80)).toEqual({
      x: 70,
      y: 0,
      width: 30,
      height: 40,
    })
    expect(
      resizeCaptureRectangle(
        { x: 20, y: 20, width: 40, height: 30 },
        'nw',
        { x: -40, y: -20 },
        100,
        80,
      ),
    ).toEqual({ x: 0, y: 0, width: 60, height: 50 })
  })

  it('hit-tests arrows close to their visible stroke', () => {
    const arrow: CaptureAnnotation = {
      id: 'arrow',
      type: 'arrow',
      start: { x: 10, y: 10 },
      end: { x: 90, y: 10 },
      color: '#f00',
      strokeWidth: 4,
    }
    expect(hitTestAnnotation(arrow, { x: 50, y: 13 }, 3)).toBe(true)
    expect(hitTestAnnotation(arrow, { x: 50, y: 30 }, 3)).toBe(false)
  })

  it('translates every point in freehand annotations', () => {
    const pen: CaptureAnnotation = {
      id: 'pen',
      type: 'pen',
      points: [
        { x: 2, y: 3 },
        { x: 7, y: 11 },
      ],
      color: '#fff',
      strokeWidth: 2,
    }
    expect(translateAnnotation(pen, { x: 5, y: -2 })).toMatchObject({
      points: [
        { x: 7, y: 1 },
        { x: 12, y: 9 },
      ],
    })
  })

  it('resizes shape annotations and text without losing their edit metadata', () => {
    const text: CaptureAnnotation = {
      id: 'text',
      type: 'text',
      point: { x: 10, y: 20 },
      text: 'hello',
      fontSize: 20,
      color: '#fff',
      strokeWidth: 4,
    }
    const bounds = getAnnotationBounds(text)
    const resized = resizeAnnotation(text, bounds, {
      x: bounds.x,
      y: bounds.y,
      width: bounds.width * 2,
      height: bounds.height * 2,
    })
    expect(resized).toMatchObject({ id: 'text', type: 'text', text: 'hello', fontSize: 40 })
  })
})
