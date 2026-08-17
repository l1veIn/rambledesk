const WINDOWS_VERBATIM_PREFIX = '\\\\?\\'
const WINDOWS_VERBATIM_UNC_PREFIX = '\\\\?\\UNC\\'

export function desktopPath(value: string): string {
  if (value.startsWith(WINDOWS_VERBATIM_UNC_PREFIX)) {
    return `\\\\${value.slice(WINDOWS_VERBATIM_UNC_PREFIX.length)}`
  }
  if (value.startsWith(WINDOWS_VERBATIM_PREFIX)) {
    return value.slice(WINDOWS_VERBATIM_PREFIX.length)
  }
  return value
}

export type DiagnosticExportResult = {
  report_id?: string
  reportId?: string
  path?: string
  scope?: string
  event_count?: number
  eventCount?: number
  request_count?: number
  requestCount?: number
  log_file_count?: number
  logFileCount?: number
}

export function diagnosticExportView(result: DiagnosticExportResult) {
  return {
    path: desktopPath(result.path ?? ''),
    events: result.event_count ?? result.eventCount ?? 0,
    requests: result.request_count ?? result.requestCount ?? 0,
    logs: result.log_file_count ?? result.logFileCount ?? 0,
  }
}
