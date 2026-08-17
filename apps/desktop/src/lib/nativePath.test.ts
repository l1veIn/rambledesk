import { describe, expect, it } from 'vitest'

import { desktopPath, diagnosticExportView } from './nativePath'

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

  it('reads diagnostic export counts from snake_case or camelCase', () => {
    expect(
      diagnosticExportView({
        path: '/Users/a/RambleDesk-diagnostics-7d.zip',
        event_count: 3,
        request_count: 2,
        log_file_count: 1,
      }),
    ).toEqual({
      path: '/Users/a/RambleDesk-diagnostics-7d.zip',
      events: 3,
      requests: 2,
      logs: 1,
    })
    expect(
      diagnosticExportView({
        path: '\\\\?\\C:\\Users\\A\\RambleDesk-diagnostics-all.zip',
        eventCount: 8,
        requestCount: 4,
        logFileCount: 2,
      }),
    ).toEqual({
      path: 'C:\\Users\\A\\RambleDesk-diagnostics-all.zip',
      events: 8,
      requests: 4,
      logs: 2,
    })
  })
})
