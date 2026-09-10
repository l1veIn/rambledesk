import { afterEach, describe, expect, it, vi } from 'vitest'

import { textAnnotation } from './annotationModel'
import {
  createPointerInteraction,
  type PointerInteractionState as InteractionState,
} from './pointerInteraction'
import type { OverlayGeometry } from './overlayGeometry'

const geometry: OverlayGeometry = {
  capture: {
    capture_session_id: 'session-1',
    image_width: 100,
    image_height: 100,
    targets: [],
  } as never,
  displayRectangle: { x: 0, y: 0, width: 100, height: 100 },
  viewportWidth: 800,
  viewportHeight: 800,
}

class FakeElement {
  closest(): null {
    return null
  }
}

function pointerEvent(overrides: Record<string, unknown> = {}) {
  return {
    button: 0,
    pointerId: 1,
    clientX: 0,
    clientY: 0,
    target: new FakeElement(),
    currentTarget: { setPointerCapture: vi.fn() },
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    ...overrides,
  } as unknown as PointerEvent
}

function harness(overrides: Partial<InteractionState> = {}) {
  const state = {
    capture: { image_width: 100, image_height: 100, targets: [] },
    completing: false,
    selection: null,
    hoveredTarget: null,
    activeTool: 'select',
    annotations: [],
    draftAnnotation: null,
    selectedAnnotationId: null,
    textDraft: null,
    style: { color: '#ff0000', strokeWidth: 4 },
    ...overrides,
  } as InteractionState & { annotations: InteractionState['annotations'] }
  const commit = vi.fn((next) => {
    ;(state as { annotations: unknown }).annotations = next
  })
  const pushUndoSnapshot = vi.fn()
  const focusTextInput = vi.fn()
  const scheduleToolbarLayout = vi.fn()
  vi.stubGlobal('Element', FakeElement)
  vi.stubGlobal('window', { getSelection: () => ({ removeAllRanges: vi.fn() }) })
  const interaction = createPointerInteraction({
    getGeometry: () => geometry,
    toImagePoint: (event) => ({ x: event.clientX, y: event.clientY }),
    getState: () => state,
    patch: (next) => {
      Object.assign(state, next)
    },
    commit,
    pushUndoSnapshot,
    closePanels: vi.fn(),
    clearError: vi.fn(),
    focusTextInput,
    scheduleToolbarLayout,
  })
  return { interaction, state, commit, pushUndoSnapshot, focusTextInput, scheduleToolbarLayout }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('pointer interaction', () => {
  it('drags out a new selection', () => {
    const { interaction, state } = harness()

    interaction.begin(pointerEvent({ clientX: 10, clientY: 10 }))
    interaction.move(pointerEvent({ clientX: 30, clientY: 30 }))

    expect(state.selection).toEqual({ x: 10, y: 10, width: 20, height: 20 })
  })

  it('clears a selection that never grew past the drag tolerance', () => {
    const { interaction, state, scheduleToolbarLayout } = harness()

    interaction.begin(pointerEvent({ clientX: 10, clientY: 10 }))
    interaction.move(pointerEvent({ clientX: 12, clientY: 12 }))
    interaction.end(pointerEvent({ clientX: 12, clientY: 12 }))

    expect(state.selection).toBeNull()
    expect(scheduleToolbarLayout).not.toHaveBeenCalled()
  })

  it('moves a hit annotation and records one undo snapshot', () => {
    const annotation = textAnnotation({ x: 10, y: 10 }, 'note', {
      color: '#ff0000',
      strokeWidth: 4,
    }, 'annotation-1')
    const { interaction, state, pushUndoSnapshot } = harness({
      selection: { x: 0, y: 0, width: 50, height: 50 },
      annotations: [annotation],
    })

    interaction.begin(pointerEvent({ clientX: 12, clientY: 12 }))
    expect(state.selectedAnnotationId).toBe('annotation-1')

    interaction.move(pointerEvent({ clientX: 22, clientY: 22 }))
    expect(state.annotations[0]).toMatchObject({ point: { x: 20, y: 20 } })

    interaction.end(pointerEvent({ clientX: 22, clientY: 22 }))
    expect(pushUndoSnapshot).toHaveBeenCalledWith([annotation])
  })

  it('starts a text draft inside the selection', () => {
    const { interaction, state, focusTextInput } = harness({
      selection: { x: 0, y: 0, width: 50, height: 50 },
      activeTool: 'text',
    })

    interaction.begin(pointerEvent({ clientX: 10, clientY: 10 }))

    expect(state.textDraft).toEqual({ point: { x: 10, y: 10 }, value: '' })
    expect(focusTextInput).toHaveBeenCalled()
  })

  it('commits a numbered counter', () => {
    const { interaction, commit } = harness({
      selection: { x: 0, y: 0, width: 50, height: 50 },
      activeTool: 'counter',
    })

    interaction.begin(pointerEvent({ clientX: 10, clientY: 10 }))

    expect(commit).toHaveBeenCalledWith([
      expect.objectContaining({ type: 'counter', number: 1 }),
    ])
  })

  it('draws a pen stroke and commits it when it has size', () => {
    const { interaction, state, commit } = harness({
      selection: { x: 0, y: 0, width: 50, height: 50 },
      activeTool: 'pen',
    })

    interaction.begin(pointerEvent({ clientX: 10, clientY: 10 }))
    expect(state.draftAnnotation).toMatchObject({
      type: 'pen',
      points: [
        { x: 10, y: 10 },
        { x: 10, y: 10 },
      ],
    })

    interaction.move(pointerEvent({ clientX: 20, clientY: 20 }))
    expect(state.draftAnnotation).toMatchObject({
      type: 'pen',
      points: [
        { x: 10, y: 10 },
        { x: 10, y: 10 },
        { x: 20, y: 20 },
      ],
    })

    interaction.end(pointerEvent({ clientX: 20, clientY: 20 }))
    expect(commit).toHaveBeenCalled()
    expect(state.draftAnnotation).toBeNull()
  })

  it('resizes the selection from a handle', () => {
    const { interaction, state } = harness({
      selection: { x: 10, y: 10, width: 30, height: 30 },
    })

    interaction.beginSelectionResize(pointerEvent({ clientX: 40, clientY: 40 }), 'se')
    interaction.move(pointerEvent({ clientX: 60, clientY: 60 }))

    expect(state.selection).toEqual({ x: 10, y: 10, width: 50, height: 50 })
  })
})
