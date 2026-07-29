import { describe, expect, it } from 'vitest'

import { requestStatusLabel } from './feedback'

describe('requestStatusLabel', () => {
  it('keeps persisted request states understandable to the operator', () => {
    expect(requestStatusLabel('waiting')).toBe('等待开始')
    expect(requestStatusLabel('in_progress')).toBe('反馈中')
    expect(requestStatusLabel('completed')).toBe('已提交')
    expect(requestStatusLabel('cancelled')).toBe('已取消')
  })
})
