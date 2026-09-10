// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
const listen = vi.hoisted(() =>
  vi.fn(async (_event: string, handler: (event: { payload: { capture_session_id: string } }) => void) => {
    handlers.set(_event, handler)
    return () => {}
  }),
)
const handlers = vi.hoisted(
  () => new Map<string, (event: { payload: { capture_session_id: string } }) => void>(),
)

vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))
vi.mock('./lib/screen-capture/screenshotRenderer', () => ({
  renderCaptureAnnotations: vi.fn(),
  exportAnnotatedCapture: vi.fn(() => 'png-base64'),
}))

import ScreenshotOverlay from './ScreenshotOverlay.svelte'

const capture = {
  capture_session_id: 'capture-1',
  image_width: 100,
  image_height: 100,
  targets: [],
  suggested_selection: null,
}

function fakeContext() {
  return {
    putImageData: vi.fn(),
    clearRect: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    stroke: vi.fn(),
    fill: vi.fn(),
    fillRect: vi.fn(),
    strokeRect: vi.fn(),
    ellipse: vi.fn(),
    arc: vi.fn(),
    closePath: vi.fn(),
    fillText: vi.fn(),
    drawImage: vi.fn(),
    setLineDash: vi.fn(),
    translate: vi.fn(),
    rotate: vi.fn(),
    scale: vi.fn(),
  }
}

function pointerEvent(type: string, x: number, y: number) {
  return new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    clientX: x,
    clientY: y,
    button: 0,
    buttons: type === 'pointerup' ? 0 : 1,
  })
}

async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 25))
}

describe('Screenshot overlay interaction', () => {
  let host: HTMLElement

  beforeEach(() => {
    invoke.mockReset()
    invoke.mockImplementation(async (command: string) => {
      if (command === 'get_active_capture_info') return capture
      if (command === 'read_capture_rgba_bytes') {
        return new Uint8Array(capture.image_width * capture.image_height * 4).buffer
      }
      return undefined
    })
    HTMLCanvasElement.prototype.getContext = vi.fn(() => fakeContext()) as never
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as never
    globalThis.ImageData = class {
      constructor(
        readonly data: Uint8ClampedArray,
        readonly width: number,
        readonly height: number,
      ) {}
    } as never
    HTMLElement.prototype.setPointerCapture = vi.fn()
    HTMLElement.prototype.releasePointerCapture = vi.fn()
    host = document.createElement('div')
    document.body.append(host)
  })

  afterEach(() => {
    host.remove()
  })

  async function render() {
    const app = mount(ScreenshotOverlay, { target: host })
    await settle()
    return app
  }

  it('loads the active capture and shows its display canvas', async () => {
    const app = await render()
    expect(invoke).toHaveBeenCalledWith('get_active_capture_info')
    expect(invoke).toHaveBeenCalledWith('show_screen_capture_overlay')
    expect(host.querySelectorAll('canvas')).toHaveLength(2)
    expect(host.querySelector('.capture-help')).not.toBeNull()
    expect(host.querySelector('.selection-frame')).toBeNull()
    await unmount(app)
  })

  it('drags out a selection and reveals the toolbar', async () => {
    const app = await render()
    const shell = host.querySelector('main')!
    shell.dispatchEvent(pointerEvent('pointerdown', 200, 200))
    shell.dispatchEvent(pointerEvent('pointermove', 320, 300))
    shell.dispatchEvent(pointerEvent('pointerup', 320, 300))
    await settle()

    expect(host.querySelector('.selection-frame')).not.toBeNull()
    expect(host.querySelector('.capture-toolbar')).not.toBeNull()
    await unmount(app)
  })

  it('cancels the capture on Escape', async () => {
    const app = await render()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await settle()
    expect(invoke).toHaveBeenCalledWith('cancel_screen_capture')
    await unmount(app)
  })

  it('completes the capture on Enter once a selection exists', async () => {
    const app = await render()
    const shell = host.querySelector('main')!
    shell.dispatchEvent(pointerEvent('pointerdown', 200, 200))
    shell.dispatchEvent(pointerEvent('pointermove', 320, 300))
    shell.dispatchEvent(pointerEvent('pointerup', 320, 300))
    await settle()

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    await settle()

    expect(invoke).toHaveBeenCalledWith(
      'complete_screen_capture',
      expect.objectContaining({
        input: expect.objectContaining({ capture_session_id: 'capture-1' }),
      }),
    )
    await unmount(app)
  })

  it('reports a failed capture session without leaving the overlay blank', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'get_active_capture_info') {
        return { capture_session_id: 'capture-1', image_width: 100, image_height: 100, targets: [] }
      }
      if (command === 'read_capture_rgba_bytes') throw new Error('pixel data is incomplete')
      return undefined
    })
    const app = await render()
    handlers.get('screen-capture-session-ready')?.({
      payload: { capture_session_id: 'capture-1' },
    })
    await settle()
    expect(host.querySelector('.capture-error')).not.toBeNull()
    expect(host.textContent).toContain('pixel data is incomplete')
    await unmount(app)
  })
})
