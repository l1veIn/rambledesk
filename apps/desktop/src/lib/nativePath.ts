const WINDOWS_VERBATIM_PREFIX = '\\\\?\\'
const WINDOWS_VERBATIM_UNC_PREFIX = '\\\\?\\UNC\\'

export function desktopPath(value: string): string {
  if (value.startsWith(WINDOWS_VERBATIM_UNC_PREFIX)) {
    return `\\\\${value.slice(WINDOWS_VERBATIM_UNC_PREFIX.length)}`
  }
  if (value.startsWith(WINDOWS_VERBATIM_PREFIX)) {
    return value.slice(WINDOWS_VERBATIM_PREFIX.length)
  }
  return value
}
