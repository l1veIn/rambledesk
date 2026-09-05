import { describe, expect, it } from 'vitest'
import type { SessionActivity } from '../managedSessionUi'
import { TimelineWindow, crossedHistoryThreshold } from './timeline-window'
import { captureActivityAnchor, restoreActivityAnchor } from './scroll-anchor'

function rows(first: number, last: number): SessionActivity[] {
  return Array.from({ length: last - first + 1 }, (_, offset) => ({ id: `row-${first + offset}`, sequence: first + offset, session_id: 'one', kind: 'agent_message', text: `Message ${first + offset}`, tool_call_id: null, created_at: 'today' }))
}

describe('timeline rendering window', () => {
  it('shows twenty conversations even when their last turn contains hundreds of work rows', () => {
    const activities: SessionActivity[] = []
    for (let turn = 0; turn < 40; turn += 1) {
      for (let work = 0; work < (turn === 39 ? 300 : 5); work += 1) {
        const sequence = activities.length + 1
        activities.push({ ...rows(sequence, sequence)[0], turn_id: `turn-${turn}`, kind: work === 0 ? 'user_message' : 'tool_call' })
      }
    }
    const window = new TimelineWindow()
    const visible = window.read('one', activities, true)
    expect(visible.filter(row => row.kind === 'user_message')).toHaveLength(20)
    expect(visible[0].turn_id).toBe('turn-20')
    expect(visible).toHaveLength(395)
    window.revealOlder(activities)
    expect(window.read('one', activities, false)).toHaveLength(495)
  })

  it('loads only on an upward near-top crossing, never initial layout or a stationary short list', () => {
    expect(crossedHistoryThreshold(null, 0)).toBe(false)
    expect(crossedHistoryThreshold(0, 0)).toBe(false)
    expect(crossedHistoryThreshold(40, 200)).toBe(false)
    expect(crossedHistoryThreshold(400, 200)).toBe(true)
    expect(crossedHistoryThreshold(240, 0)).toBe(true)
  })

  it('mounts only the latest 20 of 1000 legacy activities and reveals locally loaded history 20 at a time', () => {
    const window = new TimelineWindow()
    const activities = rows(1, 1000)
    expect(window.read('one', activities, true)).toHaveLength(20)
    expect(window.read('one', activities, true)[0].id).toBe('row-981')
    window.revealOlder(activities)
    expect(window.read('one', activities, false)).toHaveLength(40)
    expect(window.read('one', activities, false)[0].id).toBe('row-961')
  })

  it('retains review identity during fresh patches and appended messages, but resets for another session', () => {
    const window = new TimelineWindow()
    const activities = rows(1, 100)
    window.read('one', activities, true)
    const updated = [...activities, ...rows(101, 102)]
    updated[90] = { ...updated[90], text: 'Final complete patch' }
    const reviewing = window.read('one', updated, false)
    expect(reviewing[0].id).toBe('row-81')
    expect(reviewing.find((row) => row.id === 'row-91')?.text).toBe('Final complete patch')
    expect(window.read('one', updated, true)[0].id).toBe('row-83')
    expect(window.read('two', rows(1, 200), false)[0].id).toBe('row-181')
  })

  it('reveals only 20 of a fetched 100-row page while retaining the existing visible rows', () => {
    const window = new TimelineWindow()
    window.read('one', rows(101, 130), true)
    const fetched = rows(1, 130)
    expect(window.read('one', fetched, false)[0].id).toBe('row-111')
    window.revealOlder(fetched)
    expect(window.read('one', fetched, false)[0].id).toBe('row-91')
    expect(window.read('one', fetched, false)).toHaveLength(40)
  })

  it('extends the visible start to the loaded beginning of a turn instead of slicing its answer', () => {
    const window = new TimelineWindow()
    const activities = rows(1, 100).map((row) => ({ ...row, turn_id: row.sequence! <= 30 ? 'older' : 'large' }))
    const visible = window.read('one', activities, true)
    expect(visible[0].id).toBe('row-1')
    expect(visible).toHaveLength(100)
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
