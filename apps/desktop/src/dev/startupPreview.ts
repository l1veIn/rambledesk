import { runFrontendBootstrap, showStartupFailure } from '../lib/startupFallback'

if (import.meta.env.DEV) {
  const timeout = new URLSearchParams(location.search).has('timeout')
  void runFrontendBootstrap(async () => {
    if (timeout) await new Promise(() => {})
    throw new TypeError('Synthetic startup failure')
  }, failure => showStartupFailure(document.getElementById('app')!, failure), 1000)
}
