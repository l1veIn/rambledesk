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
    const expectedRoles: Record<string, string> = {
      'PinnedCapture.svelte': 'pinned-capture platform window',
      'RambleConsole.svelte': 'ramble-console platform window',
      'ScreenshotOverlay.svelte': 'screen-capture platform window',
      'ScrollCaptureController.svelte': 'scroll-capture platform window',
      [`lib/application/${['tauri', 'ApplicationTransport.test.ts'].join('')}`]:
        'Application Transport adapter test',
      [`lib/application/${['tauri', 'ApplicationTransport.ts'].join('')}`]:
        'Application Transport adapter',
      'lib/capabilities/tauri/tauriCapabilityApi.ts':
        'Native Capability API composition root',
      'lib/cooking.ts': 'desktop HTTP implementation for Cooking',
      'lib/desktop-shell/instrumentation.ts':
        'Desktop Shell instrumentation and DevTools implementation',
      'lib/updater.ts': 'desktop software-update implementation',
    }
    const actual: string[] = []
    const tauriImportMarker = ['@tauri', '-apps'].join('')

    for (const file of await sourceFiles(sourceRoot)) {
      const source = await readFile(file, 'utf8')
      if (source.includes(tauriImportMarker)) actual.push(portableRelativePath(sourceRoot, file))
    }

    expect(actual.sort()).toEqual(Object.keys(expectedRoles).sort())
    expect(Object.values(expectedRoles).every((role) => role.length > 0)).toBe(true)
  })

  it('freezes literal Tauri command ownership by platform role', async () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
    const expectedOwners: Record<
      string,
      Readonly<{ role: string; commands: readonly string[] }>
    > = {
      'PinnedCapture.svelte': {
        role: 'pinned-capture platform window',
        commands: ['close_pinned_screen_capture', 'read_pinned_screen_capture'],
      },
      'ScreenshotOverlay.svelte': {
        role: 'screen-capture platform window',
        commands: [
          'begin_scrolling_capture',
          'cancel_screen_capture',
          'complete_screen_capture',
          'get_active_capture_info',
          'pin_screen_capture',
          'read_capture_rgba_bytes',
          'show_screen_capture_overlay',
        ],
      },
      'ScrollCaptureController.svelte': {
        role: 'scroll-capture platform window',
        commands: [
          'append_scrolling_capture_frame',
          'cancel_screen_capture',
          'finish_scrolling_capture',
          'get_scrolling_capture_info',
        ],
      },
      'lib/capabilities/tauri/administrationCapabilities.ts': {
        role: 'Native Administration Capability implementations',
        commands: [
          'copy_web_access_token',
          'detect_generic_mcp_hosts',
          'export_diagnostics',
          'get_data_storage_settings',
          'get_generic_mcp_configuration',
          'get_pi_package_status',
          'install_dsh_package',
          'install_generic_mcp_hosts',
          'install_pi_package',
          'list_macos_permissions',
          'open_macos_privacy_settings',
          'open_web_access',
          'request_macos_permission',
          'set_data_storage_path',
          'uninstall_pi_package',
        ],
      },
      'lib/capabilities/tauri/captureCapabilities.ts': {
        role: 'Native Capture Plugin implementations',
        commands: [
          'begin_screen_capture',
          'capture_clipboard_once',
          'discard_clipboard_capture_image',
          'discard_screen_capture',
          'read_clipboard_capture_image',
          'read_completed_screen_capture',
        ],
      },
      'lib/capabilities/tauri/navigationCapabilities.ts': {
        role: 'Native navigation and server-path Capability implementations',
        commands: [
          'import_feedback_attachment_path',
          'open_feedback_attachment',
          'reveal_feedback_attachment',
          'reveal_path_in_folder',
          'set_pending_count',
        ],
      },
      'lib/capabilities/tauri/notificationCapability.ts': {
        role: 'Native Notification Capability implementation',
        commands: [
          'commit_notification_sound',
          'import_notification_sound',
          'read_notification_sound',
          'remove_notification_sound',
        ],
      },
      'lib/capabilities/tauri/publishedFeedbackAction.ts': {
        role: 'Native Published Feedback Capability implementation',
        commands: ['reveal_feedback_package'],
      },
      'lib/capabilities/tauri/rambleConsoleCapability.ts': {
        role: 'Native Ramble Console Capability implementation',
        commands: [
          'hide_ramble_console',
          'record_diagnostic_event',
          'show_ramble_console',
        ],
      },
      'lib/capabilities/tauri/shortcutCapability.ts': {
        role: 'Native Shortcut Capability implementation',
        commands: [
          'get_shortcut_settings',
          'reset_shortcut_settings',
          'set_shortcut_capture_active',
          'set_shortcut_setting',
        ],
      },
      'lib/capabilities/tauri/speechCapability.ts': {
        role: 'Native Speech Plugin implementation',
        commands: [
          'delete_speech_model',
          'download_speech_model',
          'list_speech_input_devices',
          'list_speech_models',
          'start_voice_ramble',
          'stop_voice_ramble',
        ],
      },
      'lib/capabilities/tauri/windowCapability.ts': {
        role: 'Native Window Capability implementation',
        commands: ['restart_application'],
      },
      'lib/desktop-shell/instrumentation.ts': {
        role: 'Desktop Shell instrumentation and DevTools implementation',
        commands: ['log_frontend_error', 'open_main_devtools'],
      },
    }
    const literalInvoke =
      /(?:\b|\.)invoke(?:<[^\n(]+>)?\(\s*(['"])([^'"]+)\1/gu
    const actualOwners: Record<string, string[]> = {}

    for (const file of await sourceFiles(sourceRoot)) {
      if (file.endsWith('.test.ts')) continue
      const source = await readFile(file, 'utf8')
      const commands = [...source.matchAll(literalInvoke)].map((match) => match[2]!)
      if (commands.length === 0) continue
      actualOwners[portableRelativePath(sourceRoot, file)] = [...new Set(commands)].sort()
    }

    expect(actualOwners).toEqual(
      Object.fromEntries(
        Object.entries(expectedOwners).map(([file, owner]) => [
          file,
          [...owner.commands].sort(),
        ]),
      ),
    )
    expect(Object.values(expectedOwners).every(({ role }) => role.length > 0)).toBe(true)
  })

  it('limits dynamic Tauri command dispatch to typed transport and capability adapters', async () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
    const expectedRoles: Record<string, string> = {
      [`lib/application/${['tauri', 'ApplicationTransport.ts'].join('')}`]:
        'typed Application Transport command map',
      'lib/capabilities/tauri/administrationCapabilities.ts':
        'typed Web Access lifecycle command union',
    }
    const dynamicInvoke =
      /(?:\b|\.)invoke(?:<[^\n(]+>)?\(\s*(?!['"])([A-Za-z_$][\w$]*)(?=\s*[,\)])/gu
    const actual: string[] = []

    for (const file of await sourceFiles(sourceRoot)) {
      if (file.endsWith('.test.ts')) continue
      const relative = portableRelativePath(sourceRoot, file)
      if (relative === 'lib/capabilities/tauri/tauriCapabilityApi.ts') continue
      const source = await readFile(file, 'utf8')
      if (dynamicInvoke.test(source)) actual.push(relative)
      dynamicInvoke.lastIndex = 0
    }

    expect(actual.sort()).toEqual(Object.keys(expectedRoles).sort())
    expect(Object.values(expectedRoles).every((role) => role.length > 0)).toBe(true)
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
