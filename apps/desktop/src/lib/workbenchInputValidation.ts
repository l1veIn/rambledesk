// Mirror the strict immutable-input contract in core workbenches/validation.rs.
// Invalid known input arrives as opaque data too, so shape checks alone cannot
// distinguish an editable request from preserved historical input.
type RecordValue = Record<string, unknown>
const object = (value: unknown): value is RecordValue => !!value && typeof value === 'object' && !Array.isArray(value)
const fields = (value: RecordValue, allowed: readonly string[]) => Object.keys(value).every((key) => allowed.includes(key))
const list = (value: unknown, min: number, max: number): value is unknown[] => Array.isArray(value) && value.length >= min && value.length <= max

function text(value: unknown, max: number, visible = true): value is string {
  if (typeof value !== 'string' || value.includes('\0')) return false
  const scalars = [...value]
  return scalars.length <= max && scalars.every((char) => !/^[\uD800-\uDFFF]$/.test(char)) &&
    (!visible || /[^\p{White_Space}]/u.test(value))
}
const optionalText = (value: unknown, max: number) => value == null || text(value, max, false)
function uniqueId(value: unknown, seen: Set<string>): value is string {
  if (typeof value !== 'string' || value.length > 64 || !/^[a-z0-9]/.test(value) || /[^a-z0-9_-]/.test(value) || seen.has(value)) return false
  seen.add(value)
  return true
}

export function validRambleInput(data: RecordValue): boolean {
  const ids = new Set<string>()
  return fields(data, ['actions']) && list(data.actions, 1, 20) && data.actions.every((action) =>
    object(action) && fields(action, ['id', 'instruction']) && uniqueId(action.id, ids) && text(action.instruction, 2000))
}

export function validQuestionsInput(data: RecordValue): boolean {
  const ids = new Set<string>()
  return fields(data, ['questions']) && list(data.questions, 1, 20) && data.questions.every((question) => {
    if (!object(question) || !fields(question, ['id', 'prompt', 'label', 'options', 'allowOther']) ||
      !uniqueId(question.id, ids) || !text(question.prompt, 2000) || !optionalText(question.label, 40) ||
      (question.allowOther !== undefined && typeof question.allowOther !== 'boolean') || !list(question.options, 2, 6)) return false
    const values = new Set<string>()
    return question.options.every((option) => {
      if (!object(option) || !fields(option, ['value', 'label', 'description']) || !text(option.value, 64) ||
        values.has(option.value) || !text(option.label, 2000) || !optionalText(option.description, 2000)) return false
      values.add(option.value)
      return true
    })
  })
}

export function validSingleChoiceInput(data: RecordValue): boolean {
  const ids = new Set<string>()
  return fields(data, ['prompt', 'options']) && text(data.prompt, 2000) && list(data.options, 2, 20) &&
    data.options.every((option) => object(option) && fields(option, ['id', 'label']) && uniqueId(option.id, ids) && text(option.label, 2000))
}

export function validDocumentReviewInput(data: RecordValue): boolean {
  if (!fields(data, ['title', 'source_version', 'paragraphs']) || !text(data.title, 200) || !text(data.source_version, 128) ||
    !list(data.paragraphs, 1, 200)) return false
  const ids = new Set<string>()
  let total = 0
  return data.paragraphs.every((paragraph) => {
    if (!object(paragraph) || !fields(paragraph, ['id', 'label', 'text']) || !uniqueId(paragraph.id, ids) ||
      !text(paragraph.text, 8000) || !optionalText(paragraph.label, 128)) return false
    total += [...paragraph.text].length
    return total <= 120000
  })
}

export function validWebReviewUrl(value: unknown): value is string {
  if (!text(value, 8192) || /[\s\\\u0000-\u001f\u007f-\u009f]/.test(value) || !/^https?:\/\//i.test(value)) return false
  try {
    const url = new URL(value)
    return !!url.hostname && !url.username && !url.password
  } catch { return false }
}

export function validWebReviewInput(data: RecordValue): boolean {
  const viewport = data.viewport
  return fields(data, ['title', 'url', 'source_version', 'viewport']) && text(data.title, 200)
    && text(data.source_version, 128) && validWebReviewUrl(data.url) && object(viewport)
    && fields(viewport, ['width', 'height']) && Number.isInteger(viewport.width) && Number.isInteger(viewport.height)
    && (viewport.width as number) >= 240 && (viewport.width as number) <= 7680
    && (viewport.height as number) >= 200 && (viewport.height as number) <= 4320
}

export function validTerminalInput(data: RecordValue): boolean {
  const ids = new Set<string>()
  return fields(data, ['cwd', 'shell', 'commands']) && text(data.cwd, 8192)
    && (data.shell == null || text(data.shell, 8192)) && list(data.commands, 1, 20)
    && data.commands.every((command) => object(command) && fields(command, ['id', 'title', 'command', 'description'])
      && uniqueId(command.id, ids) && text(command.title, 200) && text(command.command, 4000)
      && !/[\u0000-\u001f\u007f-\u009f]/.test(command.command) && optionalText(command.description, 2000))
}
