import { expect, it } from 'vitest'
import { workbenchDefinitions, resolveWorkbenchDefinition, getWorkbenchDefinition } from './registry'
import { canSubmitWorkbench } from '../../workbenchState'
import { findWorkbenchExample, workbenchExampleKey } from './examples'

it('registers each type/version once and keeps every bundled example under its own definition', () => {
  const identities = workbenchDefinitions.map((definition) => `${definition.type}:${definition.version}`)
  expect(new Set(identities).size).toBe(identities.length)
  const orders: number[] = []
  const exampleKeys: string[] = []
  for (const definition of workbenchDefinitions) {
    expect(getWorkbenchDefinition(definition.type, definition.version)).toBe(definition)
    expect(typeof definition.loadView).toBe('function')
    for (const example of definition.examples ?? []) {
      expect(resolveWorkbenchDefinition(example.spec)).toBe(definition)
      const key = workbenchExampleKey(example)
      expect(findWorkbenchExample(key)).toBe(example)
      exampleKeys.push(key)
      orders.push(example.order)
    }
  }
  expect(new Set(exampleKeys).size).toBe(exampleKeys.length)
  // Localized/alternate examples may appear between the stable primary examples.
  expect(orders.filter(Number.isInteger).sort((a,b) => a-b).slice(0,6)).toEqual([0,1,2,3,4,5])
})

it('preserves legacy, unknown version and incomplete structured-input behavior without loading a view', () => {
  expect(resolveWorkbenchDefinition(null)?.type).toBe('ramble')
  expect(resolveWorkbenchDefinition({type:'questions',version:999,data:{}})).toBeNull()
  expect(resolveWorkbenchDefinition({type:'future_review',version:1,data:{}})).toBeNull()
  const spec = workbenchDefinitions.find((definition) => definition.type === 'questions')!.examples![0].spec
  expect(canSubmitWorkbench(spec, null, 'Notes cannot replace required answers.')).toBe(false)
})
