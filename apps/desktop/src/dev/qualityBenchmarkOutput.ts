/** Keep growing report layout/serialization outside the default measured interval. */
export function createBenchmarkOutput(storageKey: string, output: HTMLTextAreaElement, status: HTMLOutputElement) {
  // Explicit diagnostic control: compare the same build with the old live-output cost.
  const mode = new URLSearchParams(location.search).get('quality-output') === 'live' ? 'live-json' : 'final-json'
  return {
    mode,
    begin() { output.value = '' },
    progress(result: unknown, checkpoint: Record<string, unknown>, label: string) {
      status.textContent = label
      if (mode === 'live-json') {
        const json = JSON.stringify(result, null, 2)
        output.value = json
        sessionStorage.setItem(storageKey, json)
      } else {
        // Counts/status survive interruption; full data remains in memory until completion.
        sessionStorage.setItem(storageKey, JSON.stringify({ ...checkpoint, outputMode: mode, checkpointOnly: true }))
      }
    },
    finish(result: unknown, label: string) {
      const json = JSON.stringify(result, null, 2)
      output.value = json
      sessionStorage.setItem(storageKey, json)
      status.textContent = label
    },
  }
}
