<script lang="ts">
  import { onMount } from 'svelte'
  import App from './App.svelte'
  import WebAccessAuthGate from './lib/WebAccessAuthGate.svelte'
  import { HttpApplicationSession } from './lib/application/httpApplicationTransport'
  import { ReplaceableApplicationTransport } from './lib/application/replaceableApplicationTransport'
  import { replaceReadyApplicationTransport } from './lib/application/browserReauthentication'
  import { createWorkbenchComposition } from './lib/application/workbenchComposition'
  import { bootstrapWebAccessSession } from './lib/application/webAccessBootstrap'
  import { createBrowserWorkbenchCapabilities } from './lib/capabilities/browser/browserCapabilities'
  import { createBrowserPublishedFeedbackAction } from './lib/publishedFeedbackAction'

  const capabilities = createBrowserWorkbenchCapabilities()
  let applicationTransport: ReplaceableApplicationTransport | null = null
  let app: App
  let authenticationRequired = true
  let authenticationEpoch = 0
  /** Hold the token gate back until the cookie resume settles, so it never flashes. */
  let resumeSettled = false

  onMount(() => {
    void resumeBrowserSession()
  })

  async function resumeBrowserSession() {
    try {
      const sessionToken = await bootstrapWebAccessSession({})
      await authenticate(sessionToken)
    } catch {
      // No usable browser session: the token gate below takes over.
    } finally {
      resumeSettled = true
    }
  }

  async function authenticate(sessionToken: string) {
    const epoch = ++authenticationEpoch
    const session = HttpApplicationSession.authenticated({
      accessToken: sessionToken,
      onTerminalError: () => {
        if (epoch === authenticationEpoch) authenticationRequired = true
      },
    })
    const nextComposition = createWorkbenchComposition({
      environment: 'browser',
      previewMode: false,
      authenticatedWebSession: session,
      capabilities,
    })
    const next = nextComposition.applicationTransport
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
    {capabilities}
    environment="browser"
    publishedFeedbackAction={createBrowserPublishedFeedbackAction(applicationTransport)}
    previewMode={false}
  />
{/if}

{#if authenticationRequired && resumeSettled}
  <WebAccessAuthGate onAuthenticated={authenticate} />
{/if}
