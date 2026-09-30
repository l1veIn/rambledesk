// Mirror the strict immutable-input contract in core workbenches/validation.rs.
// Invalid known input arrives as opaque data too, so shape checks alone cannot
// distinguish an editable request from preserved historical input.
export type RecordValue = Record<string, unknown>
export const object = (value: unknown): value is RecordValue => !!value && typeof value === 'object' && !Array.isArray(value)
export const fields = (value: RecordValue, allowed: readonly string[]) => Object.keys(value).every((key) => allowed.includes(key))
export const list = (value: unknown, min: number, max: number): value is unknown[] => Array.isArray(value) && value.length >= min && value.length <= max

export function text(value: unknown, max: number, visible = true): value is string {
  if (typeof value !== 'string' || value.includes('\0')) return false
  const scalars = [...value]
  return scalars.length <= max && scalars.every((char) => !/^[\uD800-\uDFFF]$/.test(char)) &&
    (!visible || /[^\p{White_Space}]/u.test(value))
}
export const optionalText = (value: unknown, max: number) => value == null || text(value, max, false)
export function uniqueId(value: unknown, seen: Set<string>): value is string {
  if (typeof value !== 'string' || value.length > 64 || !/^[a-z0-9]/.test(value) || /[^a-z0-9_-]/.test(value) || seen.has(value)) return false
  seen.add(value)
  return true
}

