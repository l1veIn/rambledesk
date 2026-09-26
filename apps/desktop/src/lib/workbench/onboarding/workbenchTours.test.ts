import { describe, expect, it } from 'vitest'
import { getWorkbenchTour } from './workbenchTours'

describe('workbench guides', () => {
  it('does not create a guide for unrelated or future workbench types', () => {
    for (const type of ['questions', 'future-workbench', '', undefined, null]) {
      expect(getWorkbenchTour(type, 'zh-CN')).toBeNull()
    }
  })

  it.each(['ramble', 'document_review'])('keeps %s step identity stable across locale changes', (type) => {
    const chinese = getWorkbenchTour(type, 'zh-CN')!
    const english = getWorkbenchTour(type, 'en')!
    expect(chinese.steps.map(({ id, target }) => ({ id, target })))
      .toEqual(english.steps.map(({ id, target }) => ({ id, target })))
    expect(new Set(chinese.steps.map(({ id }) => id)).size).toBe(chinese.steps.length)
    expect(chinese.labels.step(0, 5)).toBe('第 1 / 5 步')
    expect(english.labels.step(0, 5)).toBe('Step 1 of 5')
    expect(chinese.replayLabel).toBe('使用引导')
    expect(english.replayLabel).toBe('Show guide')
  })
})
