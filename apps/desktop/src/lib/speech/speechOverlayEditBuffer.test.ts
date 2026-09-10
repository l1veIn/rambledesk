import { describe, expect, it } from 'vitest'

import { synchronizeSpeechEditBuffer } from './speechOverlayEditBuffer'

describe('speech overlay edit buffer', () => {
  it('keeps typed edits when refreshed speech state has the same segment IDs', () => {
    const buffer = synchronizeSpeechEditBuffer(null, { ids: ['first'], text: 'transcribed words' })!
    buffer.text = 'my correction'

    const refreshed = synchronizeSpeechEditBuffer(buffer, { ids: ['first'], text: 'transcribed words' })

    expect(refreshed?.text).toBe('my correction')
  })

  it('starts a new buffer only when the editing segment IDs change', () => {
    const buffer = { ids: ['first'], text: 'my correction' }

    expect(synchronizeSpeechEditBuffer(buffer, { ids: ['second'], text: 'next segment' }))
      .toEqual({ ids: ['second'], text: 'next segment' })
  })

  it('copies segment IDs so later arrivals cannot become part of the edit', () => {
    const edit = { ids: ['first'], text: 'transcribed words' }
    const buffer = synchronizeSpeechEditBuffer(null, edit)
    edit.ids.push('later')

    expect(buffer?.ids).toEqual(['first'])
  })

  it('drops cancelled edits and starts fresh when the same segment is edited again', () => {
    const buffer = { ids: ['first'], text: 'cancelled correction' }
    const closed = synchronizeSpeechEditBuffer(buffer, null)

    expect(closed).toBeNull()
    expect(synchronizeSpeechEditBuffer(closed, { ids: ['first'], text: 'original words' }))
      .toEqual({ ids: ['first'], text: 'original words' })
  })
})
