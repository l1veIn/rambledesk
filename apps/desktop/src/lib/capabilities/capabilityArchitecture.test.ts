import { readdir, readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

import { describe, expect, it } from 'vitest'

async function sourceFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true })
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const target = path.join(directory, entry.name)
      if (entry.isDirectory()) return sourceFiles(target)
      return /\.(?:svelte|ts)$/u.test(entry.name) ? [target] : []
    }),
  )
  return nested.flat()
}

function portableRelativePath(from: string, to: string): string {
  return path.relative(from, to).split(path.sep).join('/')
}

describe('capability architecture', () => {
  it('keeps shared views and workbench controllers independent from Tauri detection', async () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
    const sharedRoots = [
      path.join(sourceRoot, 'App.svelte'),
      path.join(sourceRoot, 'lib'),
    ]
    const violations: string[] = []
    const tauriImportMarker = ['@tauri', '-apps'].join('')

    for (const root of sharedRoots) {
      const files = root.endsWith('.svelte') ? [root] : await sourceFiles(root)
      for (const file of files) {
        if (file.includes(`${path.sep}capabilities${path.sep}tauri${path.sep}`)) continue
        if (
          !file.endsWith('.svelte') &&
          !file.includes(`${path.sep}workbench${path.sep}`) &&
          file !== path.join(sourceRoot, 'App.svelte')
        ) {
          continue
        }
        const source = await readFile(file, 'utf8')
        if (
          source.includes(tauriImportMarker) ||
          /__TAURI_INTERNALS__|\bisTauri\b|\binvoke\s*\(/u.test(source)
        ) {
          violations.push(portableRelativePath(sourceRoot, file))
        }
      }
    }

    expect(violations).toEqual([])
  })

  it('freezes the files that may import Tauri directly', async () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
    const expected = [
      'PinnedCapture.svelte',
      'RambleConsole.svelte',
      'ScreenshotOverlay.svelte',
      'ScrollCaptureController.svelte',
      `lib/application/${['tauri', 'ApplicationTransport.test.ts'].join('')}`,
      `lib/application/${['tauri', 'ApplicationTransport.ts'].join('')}`,
      'lib/capabilities/tauri/tauriCapabilityApi.ts',
      'lib/cooking.ts',
      'lib/updater.ts',
      'main.ts',
    ]
    const actual: string[] = []
    const tauriImportMarker = ['@tauri', '-apps'].join('')

    for (const file of await sourceFiles(sourceRoot)) {
      const source = await readFile(file, 'utf8')
      if (source.includes(tauriImportMarker)) actual.push(portableRelativePath(sourceRoot, file))
    }

    expect(actual.sort()).toEqual(expected.sort())
  })

  it('keeps platform capability implementations outside Draft and Application Transport', async () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
    const capabilityRoot = path.join(sourceRoot, 'lib', 'capabilities')
    const platformFiles = [
      ...(await sourceFiles(path.join(capabilityRoot, 'browser'))),
      ...(await sourceFiles(path.join(capabilityRoot, 'tauri'))),
    ]
    const forbidden = [
      'ApplicationTransport',
      'RichFeedbackEditor',
      'draftOperations',
      '@tiptap/',
      'FeedbackWorkspace',
    ]
    const violations: string[] = []

    for (const file of platformFiles) {
      const source = await readFile(file, 'utf8')
      if (forbidden.some((marker) => source.includes(marker))) {
        violations.push(portableRelativePath(sourceRoot, file))
      }
    }

    expect(violations).toEqual([])
  })

  it('keeps the native Speech Plugin independent from Feedback Requests', async () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
    const plugin = await readFile(
      path.join(sourceRoot, 'lib', 'capabilities', 'tauri', 'speechCapability.ts'),
      'utf8',
    )
    const commands = await readFile(
      path.join(sourceRoot, '..', 'src-tauri', 'src', 'commands.rs'),
      'utf8',
    )
    const start = commands.slice(
      commands.indexOf('pub(super) async fn start_voice_ramble'),
      commands.indexOf('pub(super) async fn stop_voice_ramble'),
    )

    expect(plugin).not.toMatch(/requestId|request_id|ApplicationTransport|FeedbackWorkspace/u)
    expect(start).not.toContain(['get', 'feedback', 'workspace'].join('_'))
    expect(start).not.toContain(['Feedback', 'Status'].join(''))
  })

  it('keeps native Capture Plugins independent from request persistence', async () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
    const plugin = await readFile(
      path.join(sourceRoot, 'lib', 'capabilities', 'tauri', 'captureCapabilities.ts'),
      'utf8',
    )
    const clipboard = await readFile(
      path.join(sourceRoot, '..', 'src-tauri', 'src', 'clipboard_capture.rs'),
      'utf8',
    )
    const commands = await readFile(
      path.join(sourceRoot, '..', 'src-tauri', 'src', 'commands.rs'),
      'utf8',
    )
    const composition = await readFile(
      path.join(sourceRoot, '..', 'src-tauri', 'src', 'lib.rs'),
      'utf8',
    )

    expect(plugin).not.toMatch(/requestId|request_id|rambleContextId|expectedRevision/u)
    expect(clipboard).not.toMatch(/request_id|ramble_context_id/u)
    expect(commands).not.toMatch(/add_completed_(?:screen|clipboard)_capture/u)
    expect(composition).not.toMatch(/add_completed_(?:screen|clipboard)_capture/u)
  })
})
