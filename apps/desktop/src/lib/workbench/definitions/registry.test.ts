import { expect, it } from 'vitest'
import { workbenchDefinitions, resolveWorkbenchDefinition, getWorkbenchDefinition } from './registry'
import { canSubmitWorkbench } from '../../workbenchState'

it('registers each type/version once and keeps every bundled example under its own definition', () => {
  const identities = workbenchDefinitions.map((definition) => `${definition.type}:${definition.version}`)
  expect(new Set(identities).size).toBe(identities.length)
  const orders: number[] = []
  for (const definition of workbenchDefinitions) {
    expect(getWorkbenchDefinition(definition.type, definition.version)).toBe(definition)
    expect(typeof definition.loadView).toBe('function')
    for (const example of definition.examples ?? []) {
      expect(resolveWorkbenchDefinition(example.spec)).toBe(definition)
      orders.push(example.order)
    }
  }
  expect(orders.sort((a,b) => a-b).slice(0,6)).toEqual([0,1,2,3,4,5])
})

it('preserves legacy, unknown version and incomplete structured-input behavior without loading a view', () => {
  expect(resolveWorkbenchDefinition(null)?.type).toBe('ramble')
  expect(resolveWorkbenchDefinition({type:'questions',version:999,data:{}})).toBeNull()
  expect(resolveWorkbenchDefinition({type:'future_review',version:1,data:{}})).toBeNull()
  const spec = workbenchDefinitions.find((definition) => definition.type === 'questions')!.examples![0].spec
  expect(canSubmitWorkbench(spec, null, 'Notes cannot replace required answers.')).toBe(false)
})
