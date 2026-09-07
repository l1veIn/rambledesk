import type { SessionInputResponse } from '$lib/generated/feedback'

/** Application form subset: flat text, titled string choices, choice arrays,
 * boolean and numeric fields with required/bounds checks. Unsupported constraints
 * remain visible for decline/cancel; the adapter validates every accepted answer. */
type JsonObject = Record<string, unknown>
export type InputChoice = { value: string; label: string; description: string }
export type InputField = {
  id: string; label: string; description: string; required: boolean
  type: 'string' | 'array' | 'boolean' | 'number' | 'integer'
  choices: InputChoice[]; allowOther: boolean; schema: JsonObject
}
function object(value: unknown): JsonObject {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('This question format is not supported.')
  return value as JsonObject
}
function text(value: unknown, fallback = '') { return typeof value === 'string' ? value : fallback }
export function inputSchema(raw: string): JsonObject {
  try { return object(JSON.parse(raw)) }
  catch { throw new Error('This question format is not supported.') }
}
function choices(schema: JsonObject): InputChoice[] {
  const titled = schema.oneOf ?? schema.anyOf
  if (Array.isArray(titled)) return titled.map(item => {
    const option = object(item)
    if (typeof option.const !== 'string') throw new Error('This question format is not supported.')
    return { value: option.const, label: text(option.title, option.const), description: text(option.description) }
  })
  if (Array.isArray(schema.enum)) return schema.enum.map((value, index) => {
    if (typeof value !== 'string') throw new Error('This question format is not supported.')
    return { value, label: Array.isArray(schema.enumNames) ? text(schema.enumNames[index], value) : value, description: '' }
  })
  return []
}

/** Render only the flat form types supported by the ACP input bridge. */
export function inputFields(schema: JsonObject): InputField[] {
  if (schema.type !== 'object' || schema['x-rambledesk-unsupported'] === true) throw new Error('This question format is not supported.')
  const properties = object(schema.properties)
  const required = Array.isArray(schema.required) ? schema.required : []
  if (required.some(id => typeof id !== 'string' || !Object.hasOwn(properties, id))) throw new Error('This question format is not supported.')
  return Object.entries(properties).map(([id, value]) => {
    const field = object(value)
    if (!['string', 'array', 'boolean', 'number', 'integer'].includes(String(field.type))) throw new Error('This question format is not supported.')
    const options = field.type === 'array' ? choices(object(field.items)) : choices(field)
    if (field.type === 'array' && (!options.length || (object(field.items).type !== undefined && object(field.items).type !== 'string'))) throw new Error('This question format is not supported.')
    return { id, label: text(field.title, id), description: text(field.description), required: required.includes(id),
      type: field.type as InputField['type'], choices: options, allowOther: field['x-rambledesk-allow-other'] === true, schema: field }
  })
}

/** Construct the original request's typed response; never use chat/prompt for answers. */
export function inputResponse(fields: InputField[], values: JsonObject): SessionInputResponse {
  const content: JsonObject = Object.create(null)
  for (const field of fields) {
    let value = Object.hasOwn(values, field.id) ? values[field.id] : undefined
    const missing = value === undefined || value === '' || (Array.isArray(value) && value.length === 0)
    if (missing) {
      if (field.required) throw new Error('Complete all required answers.')
      continue
    }
    if (field.type === 'number' || field.type === 'integer') {
      value = typeof value === 'string' && value.trim() ? Number(value) : value
      if (typeof value !== 'number' || !Number.isFinite(value) || (field.type === 'integer' && !Number.isInteger(value))
        || (typeof field.schema.minimum === 'number' && value < field.schema.minimum)
        || (typeof field.schema.maximum === 'number' && value > field.schema.maximum)) throw new Error('Enter a valid number within the requested range.')
    } else if (field.type === 'boolean') {
      if (typeof value !== 'boolean') throw new Error('Complete all required answers.')
    } else {
      const answers = field.type === 'array' ? value : [value]
      if (!Array.isArray(answers) || answers.some(answer => typeof answer !== 'string') || new Set(answers).size !== answers.length) throw new Error('Choose a valid answer.')
      if (field.choices.length && !field.allowOther && answers.some(answer => !field.choices.some(choice => choice.value === answer))) throw new Error('Choose a valid answer.')
      const length = field.type === 'array' ? answers.length : [...String(value)].length
      const minimum = field.schema[field.type === 'array' ? 'minItems' : 'minLength']
      const maximum = field.schema[field.type === 'array' ? 'maxItems' : 'maxLength']
      if ((typeof minimum === 'number' && length < minimum) || (typeof maximum === 'number' && length > maximum)) throw new Error('The answer does not meet the requested length.')
    }
    content[field.id] = value
  }
  return { action: 'accept', content_json: JSON.stringify(content) }
}
