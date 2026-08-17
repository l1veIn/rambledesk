export type DesktopPlatform = 'macOS' | 'Windows' | 'Linux'

export function detectDesktopPlatform(platform: string, userAgent: string): DesktopPlatform {
  const identity = `${platform} ${userAgent}`
  if (/Mac|iPhone|iPad/i.test(identity)) return 'macOS'
  if (/Win/i.test(identity)) return 'Windows'
  return 'Linux'
}

export function currentDesktopPlatform(): DesktopPlatform {
  const navigator = globalThis.navigator
  return detectDesktopPlatform(navigator?.platform ?? '', navigator?.userAgent ?? '')
}
