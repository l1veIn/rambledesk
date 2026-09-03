import { describe, expect, it } from 'vitest'

import { selectWorkbenchEntry, type WorkbenchEntry } from './workbenchEntry'

const nativeRoutes: ReadonlyArray<
  readonly [pathname: string, hash: string, expected: WorkbenchEntry]
> = [
  ['/', '#capture', 'capture'],
  ['/', '#capture-scroll', 'scroll-capture'],
  ['/', '#capture-pin=window-1', 'pinned-capture'],
  ['/', '#capture-pin=', 'pinned-capture'],
  ['/', '#ramble-console', 'ramble-console'],
  ['/', '#speech-overlay', 'speech-overlay'],
  ['/ramble-console', '', 'ramble-console'],
  ['/nested/ramble-console', '', 'ramble-console'],
]

describe('selectWorkbenchEntry', () => {
  it.each(nativeRoutes)(
    'keeps browser history route %s%s behind the authenticated browser root',
    (pathname, hash) => {
      expect(selectWorkbenchEntry({ isTauri: false, previewMode: false, pathname, hash })).toBe(
        'browser',
      )
    },
  )

  it.each(nativeRoutes)(
    'preserves the Tauri root for %s%s',
    (pathname, hash, expected) => {
      expect(selectWorkbenchEntry({ isTauri: true, previewMode: false, pathname, hash })).toBe(
        expected,
      )
    },
  )

  it('keeps ordinary desktop, browser, and explicit development preview roots distinct', () => {
    expect(
      selectWorkbenchEntry({ isTauri: true, previewMode: false, pathname: '/', hash: '' }),
    ).toBe('desktop')
    expect(
      selectWorkbenchEntry({ isTauri: false, previewMode: false, pathname: '/', hash: '' }),
    ).toBe('browser')
    expect(
      selectWorkbenchEntry({ isTauri: false, previewMode: true, pathname: '/', hash: '#capture' }),
    ).toBe('preview')
  })
})
