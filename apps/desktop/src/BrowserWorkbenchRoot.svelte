<script lang="ts">
  import App from './App.svelte'
  import WebAccessAuthGate from './lib/WebAccessAuthGate.svelte'
  import { HttpApplicationSession } from './lib/application/httpApplicationTransport'
  import { ReplaceableApplicationTransport } from './lib/application/replaceableApplicationTransport'
  import { replaceReadyApplicationTransport } from './lib/application/browserReauthentication'
  import { createWorkbenchComposition } from './lib/application/workbenchComposition'
  import { createBrowserPublishedFeedbackAction } from './lib/publishedFeedbackAction'

  let applicationTransport: ReplaceableApplicationTransport | null = null
  let app: App
  let authenticationRequired = true
  let authenticationEpoch = 0

  async function authenticate(sessionToken: string) {
    const epoch = ++authenticationEpoch
    const session = HttpApplicationSession.authenticated({
      accessToken: sessionToken,
      onTerminalError: () => {
        if (epoch === authenticationEpoch) authenticationRequired = true
      },
    })
    const next = createWorkbenchComposition({
      environment: 'browser',
      previewMode: false,
      authenticatedWebSession: session,
    }).applicationTransport
    if (epoch !== authenticationEpoch) return
    if (applicationTransport) {
      await replaceReadyApplicationTransport(applicationTransport, next, () => {
        app?.refetchAfterTransportReady()
      })
    } else {
      await next.waitUntilReady()
      applicationTransport = new ReplaceableApplicationTransport(next)
    }
    if (epoch === authenticationEpoch) authenticationRequired = false
  }
</script>

{#if applicationTransport}
  <App
    bind:this={app}
    {applicationTransport}
    publishedFeedbackAction={createBrowserPublishedFeedbackAction(applicationTransport)}
    previewMode={false}
  />
{/if}

{#if authenticationRequired}
  <WebAccessAuthGate onAuthenticated={authenticate} />
{/if}
