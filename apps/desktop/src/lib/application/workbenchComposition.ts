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
  /** Fixture-backed transport for the `?preview=fixtures` workbench. */
  previewTransport?: ApplicationTransport
  desktopTransport?: ApplicationTransport
  authenticatedWebSession?: HttpApplicationSession
  capabilities?: WorkbenchCapabilities
}>

export type WorkbenchComposition = Readonly<{
  applicationTransport: ApplicationTransport
  capabilities: WorkbenchCapabilities
  previewMode: boolean
  environment: 'desktop' | 'browser'
}>

/** Selects implementations only; credentials and native bindings are composed outside. */
export function createWorkbenchComposition(
  input: WorkbenchCompositionInput,
): WorkbenchComposition {
  const capabilities = input.capabilities ?? createUnavailableWorkbenchCapabilities()
  if (input.previewTransport) {
    return {
      applicationTransport: input.previewTransport,
      capabilities,
      previewMode: true,
      environment: input.environment,
    }
  }
  if (input.environment === 'desktop') {
    if (!input.desktopTransport) {
      throw new Error('Desktop composition requires a Tauri ApplicationTransport implementation.')
    }
    return {
      applicationTransport: input.desktopTransport,
      capabilities,
      previewMode: false,
      environment: 'desktop',
    }
  }
  if (input.authenticatedWebSession) {
    return {
      applicationTransport: new HttpApplicationTransport(
        input.authenticatedWebSession.lease(),
        capabilities.manifest,
      ),
      capabilities,
      previewMode: false,
      environment: 'browser',
    }
  }
  return {
    applicationTransport: new UnavailableApplicationTransport(capabilities.manifest),
    capabilities,
    previewMode: false,
    environment: 'browser',
  }
}
