import { describe, expect, it } from 'vitest'
import { inputFields, inputResponse } from './sessionInputForm'

describe('Agent input forms', () => {
  const schema = { type: 'object', required: ['approach', 'checks', 'ready', 'count'], properties: {
    approach: { type: 'string', title: 'Choose approach', oneOf: [{ const: 'small', title: 'Small change' }, { const: 'full', title: 'Full rewrite' }] },
    checks: { type: 'array', items: { type: 'string', enum: ['unit', 'browser'] }, minItems: 1 },
    ready: { type: 'boolean' }, count: { type: 'integer', minimum: 1, maximum: 3 }, notes: { type: 'string' },
  } }

  it('preserves wire values while presenting titled choices and typed answers', () => {
    const fields = inputFields(schema)
    expect(fields[0].choices).toEqual([{ value: 'small', label: 'Small change', description: '' }, { value: 'full', label: 'Full rewrite', description: '' }])
    expect(inputResponse(fields, { approach: 'small', checks: ['browser'], ready: false, count: '2', notes: '' })).toEqual({
      action: 'accept', content: { approach: 'small', checks: ['browser'], ready: false, count: 2 },
    })
  })

  it('rejects missing answers, invented choices, duplicate selections and invalid numbers', () => {
    const fields = inputFields(schema)
    const answers = { approach: 'small', checks: ['unit'], ready: false, count: '2' }
    for (const change of [{ approach: '' }, { approach: 'invented' }, { checks: [] }, { checks: ['unit', 'unit'] }, { checks: ['fake'] }, { count: 'NaN' }, { count: '2.5' }, { count: '0' }, { ready: '' }]) {
      expect(() => inputResponse(fields, { ...answers, ...change })).toThrow()
    }
  })

  it('allows a custom answer only for agents explicitly supporting it', () => {
    const fields = inputFields({ type: 'object', required: ['direction'], properties: {
      direction: { type: 'string', enum: ['Left', 'Right'], 'x-rambledesk-allow-other': true },
    } })
    expect(inputResponse(fields, { direction: 'Go straight' }).content).toEqual({ direction: 'Go straight' })
  })

  it('renders the titled multiselect schema emitted by Claude and Kimi without an items type', () => {
    const fields = inputFields({ type: 'object', required: ['targets'], properties: {
      targets: { type: 'array', items: { anyOf: [{ const: 'web', title: 'Web app' }, { const: 'desktop', title: 'Desktop app' }] }, minItems: 1 },
    } })
    expect(fields[0].choices.map(choice => choice.label)).toEqual(['Web app', 'Desktop app'])
    expect(inputResponse(fields, { targets: ['web', 'desktop'] }).content).toEqual({ targets: ['web', 'desktop'] })
  })

  it('does not silently submit a partially understood form or a prototype property', () => {
    expect(() => inputFields({ type: 'object', 'x-rambledesk-unsupported': true, properties: { address: { type: 'string', format: 'email' } } })).toThrow()
    expect(() => inputFields({ type: 'object', properties: { nested: { type: 'object', properties: {} } } })).toThrow()
    expect(() => inputFields({ type: 'object', required: ['missing'], properties: {} })).toThrow()
    const fields = inputFields(JSON.parse('{"type":"object","required":["__proto__"],"properties":{"__proto__":{"type":"string"}}}'))
    expect(() => inputResponse(fields, {})).toThrow()
    expect(inputResponse(fields, JSON.parse('{"__proto__":"answer"}')).content).toEqual(JSON.parse('{"__proto__":"answer"}'))
  })
})
