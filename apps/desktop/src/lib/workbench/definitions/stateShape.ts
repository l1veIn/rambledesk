export const record = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value)
export const text = (value: unknown): value is string => typeof value === 'string'
export const nullableText = (value: unknown): value is string | null => value === null || text(value)
export const nullableInteger = (value: unknown): value is number | null => value === null || Number.isInteger(value)
