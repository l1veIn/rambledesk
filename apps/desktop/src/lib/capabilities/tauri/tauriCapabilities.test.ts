import { describe, expect, it, vi } from 'vitest'

import { CAPABILITY_NAMES } from '../capabilityManifest'
import { createTauriWorkbenchCapabilities } from './index'
import type {
  TauriCapabilityApi,
  TauriEvent,
  TauriUnlisten,
} from './tauriCapabilityApi'

type FakeApi = TauriCapabilityApi & Readonly<{
  invokeMock: ReturnType<typeof vi.fn>
  listenMock: ReturnType<typeof vi.fn>
  emitMock: ReturnType<typeof vi.fn>
  window: ReturnType<typeof fakeWindow>
  webview: ReturnType<typeof fakeWebview>
}>

function fakeWindow() {
  return {
    isMaximized: vi.fn(async () => false),
    isFullscreen: vi.fn(async () => true),
    setFullscreen: vi.fn(async () => undefined),
    minimize: vi.fn(async () => undefined),
    toggleMaximize: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
    startDragging: vi.fn(async () => undefined),
    onResized: vi.fn(async () => vi.fn()),
    onFocusChanged: vi.fn(async () => vi.fn()),
  }
}

function fakeWebview() {
  return {
    onDragDropEvent: vi.fn(
      async (
        _handler: Parameters<ReturnType<TauriCapabilityApi['currentWebview']>['onDragDropEvent']>[0],
      ): Promise<TauriUnlisten> => () => undefined,
    ),
  }
}

function createFakeApi(responses: Record<string, unknown> = {}): FakeApi {
  const window = fakeWindow()
  const webview = fakeWebview()
  const invokeMock = vi.fn(
    async (command: string, args?: Record<string, unknown>) => {
      if (command === 'start_voice_ramble' && responses[command] === undefined) {
        const input = args?.input as { recognition_session_id?: string } | undefined
        return {
          recognition_session_id: input?.recognition_session_id,
          provider: 'sense_voice',
          model_path: '/models/sense-voice',
        }
      }
      return responses[command]
    },
  )
  const listenMock = vi.fn(async () => vi.fn())
  const emitMock = vi.fn(async () => undefined)
  return {
    invoke: invokeMock as TauriCapabilityApi['invoke'],
    listen: listenMock as TauriCapabilityApi['listen'],
    emitTo: emitMock,
    currentWindow: () => window,
    currentWebview: () => webview,
    choosePath: vi.fn(async () => null),
    savePath: vi.fn(async () => null),
    notificationPermissionGranted: vi.fn(async () => false),
    requestNotificationPermission: vi.fn(async () => 'default' as const),
    sendNotification: vi.fn(),
    openUrl: vi.fn(async () => undefined),
    getVersion: vi.fn(async () => '1.2.3'),
    checkForUpdates: vi.fn(async () => undefined),
    installUpdate: vi.fn(async () => undefined),
    restartAfterUpdate: vi.fn(async () => undefined),
    invokeMock,
    listenMock,
    emitMock,
    window,
    webview,
  }
}

describe('Tauri Workbench capabilities', () => {
  it('derives native slots plus the shared browser image-paste slot', () => {
    const capabilities = createTauriWorkbenchCapabilities(createFakeApi())
    expect(Object.keys(capabilities.manifest).sort()).toEqual([...CAPABILITY_NAMES].sort())
    for (const name of CAPABILITY_NAMES) {
      expect(capabilities[name].status).toEqual({
        availability: 'available',
        source: name === 'imagePaste' ? 'browser' : 'native',
      })
      expect(capabilities.manifest[name]).toEqual(capabilities[name].status)
    }
  })

  it('preserves window and notification behavior', async () => {
    const api = createFakeApi({ read_notification_sound: [1, 2, 255] })
    const capabilities = createTauriWorkbenchCapabilities(api)

    await capabilities.windowControls.implementation.leaveFullscreen()
    expect(api.window.isFullscreen).toHaveBeenCalledOnce()
    expect(api.window.setFullscreen).toHaveBeenCalledWith(false)
    await capabilities.windowControls.implementation.restart()
    expect(api.invokeMock).toHaveBeenCalledWith('restart_application')

    expect(await capabilities.notifications.implementation.permission()).toBe('default')
    const bytes = await capabilities.notifications.implementation.readCustomSound('sound-1')
    expect([...new Uint8Array(bytes)]).toEqual([1, 2, 255])
    expect(api.invokeMock).toHaveBeenCalledWith('read_notification_sound', { id: 'sound-1' })
    await capabilities.notifications.implementation.commitSound('sound-1')
    await capabilities.notifications.implementation.removeSound('sound-1')
    expect(api.invokeMock).toHaveBeenCalledWith('commit_notification_sound', { id: 'sound-1' })
    expect(api.invokeMock).toHaveBeenCalledWith('remove_notification_sound', { id: 'sound-1' })
  })

  it('maps navigation and server path operations without changing envelopes', async () => {
    const api = createFakeApi()
    const capabilities = createTauriWorkbenchCapabilities(api)

    await capabilities.tray.implementation.setPendingCount(4)
    await capabilities.externalLinks.implementation.open('https://example.com')
    await capabilities.serverPaths.implementation.chooseSaveFile({
      defaultName: 'RambleDesk-diagnostics.zip',
      extensions: ['zip'],
    })
    await capabilities.serverPaths.implementation.openAttachment({
      requestId: 'request-1',
      attachmentId: 'attachment-1',
      kind: 'workspace',
    })
    await capabilities.serverPaths.implementation.revealAttachment({
      requestId: 'request-1',
      attachmentId: 'attachment-2',
      kind: 'request',
    })

    expect(api.invokeMock).toHaveBeenCalledWith('set_pending_count', { count: 4 })
    expect(api.openUrl).toHaveBeenCalledWith('https://example.com')
    expect(api.savePath).toHaveBeenCalledWith({
      defaultPath: 'RambleDesk-diagnostics.zip',
      filters: [{ name: 'Files', extensions: ['zip'] }],
    })
    expect(api.invokeMock).toHaveBeenCalledWith('open_feedback_attachment', {
      input: { requestId: 'request-1', attachmentId: 'attachment-1', kind: 'workspace' },
    })
    expect(api.invokeMock).toHaveBeenCalledWith('reveal_feedback_attachment', {
      input: { requestId: 'request-1', attachmentId: 'attachment-2', kind: 'request' },
    })
  })

  it('owns native file-drop subscription in the server-path implementation', async () => {
    const api = createFakeApi()
    const capabilities = createTauriWorkbenchCapabilities(api)
    const handler = vi.fn()
    const onError = vi.fn()
    const unsubscribe = capabilities.serverPaths.implementation.onFileDrop(handler, onError)
    await vi.waitFor(() => expect(api.webview.onDragDropEvent).toHaveBeenCalledOnce())
    const listener = api.webview.onDragDropEvent.mock.calls[0]?.[0] as
      | ((event: TauriEvent<{ type: 'drop'; paths: string[] }>) => void)
      | undefined

    listener?.({ payload: { type: 'drop', paths: ['/tmp/notes.txt'] } })

    expect(handler).toHaveBeenCalledWith({ type: 'drop', paths: ['/tmp/notes.txt'] })
    expect(onError).not.toHaveBeenCalled()
    unsubscribe()
  })

  it('cancels a server-path file-drop subscription before async registration completes', async () => {
    const lateUnlisten = vi.fn()
    let resolveDrop: ((unlisten: TauriUnlisten) => void) | undefined
    const api = createFakeApi()
    api.webview.onDragDropEvent.mockImplementationOnce(
      () => new Promise<TauriUnlisten>((resolve) => (resolveDrop = resolve)),
    )
    const capabilities = createTauriWorkbenchCapabilities(api)
    const onError = vi.fn()

    const unsubscribe = capabilities.serverPaths.implementation.onFileDrop(
      vi.fn(),
      onError,
    )
    unsubscribe()
    resolveDrop?.(lateUnlisten)
    await Promise.resolve()

    expect(lateUnlisten).toHaveBeenCalledOnce()
    expect(onError).not.toHaveBeenCalled()
  })

  it('maps server-path import, capture, and speech inputs to the existing native wire shape', async () => {
    const api = createFakeApi()
    const capabilities = createTauriWorkbenchCapabilities(api)

    await capabilities.serverPaths.implementation.importAttachmentPath({
      requestId: 'request-1',
      path: '/tmp/example.png',
      expectedRevision: 7,
    })
    await capabilities.screenCapture.implementation.complete({
      requestId: 'request-1',
      captureSessionId: 'capture-1',
      expectedRevision: 8,
    })
    await capabilities.clipboardCapture.implementation.captureOnce({
      requestId: 'request-1',
      rambleContextId: 'context-1',
    })
    await capabilities.clipboardCapture.implementation.completeImage({
      requestId: 'request-1',
      captureId: 'clipboard-1',
      rambleContextId: 'context-1',
      fileName: 'clipboard.png',
      expectedRevision: 9,
    })
    const speechSession = capabilities.speech.implementation.start(
      {
        inputDevice: null,
        modelId: 'sense-voice-small',
        vadThreshold: 0.4,
        vadSilenceMs: 650,
        hotwords: ['RambleDesk'],
      },
      { onEvent: vi.fn(), onError: vi.fn() },
    )
    await speechSession.ready

    expect(api.invokeMock).toHaveBeenCalledWith('import_feedback_attachment_path', {
      requestId: 'request-1', path: '/tmp/example.png', expectedRevision: 7,
    })
    expect(api.invokeMock).toHaveBeenCalledWith('add_completed_screen_capture', {
      requestId: 'request-1', captureSessionId: 'capture-1', expectedRevision: 8,
    })
    expect(api.invokeMock).toHaveBeenCalledWith('capture_clipboard_once', {
      input: { request_id: 'request-1', ramble_context_id: 'context-1' },
    })
    expect(api.invokeMock).toHaveBeenCalledWith('add_completed_clipboard_capture', {
      requestId: 'request-1',
      captureId: 'clipboard-1',
      rambleContextId: 'context-1',
      fileName: 'clipboard.png',
      expectedRevision: 9,
    })
    expect(api.invokeMock).toHaveBeenCalledWith('start_voice_ramble', {
      input: {
        recognition_session_id: speechSession.id,
        input_device: null,
        model_id: 'sense-voice-small',
        vad_threshold: 0.4,
        vad_silence_ms: 650,
        hotwords: ['RambleDesk'],
      },
    })
  })

  it('keeps event registration cancellable before and after async setup', async () => {
    const lateUnlisten = vi.fn()
    let resolveListen: ((unlisten: TauriUnlisten) => void) | undefined
    const api = createFakeApi()
    api.listenMock.mockImplementationOnce(
      () => new Promise<TauriUnlisten>((resolve) => (resolveListen = resolve)),
    )
    const capabilities = createTauriWorkbenchCapabilities(api)
    const handler = vi.fn()
    const onError = vi.fn()
    const unsubscribe = capabilities.screenCapture.implementation.onReady(handler, onError)
    unsubscribe()
    resolveListen?.(lateUnlisten)
    await Promise.resolve()
    expect(api.listenMock).toHaveBeenCalledWith('screen-capture-ready', expect.any(Function))
    expect(lateUnlisten).toHaveBeenCalledOnce()
    expect(onError).not.toHaveBeenCalled()

    const payload = { capture_session_id: 'capture-1', file_name: 'capture.png' }
    const api2 = createFakeApi()
    const capabilities2 = createTauriWorkbenchCapabilities(api2)
    capabilities2.screenCapture.implementation.onReady(handler, onError)
    await Promise.resolve()
    const registered = api2.listenMock.mock.calls[0]?.[1] as
      | ((event: TauriEvent<typeof payload>) => void)
      | undefined
    registered?.({ payload })
    expect(handler).toHaveBeenCalledWith(payload)
  })

  it('preserves console, shortcut, administration, and updater commands', async () => {
    const api = createFakeApi()
    const capabilities = createTauriWorkbenchCapabilities(api)

    await capabilities.rambleConsole.implementation.show()
    await capabilities.rambleConsole.implementation.restoreVisibility()
    await capabilities.rambleConsole.implementation.hide()
    await capabilities.rambleConsole.implementation.recordDiagnostic('ramble_started', 'request-1')
    await capabilities.globalShortcuts.implementation.update('rambleToggle', 'Ctrl+Shift+R')
    await capabilities.dataStorageAdministration.implementation.select('/tmp/data')
    await capabilities.hostIntegrationAdministration.implementation.installGenericMcpHosts(['pi', 'codex'])
    await capabilities.hostIntegrationAdministration.implementation.piStatus()
    await capabilities.hostIntegrationAdministration.implementation.installDsh()
    await capabilities.webAccessAdministration.implementation.setEnabled(true)
    await capabilities.webAccessAdministration.implementation.setEnabled(false)
    await capabilities.diagnostics.implementation.export('last_24_hours', '/tmp/report.zip')
    await capabilities.softwareUpdates.implementation.check({ prompt: true, forcePrompt: false })
    await capabilities.softwareUpdates.implementation.install()

    expect(api.invokeMock.mock.calls.filter(([command]) => command === 'show_ramble_console')).toHaveLength(2)
    expect(api.invokeMock).toHaveBeenCalledWith('show_ramble_console')
    expect(api.emitMock).toHaveBeenCalledWith('ramble-console', 'ramble-console-show')
    expect(api.invokeMock).toHaveBeenCalledWith('hide_ramble_console')
    expect(api.emitMock).toHaveBeenCalledWith('ramble-console', 'ramble-console-hide')
    expect(api.invokeMock).toHaveBeenCalledWith('record_diagnostic_event', {
      activity: 'ramble_started', caseId: 'request-1',
    })
    expect(api.invokeMock).toHaveBeenCalledWith('set_shortcut_setting', {
      action: 'rambleToggle', shortcut: 'Ctrl+Shift+R',
    })
    expect(api.invokeMock).toHaveBeenCalledWith('set_data_storage_path', { path: '/tmp/data' })
    expect(api.invokeMock).toHaveBeenCalledWith('install_generic_mcp_hosts', {
      hostIds: ['pi', 'codex'],
    })
    expect(api.invokeMock).toHaveBeenCalledWith('get_pi_package_status', { checkoutRoot: null })
    expect(api.invokeMock).toHaveBeenCalledWith('install_dsh_package', {
      checkoutRoot: null, profileId: null,
    })
    expect(api.invokeMock).toHaveBeenCalledWith('start_web_access')
    expect(api.invokeMock).toHaveBeenCalledWith('stop_web_access')
    expect(api.invokeMock).toHaveBeenCalledWith('export_diagnostics', {
      scope: 'last_24_hours', path: '/tmp/report.zip',
    })
    expect(api.checkForUpdates).toHaveBeenCalledWith({ prompt: true, forcePrompt: false })
    expect(api.installUpdate).toHaveBeenCalledOnce()
  })

  it('covers the remaining focused operation and event mappings', async () => {
    const api = createFakeApi()
    const capabilities = createTauriWorkbenchCapabilities(api)
    const handler = vi.fn()
    const onError = vi.fn()

    await capabilities.notifications.implementation.requestPermission()
    await capabilities.notifications.implementation.send({ title: 'New request', body: 'Body' })
    await capabilities.notifications.implementation.importSound('/tmp/sound.mp3')
    await capabilities.screenCapture.implementation.begin()
    await capabilities.screenCapture.implementation.discard('capture-1')
    await capabilities.clipboardCapture.implementation.discardImage('clipboard-1')
    await capabilities.globalShortcuts.implementation.read()
    await capabilities.globalShortcuts.implementation.reset()
    await capabilities.globalShortcuts.implementation.setCaptureActive(true)
    await capabilities.speech.implementation.listModels()
    await capabilities.speech.implementation.downloadModel('sense-voice-small')
    await capabilities.speech.implementation.deleteModel('sense-voice-small')
    await capabilities.speech.implementation.listInputDevices()
    await capabilities.systemPermissions.implementation.list()
    await capabilities.systemPermissions.implementation.request('screen_recording')
    await capabilities.systemPermissions.implementation.openSettings('screen_recording')
    await capabilities.dataStorageAdministration.implementation.read()
    await capabilities.hostIntegrationAdministration.implementation.genericMcpConfiguration()
    await capabilities.hostIntegrationAdministration.implementation.detectGenericMcpHosts()
    await capabilities.hostIntegrationAdministration.implementation.installPi()
    await capabilities.hostIntegrationAdministration.implementation.uninstallPi()
    await capabilities.webAccessAdministration.implementation.status()
    await capabilities.webAccessAdministration.implementation.open()
    await capabilities.webAccessAdministration.implementation.copyToken()
    expect(await capabilities.softwareUpdates.implementation.version()).toBe('1.2.3')
    capabilities.screenCapture.implementation.onFinished(handler, onError)
    capabilities.screenCapture.implementation.onShortcut(handler, onError)
    capabilities.globalShortcuts.implementation.onRambleToggle(handler, onError)
    capabilities.speech.implementation.onModelProgress(handler, onError)
    capabilities.rambleConsole.implementation.onCommand(handler, onError)
    capabilities.rambleConsole.implementation.onReady(handler, onError)
    capabilities.dataStorageAdministration.implementation.onProgress(handler, onError)
    await Promise.resolve()

    expect(api.requestNotificationPermission).toHaveBeenCalledOnce()
    expect(api.sendNotification).toHaveBeenCalledWith({ title: 'New request', body: 'Body' })
    expect(api.invokeMock).toHaveBeenCalledWith('import_notification_sound', {
      path: '/tmp/sound.mp3',
    })
    expect(api.invokeMock).toHaveBeenCalledWith('begin_screen_capture')
    expect(api.invokeMock).toHaveBeenCalledWith('discard_screen_capture', {
      captureSessionId: 'capture-1',
    })
    expect(api.invokeMock).toHaveBeenCalledWith('discard_clipboard_capture_image', {
      captureId: 'clipboard-1',
    })
    expect(api.invokeMock).toHaveBeenCalledWith('set_shortcut_capture_active', { active: true })
    expect(api.invokeMock).toHaveBeenCalledWith('download_speech_model', {
      modelId: 'sense-voice-small',
    })
    expect(api.invokeMock).toHaveBeenCalledWith('request_macos_permission', {
      permission: 'screen_recording',
    })
    expect(api.invokeMock).toHaveBeenCalledWith('open_macos_privacy_settings', {
      permission: 'screen_recording',
    })
    expect(api.invokeMock).toHaveBeenCalledWith('install_pi_package', { checkoutRoot: null })
    expect(api.invokeMock).toHaveBeenCalledWith('uninstall_pi_package', { checkoutRoot: null })
    expect(api.invokeMock).toHaveBeenCalledWith('get_web_access_status')
    expect(api.invokeMock).toHaveBeenCalledWith('open_web_access')
    expect(api.invokeMock).toHaveBeenCalledWith('copy_web_access_token')
    expect(api.listenMock.mock.calls.map(([event]) => event)).toEqual([
      'screen-capture-finished',
      'screen-capture-shortcut',
      'ramble-toggle-shortcut',
      'speech-model-progress',
      'ramble-console-command',
      'ramble-console-ready',
      'storage-migration-progress',
    ])
  })
})
