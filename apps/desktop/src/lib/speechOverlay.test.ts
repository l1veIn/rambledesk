import { describe, expect, it } from 'vitest'
import { selectedSpeechGroup, speechOverlayVisible, speechReviewCommand, type SpeechOverlayState } from './speechOverlay'
import type { SpeechDraftGroup } from './workbench/speechDraftQueue'

const group = (id: string): SpeechDraftGroup => ({ ids: [id], requestId: id, requestTitle: id, action: null, text: `speech for ${id}`, busy: false, error: '' })
function state(): SpeechOverlayState {
  return { enabled: true, opacity: 97, selectedGroupId: 'B', shortcuts: { speechAccept: 'Ctrl+Shift+Enter', speechDiscard: 'Ctrl+Shift+Backspace' }, phase: 'idle', level: 0, partial: '', error: '', target: null, groups: [group('A'), group('B')], receipt: null }
}

describe('speech overlay review', () => {
  it('hides the window without disabling review or losing the selected group', () => {
    const hidden = { ...state(), enabled: false }
    expect(speechOverlayVisible(hidden)).toBe(false)
    expect(speechReviewCommand(hidden, 'accept')).toEqual({ type: 'accept-speech', ids: ['B'] })
    expect(speechReviewCommand(hidden, 'discard')).toEqual({ type: 'discard-speech', ids: ['B'] })
  })

  it('tracks selection by segment identity when preceding groups disappear', () => {
    const current = state()
    current.groups.shift()
    expect(selectedSpeechGroup(current)?.requestId).toBe('B')
    current.groups = [group('C')]
    expect(speechReviewCommand(current, 'accept')).toEqual({ type: 'accept-speech', ids: ['C'] })
  })

  it('snapshots the displayed group and excludes later arrivals', () => {
    const current = state()
    const command = speechReviewCommand(current, 'accept')
    current.groups[1].ids.push('later')
    expect(command).toEqual({ type: 'accept-speech', ids: ['B'] })
  })

  it('does nothing while writing, interaction is locked, or review is empty', () => {
    const current = state()
    expect(speechReviewCommand(current, 'discard', true)).toBeNull()
    current.groups[1].busy = true
    expect(speechReviewCommand(current, 'accept')).toBeNull()
    expect(speechReviewCommand(current, 'discard')).toBeNull()
    current.groups = []
    expect(speechReviewCommand(current, 'accept')).toBeNull()
    expect(speechOverlayVisible(current)).toBe(false)
  })
})
