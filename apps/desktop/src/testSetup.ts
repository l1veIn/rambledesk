/**
 * Browser globals that production modules read at import time.
 *
 * Vitest runs in the node environment, so `preferences.ts` would throw while it
 * resolves the locale and the initial UI state. Tests that need their own values
 * still call `vi.stubGlobal`, which overrides these.
 */
const storage = new Map<string, string>()

Object.defineProperty(globalThis, 'localStorage', {
  configurable: true,
  value: {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
    removeItem: (key: string) => storage.delete(key),
    clear: () => storage.clear(),
  },
})

Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: {
    language: 'en-US',
    // TipTap's platform detection reads this at editor construction.
    userAgent: 'node',
    platform: 'node',
  },
})
