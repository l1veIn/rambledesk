import { describe, expect, it } from 'vitest'
import type { WorkbenchSpec } from './generated/feedback'
import { resolveWorkbenchPolicy, workbenchIsReadOnly, workbenchSupportsApproval } from './workbenchPolicy'
import { canSubmitWorkbench } from './workbenchState'
import { validTerminalInput } from './workbenchInputValidation'

const action = { id: 'read', instruction: 'Read the result' }
const choice = { id: 'yes', label: 'Yes' }
const option = { value: 'yes', label: 'Yes' }
const question = { id: 'question', prompt: 'Continue?', options: [option, { value: 'no', label: 'No' }], allowOther: true }
const paragraph = { id: 'intro', text: 'Original paragraph' }
const inputs = {
  ramble: { actions: [action] },
  questions: { questions: [question] },
  single_choice: { prompt: 'Choose', options: [choice, { id: 'no', label: 'No' }] },
  document_review: { title: 'Draft', source_version: 'v1', paragraphs: [paragraph] },
}
const spec = (type: keyof typeof inputs, data: object = inputs[type]): WorkbenchSpec => ({ type, version: 1, data: data as Record<string, unknown> })
function rejected(type: keyof typeof inputs, data: object) {
  const input = spec(type, data)
  expect(resolveWorkbenchPolicy(input)).toBeNull()
  expect(workbenchIsReadOnly(input)).toBe(true)
  expect(workbenchSupportsApproval(input)).toBe(false)
  expect(canSubmitWorkbench(input, null, 'Even saved notes cannot make an opaque input editable')).toBe(false)
}

describe('workbench input contract before enabling mutations', () => {
  it('accepts terminal instructions in request materials and keeps legacy command data compatible', () => {
    for (const data of [{ cwd: '/project' }, { cwd: '/project', commands: [] },
      { cwd: '/project', commands: [{ id: 'help', title: 'Help', command: 'my-cli --help' }] }]) {
      expect(validTerminalInput(data)).toBe(true)
      expect(resolveWorkbenchPolicy({ type: 'terminal', version: 1, data })?.type).toBe('terminal')
    }
    for (const commands of [null, 'my-cli --help', [{ id: 'help', title: 'Help', command: 'my-cli --help\n' }]]) {
      expect(validTerminalInput({ cwd: '/project', commands })).toBe(false)
    }
  })
  it.each(Object.keys(inputs) as Array<keyof typeof inputs>)('accepts valid %s input but preserves extra data fields as read-only', (type) => {
    expect(resolveWorkbenchPolicy(spec(type))?.type).toBe(type)
    rejected(type, { ...inputs[type], future: true })
  })

  it('rejects unknown fields at every nested contract boundary', () => {
    rejected('ramble', { actions: [{ ...action, future: true }] })
    rejected('questions', { questions: [{ ...question, future: true }] })
    rejected('questions', { questions: [{ ...question, options: [{ ...option, future: true }, question.options[1]] }] })
    rejected('single_choice', { ...inputs.single_choice, options: [{ ...choice, future: true }, inputs.single_choice.options[1]] })
    rejected('document_review', { ...inputs.document_review, paragraphs: [{ ...paragraph, future: true }] })
  })

  it('rejects empty and oversized collections, duplicate identities, and invalid IDs', () => {
    const contracts = [
      ['ramble', 'actions', action, 1, 20], ['questions', 'questions', question, 1, 20],
      ['single_choice', 'options', choice, 2, 20], ['document_review', 'paragraphs', paragraph, 1, 200],
    ] as const
    for (const [type, key, item, min, max] of contracts) {
      rejected(type, { ...inputs[type], [key]: Array.from({ length: min - 1 }, (_, id) => ({ ...item, id: `id-${id}` })) })
      rejected(type, { ...inputs[type], [key]: Array.from({ length: max + 1 }, (_, id) => ({ ...item, id: `id-${id}` })) })
      rejected(type, { ...inputs[type], [key]: [item, item] })
      for (const id of ['', 'Uppercase', '-first', 'a\n', 'a'.repeat(65)]) {
        const items = [{ ...item, id }, { ...item, id: 'valid' }]
        rejected(type, { ...inputs[type], [key]: items })
      }
    }
  })

  it('counts Unicode scalars and matches Rust visible-text validation, including NUL', () => {
    const fields = [
      ['ramble', (value: string) => ({ actions: [{ ...action, instruction: value }] }), 2000],
      ['questions', (value: string) => ({ questions: [{ ...question, prompt: value }] }), 2000],
      ['single_choice', (value: string) => ({ ...inputs.single_choice, prompt: value }), 2000],
      ['document_review', (value: string) => ({ ...inputs.document_review, title: value }), 200],
    ] as const
    for (const [type, data, max] of fields) {
      expect(resolveWorkbenchPolicy(spec(type, data('😀'.repeat(max))))?.type).toBe(type)
      for (const value of ['', ' \n\t', '\u0085', 'text\0', '😀'.repeat(max + 1), '\uD800']) rejected(type, data(value))
      // BOM is not Unicode White_Space and Rust treats it as visible.
      expect(resolveWorkbenchPolicy(spec(type, data('\uFEFF')))?.type).toBe(type)
    }
  })

  it('validates question option values, counts, labels and nullable optional fields', () => {
    const withQuestion = (value: object) => ({ questions: [value] })
    for (const allowOther of [null, 'true', 1]) rejected('questions', withQuestion({ ...question, allowOther }))
    for (const label of [42, 'x'.repeat(41), '\0']) rejected('questions', withQuestion({ ...question, label }))
    for (const label of [undefined, null, '', ' '.repeat(40)]) {
      expect(resolveWorkbenchPolicy(spec('questions', withQuestion({ ...question, label, allowOther: undefined })))).not.toBeNull()
    }
    for (const options of [[option], Array.from({ length: 7 }, (_, index) => ({ ...option, value: String(index) })), [option, option]]) {
      rejected('questions', withQuestion({ ...question, options }))
    }
    for (const invalid of [{ value: 'x'.repeat(65) }, { value: ' ' }, { label: '' }, { label: 'x'.repeat(2001) },
      { description: 42 }, { description: 'x'.repeat(2001) }, { description: '\0' }]) {
      rejected('questions', withQuestion({ ...question, options: [{ ...option, ...invalid }, question.options[1]] }))
    }
    expect(resolveWorkbenchPolicy(spec('questions', withQuestion({ ...question, options: [
      { value: 'Any visible value 😀', label: 'Label', description: null }, { value: 'second', label: 'Two', description: '' },
    ] })))).not.toBeNull()
  })

  it('validates single-choice labels and document source limits including total scalar length', () => {
    rejected('single_choice', { ...inputs.single_choice, options: [{ ...choice, label: 'x'.repeat(2001) }, inputs.single_choice.options[1]] })
    for (const source_version of ['', ' ', '\0', 'x'.repeat(129)]) rejected('document_review', { ...inputs.document_review, source_version })
    for (const invalid of [{ label: 4 }, { label: 'x'.repeat(129) }, { label: '\0' }, { text: ' ' }, { text: 'x'.repeat(8001) }]) {
      rejected('document_review', { ...inputs.document_review, paragraphs: [{ ...paragraph, ...invalid }] })
    }
    const paragraphs = Array.from({ length: 15 }, (_, index) => ({ id: `p-${index}`, text: '😀'.repeat(8000), label: null }))
    expect(resolveWorkbenchPolicy(spec('document_review', { ...inputs.document_review, paragraphs }))).not.toBeNull()
    rejected('document_review', { ...inputs.document_review, paragraphs: [...paragraphs, { id: 'overflow', text: 'x' }] })
  })

  it('allows approval only for legacy requests and valid Ramble input', () => {
    expect(workbenchSupportsApproval(null)).toBe(true)
    expect(workbenchSupportsApproval(spec('ramble'))).toBe(true)
    for (const type of ['questions', 'single_choice', 'document_review'] as const) expect(workbenchSupportsApproval(spec(type))).toBe(false)
    expect(workbenchSupportsApproval({ ...spec('ramble'), version: 2 })).toBe(false)
  })
})
