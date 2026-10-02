import { describe, expect, it } from 'vitest'
import { getWorkbenchTour } from './workbenchTours'
import { getWorkbenchDefinition } from '../definitions/registry'
import type { WorkbenchDefinition } from '../definitions/contracts'

describe('workbench guides', () => {
  it('requires declared, nonempty guide metadata', () => {
    for (const definition of [getWorkbenchDefinition('questions'), getWorkbenchDefinition('sort'), { type: 'future-workbench' }, { type: 'empty', guide: { version: 1, steps: [] } }, undefined, null]) {
      expect(getWorkbenchTour(definition, 'zh-CN')).toBeNull()
    }
  })

  it('uses an arbitrary definition identity, guide version and bilingual steps without registration', () => {
    const definition: Pick<WorkbenchDefinition, 'type' | 'guide'> = {
      type: 'custom-review',
      guide: {
        version: 3,
        steps: [{ id: 'source', target: '[data-custom-source]', title: ['查看材料', 'Read the source'], body: ['在这里查看', 'Read here'] }],
      },
    }
    expect(getWorkbenchDefinition(definition.type)).toBeUndefined()
    const chinese = getWorkbenchTour(definition, 'zh-CN')!
    const english = getWorkbenchTour(definition, 'en')!
    expect(chinese.id).toBe('custom-review')
    expect(chinese.version).toBe(3)
    expect(chinese.steps).toEqual([{ id: 'source', target: '[data-custom-source]', title: '查看材料', body: '在这里查看' }])
    expect(english.steps).toEqual([{ id: 'source', target: '[data-custom-source]', title: 'Read the source', body: 'Read here' }])
    expect(definition.guide!.steps[0].title).toEqual(['查看材料', 'Read the source'])
  })

  it.each(['ramble', 'document_review', 'web_review', 'terminal', 'visual_feedback', 'diff_review'])('keeps %s step identity stable across locale changes', (type) => {
    const definition = getWorkbenchDefinition(type)!
    const chinese = getWorkbenchTour(definition, 'zh-CN')!
    const english = getWorkbenchTour(definition, 'en')!
    expect(chinese.id).toBe(type)
    expect(chinese.version).toBe(1)
    expect(chinese.steps).toHaveLength(5)
    expect(chinese.steps.map(({ id, target }) => ({ id, target })))
      .toEqual(english.steps.map(({ id, target }) => ({ id, target })))
    expect(new Set(chinese.steps.map(({ id }) => id)).size).toBe(chinese.steps.length)
    expect(chinese.labels.step(0, 5)).toBe('第 1 / 5 步')
    expect(english.labels.step(0, 5)).toBe('Step 1 of 5')
    expect(chinese.replayLabel).toBe('使用引导')
    expect(english.replayLabel).toBe('Show guide')
  })
})
