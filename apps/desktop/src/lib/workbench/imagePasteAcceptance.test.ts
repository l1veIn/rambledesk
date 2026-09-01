import { describe, expect, it } from 'vitest'

import { canAcceptImagePaste } from './imagePasteAcceptance'

describe('image-paste acceptance', () => {
  const editable = {
    loadingWorkspace: false,
    requestStatus: 'in_progress',
    interactionLocked: false,
    attachmentBusy: false,
  } as const

  it('accepts only an editable active workspace', () => {
    expect(canAcceptImagePaste(editable)).toBe(true)
  })

  it.each([
    ['loading', { loadingWorkspace: true }],
    ['completed', { requestStatus: 'completed' }],
    ['cancelled', { requestStatus: 'cancelled' }],
    ['missing', { requestStatus: null }],
    ['interaction locked', { interactionLocked: true }],
    ['attachment busy', { attachmentBusy: true }],
  ] as const)('rejects %s workspaces before the DOM event is intercepted', (_label, override) => {
    expect(canAcceptImagePaste({ ...editable, ...override })).toBe(false)
  })
})
