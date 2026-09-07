import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import DraftManagedSessionWorkspace from './DraftManagedSessionWorkspace.svelte'
import { createDraftManagedSessionController } from './draftManagedSessionController'
import { createManagedSessionDraftStorage } from './managedSessionDrafts'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

describe('compact new session workspace', () => {
  it('places the project above the composer and the agent selector beside send, with an actionable missing-folder state', () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    const data = new Map<string, string>()
    const storage = createManagedSessionDraftStorage({ getItem: key => data.get(key) ?? null, setItem: (key, value) => { data.set(key, value) } })
    storage.save('draft', { choice: '', cwd: '', text: 'Preserved task' })
    const controller = createDraftManagedSessionController(transport, 'draft', storage, vi.fn())
    const onConfigure = vi.fn()
    const onChooseDirectory = vi.fn()
    const body = render(DraftManagedSessionWorkspace, { props: { transport, controller, draftId: 'draft', onConfigure, onChooseDirectory } }).body
    expect(body).toContain('Choose a project folder to start.')
    expect(body).toContain('aria-label="Project directory: Choose a project"')
    expect(body).toContain('aria-label="Choose an agent: Choose an agent"')
    expect(body.indexOf('aria-label="Project directory:')).toBeLessThan(body.indexOf('agent-composer relative'))
    expect(body.indexOf('agent-composer relative')).toBeLessThan(body.indexOf('aria-label="Choose an agent:'))
    expect(body.indexOf('aria-label="Choose an agent:')).toBeLessThan(body.indexOf('aria-label="Send message"'))
    expect(body).toMatch(/<button[^>]*disabled[^>]*aria-label="Send message"|<button[^>]*aria-label="Send message"[^>]*disabled/u)
    expect(body).not.toContain('<header')
    expect(body).not.toContain('<select')
    expect(transport.calls).toHaveLength(0)
    expect(onConfigure).not.toHaveBeenCalled()
    expect(onChooseDirectory).not.toHaveBeenCalled()
  })
})
