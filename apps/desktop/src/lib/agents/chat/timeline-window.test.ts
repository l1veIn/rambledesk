import { describe, expect, it } from 'vitest'
import type { SessionActivity } from '../managedSessionUi'
import { TimelineWindow } from './timeline-window'
import { captureActivityAnchor, restoreActivityAnchor } from './scroll-anchor'

function rows(first: number, last: number): SessionActivity[] {
  return Array.from({ length: last - first + 1 }, (_, offset) => ({ id: `row-${first + offset}`, sequence: first + offset, session_id: 'one', kind: 'agent_message', text: `Message ${first + offset}`, tool_call_id: null, created_at: 'today' }))
}

describe('timeline rendering window', () => {
  it('mounts only the latest 60 of 1000 activities and reveals locally loaded history 60 at a time', () => {
    const window = new TimelineWindow()
    const activities = rows(1, 1000)
    expect(window.read('one', activities, true)).toHaveLength(60)
    expect(window.read('one', activities, true)[0].id).toBe('row-941')
    window.revealOlder(activities)
    expect(window.read('one', activities, false)).toHaveLength(120)
    expect(window.read('one', activities, false)[0].id).toBe('row-881')
  })

  it('retains review identity during fresh patches and appended messages, but resets for another session', () => {
    const window = new TimelineWindow()
    const activities = rows(1, 100)
    window.read('one', activities, true)
    const updated = [...activities, ...rows(101, 102)]
    updated[50] = { ...updated[50], text: 'Final complete patch' }
    const reviewing = window.read('one', updated, false)
    expect(reviewing[0].id).toBe('row-41')
    expect(reviewing.find((row) => row.id === 'row-51')?.text).toBe('Final complete patch')
    expect(window.read('one', updated, true)[0].id).toBe('row-43')
    expect(window.read('two', rows(1, 200), false)[0].id).toBe('row-141')
  })

  it('reveals only 60 of a fetched 100-row page while retaining the existing visible rows', () => {
    const window = new TimelineWindow()
    window.read('one', rows(101, 130), true)
    const fetched = rows(1, 130)
    expect(window.read('one', fetched, false)[0].id).toBe('row-101')
    window.revealOlder(fetched)
    expect(window.read('one', fetched, false)[0].id).toBe('row-41')
    expect(window.read('one', fetched, false)).toHaveLength(90)
  })

  it('extends the visible start to the loaded beginning of a turn instead of slicing its answer', () => {
    const window = new TimelineWindow()
    const activities = rows(1, 100).map((row) => ({ ...row, turn_id: row.sequence! <= 30 ? 'older' : 'large' }))
    const visible = window.read('one', activities, true)
    expect(visible[0].id).toBe('row-31')
    expect(visible).toHaveLength(70)
    expect(visible.at(-1)?.id).toBe('row-100')
  })

  it('retains a partial turn and includes its beginning when a previous server page arrives', () => {
    const window = new TimelineWindow()
    const page = rows(101, 130).map((row) => ({ ...row, turn_id: 'large' }))
    expect(window.read('one', page, true)).toHaveLength(30)
    const all = rows(1, 130).map((row) => ({ ...row, turn_id: 'large' }))
    const visible = window.read('one', all, false)
    expect(visible[0].id).toBe('row-1')
    expect(visible.at(-1)?.id).toBe('row-130')
  })
})

describe('prepended activity scroll anchors', () => {
  it('keeps the first partially visible row at its original offset after prepend and Markdown layout', () => {
    const geometry = { top: 100, scrollTop: 150, scrollHeight: 1000 }
    const positions = [{ id: 'a', top: 0 }, { id: 'b', top: 100 }, { id: 'c', top: 200 }]
    const elements = () => positions.map((row) => ({ dataset: { activityId: row.id }, getBoundingClientRect: () => ({ top: geometry.top + row.top - geometry.scrollTop, bottom: geometry.top + row.top + 100 - geometry.scrollTop }) }))
    const viewport = { get scrollTop() { return geometry.scrollTop }, set scrollTop(value) { geometry.scrollTop = value },
      get scrollHeight() { return geometry.scrollHeight }, getBoundingClientRect: () => ({ top: geometry.top }), querySelectorAll: elements } as unknown as HTMLElement
    const anchor = captureActivityAnchor(viewport)
    expect(anchor).toMatchObject({ id: 'b', offset: -50 })
    for (const row of positions) row.top += 500
    geometry.scrollHeight += 500
    restoreActivityAnchor(viewport, anchor!)
    expect(geometry.scrollTop).toBe(650)
    expect(captureActivityAnchor(viewport)?.offset).toBe(-50)
    for (const row of positions) row.top += 40
    geometry.scrollHeight += 40
    restoreActivityAnchor(viewport, anchor!)
    expect(geometry.scrollTop).toBe(690)
    expect(captureActivityAnchor(viewport)?.id).toBe('b')
  })
})
