import { describe, expect, it } from 'vitest'

import { desktopPath } from './nativePath'

describe('desktopPath', () => {
  it('converts a Windows verbatim drive path for Explorer', () => {
    expect(desktopPath('\\\\?\\C:\\Users\\A\\反馈包')).toBe(
      'C:\\Users\\A\\反馈包',
    )
  })

  it('converts a Windows verbatim UNC path', () => {
    expect(desktopPath('\\\\?\\UNC\\server\\share\\反馈包')).toBe(
      '\\\\server\\share\\反馈包',
    )
  })

  it('preserves ordinary Windows, macOS, and Linux paths', () => {
    expect(desktopPath('D:\\feedback')).toBe('D:\\feedback')
    expect(desktopPath('/Users/a/feedback')).toBe('/Users/a/feedback')
  })
})
