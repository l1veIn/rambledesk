<script lang="ts">
  import { emitTo, listen } from '@tauri-apps/api/event'
  import { invoke } from '@tauri-apps/api/core'
  import { onMount, tick } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { PhysicalPosition } from '@tauri-apps/api/dpi'
  import { shortcutSettings } from './lib/shortcutSettings'
  import RecordingOverlay from './lib/RecordingOverlay.svelte'
  import { RAMBLE_CONSOLE_COMMAND_EVENT, type RambleConsoleCommand } from './lib/rambleConsole'
  import { SPEECH_OVERLAY_READY_EVENT, SPEECH_OVERLAY_STATE_EVENT, speechOverlayVisible, type SpeechOverlayState } from './lib/speechOverlay'

  let state: SpeechOverlayState = { enabled: true, opacity: 95, selectedGroupId: null, shortcuts: $shortcutSettings, phase: 'idle', level: 0, partial: '', error: '', target: null, groups: [], receipt: null }
  let content: HTMLDivElement
  let commandError = ''
  let layoutQueue = Promise.resolve()
  let lastLayout = ''
  type Drag = {
    pointer: number; handle: HTMLElement; startX: number; startY: number; x: number; y: number;
    origin: Promise<{ position: PhysicalPosition; scale: number }>; moving: boolean;
  }
  let drag: Drag | null = null

  function startDrag(event: PointerEvent) {
    if (event.button !== 0 || drag) return
    event.preventDefault()
    const handle = event.currentTarget as HTMLElement
    handle.setPointerCapture(event.pointerId)
    const window = getCurrentWindow()
    drag = {
      pointer: event.pointerId, handle, startX: event.screenX, startY: event.screenY, x: event.screenX, y: event.screenY, moving: false,
      origin: Promise.all([window.outerPosition(), window.scaleFactor()]).then(([position, scale]) => ({ position, scale })),
    }
    void drag.origin.catch((cause) => { commandError = String(cause) })
  }

  function moveDrag(event: PointerEvent) {
    if (!drag || drag.pointer !== event.pointerId) return
    drag.x = event.screenX
    drag.y = event.screenY
    void applyDrag(drag)
  }

  async function applyDrag(current: Drag) {
    if (current.moving) return
    current.moving = true
    try {
      const { position, scale } = await current.origin
      // Coalesce moves but always apply the final pointer position, even for a
      // gesture that ends before the first desktop response comes back.
      for (;;) {
        const { x, y } = current
        await getCurrentWindow().setPosition(new PhysicalPosition(
          Math.round(position.x + (x - current.startX) * scale),
          Math.round(position.y + (y - current.startY) * scale),
        ))
        if (x === current.x && y === current.y) break
      }
    } catch (cause) { commandError = String(cause) }
    finally { current.moving = false }
  }

  function endDrag(event: PointerEvent) {
    if (!drag || drag.pointer !== event.pointerId) return
    moveDrag(event)
    if (drag.handle.hasPointerCapture(event.pointerId)) drag.handle.releasePointerCapture(event.pointerId)
    drag = null
  }

  function updateLayout() {
    const visible = speechOverlayVisible(state)
    const height = Math.ceil(content?.getBoundingClientRect().height ?? 0)
    const key = `${visible}:${height}`
    if (key === lastLayout) return
    lastLayout = key
    layoutQueue = layoutQueue.then(() => invoke('set_speech_overlay_layout', { visible, height }))
      .then(() => undefined)
      .catch((cause) => { lastLayout = ''; commandError = String(cause) })
  }

  onMount(() => {
    let disposed = false
    let unlisten = () => {}
    const observer = new ResizeObserver(updateLayout)
    observer.observe(content)
    void listen<SpeechOverlayState>(SPEECH_OVERLAY_STATE_EVENT, ({ payload }) => {
      state = payload
      commandError = ''
      void tick().then(updateLayout)
    }).then(async (stop) => {
      if (disposed) { stop(); return }
      unlisten = stop
      await emitTo('main', SPEECH_OVERLAY_READY_EVENT)
    }).catch((cause) => { commandError = String(cause) })
    return () => { disposed = true; unlisten(); observer.disconnect() }
  })

  async function send(command: RambleConsoleCommand) {
    try {
      if (command.type === 'open-speech-target') await invoke('focus_speech_feedback')
      await emitTo('main', RAMBLE_CONSOLE_COMMAND_EVENT, command)
    } catch (cause) {
      commandError = cause instanceof Error ? cause.message : String(cause)
    }
  }
</script>

<svelte:window onpointermove={moveDrag} onpointerup={endDrag} onpointercancel={endDrag} />

<div bind:this={content}>
  <RecordingOverlay state={commandError ? { ...state, error: commandError } : state} embedded onCommand={(command) => void send(command)} onStartDrag={startDrag} />
</div>
