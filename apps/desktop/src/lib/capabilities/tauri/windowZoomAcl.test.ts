import { readdir, readFile } from 'node:fs/promises'
import { describe, expect, it } from 'vitest'

describe('workbench zoom permission', () => {
  it('grants webview zoom only to the main window', async () => {
    const directory = new URL('../../../../src-tauri/capabilities/', import.meta.url)
    const grantingWindows: unknown[] = []
    for (const file of await readdir(directory)) {
      if (!file.endsWith('.json')) continue
      const capability = JSON.parse(await readFile(new URL(file, directory), 'utf8'))
      if (capability.permissions.some((permission: string | { identifier: string }) =>
        (typeof permission === 'string' ? permission : permission.identifier) === 'core:webview:allow-set-webview-zoom')) {
        grantingWindows.push(capability.windows)
      }
    }
    expect(grantingWindows).toEqual([['main']])
  })
})
