import type { ApplicationTransport } from './applicationTransport'
import {
  HttpApplicationTransport,
  type HttpApplicationSession,
} from './httpApplicationTransport'
import { UnavailableApplicationTransport } from './unavailableApplicationTransport'
import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'

export type WorkbenchCompositionInput = Readonly<{
  environment: 'desktop' | 'browser'
  previewMode: boolean
  desktopTransport?: ApplicationTransport
  authenticatedWebSession?: HttpApplicationSession
  capabilities?: WorkbenchCapabilities
}>

export type WorkbenchComposition = Readonly<{
  applicationTransport: ApplicationTransport
  capabilities: WorkbenchCapabilities
  previewMode: boolean
}>

/** Selects implementations only; credentials and native bindings are composed outside. */
export function createWorkbenchComposition(
  input: WorkbenchCompositionInput,
): WorkbenchComposition {
  const capabilities = input.capabilities ?? createUnavailableWorkbenchCapabilities()
  if (input.previewMode) {
    return {
      applicationTransport: new UnavailableApplicationTransport(capabilities.manifest),
      capabilities,
      previewMode: true,
    }
  }
  if (input.environment === 'desktop') {
    if (!input.desktopTransport) {
      throw new Error('Desktop composition requires a Tauri ApplicationTransport implementation.')
    }
    return { applicationTransport: input.desktopTransport, capabilities, previewMode: false }
  }
  if (input.authenticatedWebSession) {
    return {
      applicationTransport: new HttpApplicationTransport(
        input.authenticatedWebSession.lease(),
        capabilities.manifest,
      ),
      capabilities,
      previewMode: false,
    }
  }
  return {
    applicationTransport: new UnavailableApplicationTransport(capabilities.manifest),
    capabilities,
    previewMode: false,
  }
}
