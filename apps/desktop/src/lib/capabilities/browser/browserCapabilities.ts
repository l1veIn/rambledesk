import type { CapabilityStatus } from '../capabilityManifest'
import {
  createWorkbenchCapabilities,
  type CapabilitySlot,
  type ExternalLinkCapability,
  type WorkbenchCapabilities,
} from '../workbenchCapabilities'
import { createUnavailableWorkbenchCapabilities } from '../unavailableCapabilities'
import { createBrowserImagePasteCapability } from './imagePasteCapability'

export type BrowserWindowOpen = (
  url?: string | URL,
  target?: string,
  features?: string,
) => WindowProxy | null

export type BrowserCapabilityEnvironment = Readonly<{
  pageUrl: string
  open: BrowserWindowOpen
}>

const BROWSER_AVAILABLE: CapabilityStatus = Object.freeze({
  availability: 'available',
  source: 'browser',
})

function browserSlot<Implementation>(
  implementation: Implementation,
): CapabilitySlot<Implementation> {
  return Object.freeze({ status: BROWSER_AVAILABLE, implementation })
}

export function createBrowserExternalLinkCapability(
  environment: BrowserCapabilityEnvironment,
): ExternalLinkCapability {
  return {
    async open(rawUrl) {
      const url = parseSafeExternalUrl(rawUrl, environment.pageUrl)
      const opened = environment.open(url.href, '_blank', 'noopener,noreferrer')
      if (opened) opened.opener = null
    },
  }
}

/**
 * Browser registry. Ordinary external links and DOM-scoped image paste are
 * available; native paths, native capture, speech, and Desktop administration
 * remain explicitly unavailable.
 */
export function createBrowserWorkbenchCapabilities(
  environment: BrowserCapabilityEnvironment = browserEnvironment(),
): WorkbenchCapabilities {
  const unavailable = createUnavailableWorkbenchCapabilities()
  const { manifest: _manifest, ...slots } = unavailable
  return createWorkbenchCapabilities({
    ...slots,
    externalLinks: browserSlot(createBrowserExternalLinkCapability(environment)),
    imagePaste: browserSlot(createBrowserImagePasteCapability()),
  })
}

function browserEnvironment(): BrowserCapabilityEnvironment {
  return {
    pageUrl: window.location.href,
    open: window.open.bind(window),
  }
}

function parseSafeExternalUrl(rawUrl: string, pageUrl: string): URL {
  let url: URL
  try {
    url = new URL(rawUrl, pageUrl)
  } catch {
    throw new TypeError('External link URL is invalid.')
  }
  if (url.protocol !== 'https:' && url.protocol !== 'http:' && url.protocol !== 'mailto:') {
    throw new TypeError(`External link protocol is not allowed: ${url.protocol}`)
  }
  return url
}
