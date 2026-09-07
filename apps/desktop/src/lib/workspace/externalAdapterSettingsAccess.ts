import type { SettingsSection } from '$lib/workbench/types'
import { diagnosticErrorCategory, recordClientDiagnostic, startClientDiagnostic } from '$lib/diagnostics/clientDiagnostics'

/** External integration reads belong to an explicit visit to their settings page. */
export function createExternalAdapterSettingsAccess(onEnter: readonly (() => Promise<unknown>)[]) {
  let active = false
  let previousVisit: string | undefined
  return {
    isActive: () => active,
    async setSection(section: SettingsSection, available: boolean): Promise<void> {
      const next = available && section === 'adapters'
      const entered = next && !active
      active = next
      const visit = `${section === 'adapters'}:${available}:${next}`
      if (visit !== previousVisit) {
        previousVisit = visit
        recordClientDiagnostic({ activity: 'external_adapter_settings', outcome: section === 'adapters' && !available ? 'blocked' : entered ? 'ok' : 'skipped',
          details: { action: 'visit', source: 'external_settings', available, entered } })
      }
      if (entered) {
        const finish = startClientDiagnostic('external_adapter_settings', { action: 'probe', source: 'external_settings', target_count: onEnter.length })
        try {
          const results = await Promise.allSettled(onEnter.map(read => read()))
          const failedCount = results.filter(result => result.status === 'rejected').length
          finish(failedCount ? 'failed' : 'ok', { checked_count: results.length, failed_count: failedCount })
        } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); throw cause }
      }
    },
  }
}
