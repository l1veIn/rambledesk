import { describe, expect, it } from 'vitest'

import { TAB_MAX_WIDTH, TAB_MIN_WIDTH, tabStripLayout } from './tabStripLayout'

describe('tab strip layout', () => {
  it('fills the strip with uniform tabs up to the maximum width', () => {
    expect(tabStripLayout(1200, 3)).toEqual({ tabWidth: TAB_MAX_WIDTH, overflowing: false })
    expect(tabStripLayout(600, 4)).toEqual({ tabWidth: 150, overflowing: false })
  })

  it('stops at the minimum width and starts scrolling', () => {
    expect(tabStripLayout(400, 4)).toEqual({ tabWidth: TAB_MIN_WIDTH, overflowing: true })
    expect(tabStripLayout(100, 3)).toEqual({ tabWidth: TAB_MIN_WIDTH, overflowing: true })
  })

  it('hands the whole strip to a single tab', () => {
    expect(tabStripLayout(1000, 1)).toEqual({ tabWidth: TAB_MAX_WIDTH, overflowing: false })
  })

  it('falls back to the preferred width before the strip is measured', () => {
    expect(tabStripLayout(0, 5)).toEqual({ tabWidth: TAB_MAX_WIDTH, overflowing: false })
    expect(tabStripLayout(Number.NaN, 2)).toEqual({ tabWidth: TAB_MAX_WIDTH, overflowing: false })
    expect(tabStripLayout(500, 0)).toEqual({ tabWidth: TAB_MAX_WIDTH, overflowing: false })
  })
})
