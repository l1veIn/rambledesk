import { describe, expect, it } from 'vitest'

import { updateTaskTabTitles } from './taskTabTitles'
import { requestTaskViewDescriptor, sessionViewDescriptor } from './viewDescriptors'

describe('task tab titles', () => {
  it('keeps submitted and pending request titles when switching to another session', () => {
    const views = [
      sessionViewDescriptor('codex', 'one'),
      requestTaskViewDescriptor('submitted-one'),
      requestTaskViewDescriptor('submitted-two'),
      requestTaskViewDescriptor('pending'),
    ]
    const pending = { request_id: 'pending', title: 'Please verify startup' }
    const titles = updateTaskTabTitles(new Map(), views, [
      { request_id: 'submitted-one', title: 'Confirm the startup design' },
      { request_id: 'submitted-two', title: 'Add the startup setting' },
      pending,
    ])

    const afterSwitch = updateTaskTabTitles(titles, [
      ...views,
      sessionViewDescriptor('claude-code', 'two'),
    ], [pending, { request_id: 'other-session', title: 'Verify workspace actions' }])

    expect([...afterSwitch]).toEqual([
      ['submitted-one', 'Confirm the startup design'],
      ['submitted-two', 'Add the startup setting'],
      ['pending', 'Please verify startup'],
    ])
    expect(updateTaskTabTitles(afterSwitch, views, [])).toEqual(titles)
  })

  it('retains a title learned from a loaded task after the active request changes', () => {
    const views = [requestTaskViewDescriptor('outside-list')]
    const titles = updateTaskTabTitles(new Map(), views, [
      { request_id: 'outside-list', title: 'Task opened directly' },
    ])

    expect(updateTaskTabTitles(titles, views, [
      { request_id: 'next-request', title: 'Another request' },
    ]).get('outside-list')).toBe('Task opened directly')
  })

  it('refreshes known titles and releases titles when their tabs close', () => {
    const views = [requestTaskViewDescriptor('one'), requestTaskViewDescriptor('two')]
    const titles = updateTaskTabTitles(new Map(), views, [
      { request_id: 'one', title: 'Original title' },
      { request_id: 'two', title: 'Second task' },
    ])
    const updated = updateTaskTabTitles(titles, views, [
      { request_id: 'one', title: 'Updated title' },
    ])
    expect([...updated]).toEqual([['one', 'Updated title'], ['two', 'Second task']])

    const afterClose = updateTaskTabTitles(updated, [views[1]], [])
    expect([...afterClose]).toEqual([['two', 'Second task']])
    expect(updateTaskTabTitles(afterClose, views, []).has('one')).toBe(false)
  })
})
