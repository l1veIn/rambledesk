import type { WorkspaceViewDescriptor } from './viewDescriptors'

export type WorkspaceSurface = 'inbox' | 'session' | 'standalone'

/**
 * Decides which part of the right-hand workspace belongs to the active tab.
 * Session tabs own the request list and workbench; singleton and task tabs own
 * the complete surface to the right of the Session rail.
 */
export function workspaceSurface(view: WorkspaceViewDescriptor | null): WorkspaceSurface {
  if (!view) return 'standalone'
  switch (view.kind) {
    case 'inbox':
      return 'inbox'
    case 'session':
      return 'session'
    case 'settings':
    case 'request-task':
    case 'rambelle-profile':
      return 'standalone'
  }
}
