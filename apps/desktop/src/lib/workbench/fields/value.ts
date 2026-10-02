export const validFieldText = (value: unknown, limit: number): value is string => typeof value === 'string'
  && !value.includes('\0') && [...value].length <= limit

/** Property order is not part of a business contract or anchor identity. */
export function fieldFingerprint(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(fieldFingerprint).join(',')}]`
  if (value && typeof value === 'object') return `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b))
    .map(([key, item]) => `${JSON.stringify(key)}:${fieldFingerprint(item)}`).join(',')}}`
  return JSON.stringify(value) ?? 'undefined'
}
