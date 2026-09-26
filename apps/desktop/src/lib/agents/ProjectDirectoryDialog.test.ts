// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { BrowseProjectDirectoriesInput, ProjectDirectoryError, ProjectDirectoryListing } from '$lib/generated/feedback'
import { locale } from '$lib/preferences'
import ProjectDirectoryDialog from './ProjectDirectoryDialog.svelte'

let view: ReturnType<typeof mount> | undefined
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]')
const trigger = () => document.querySelector<HTMLButtonElement>('[aria-label^="Project directory:"]')!
const button = (label: string) => [...(dialog()?.querySelectorAll<HTMLButtonElement>('button') ?? [])]
  .find(element => element.textContent?.trim() === label || element.getAttribute('aria-label') === label)!
const input = () => document.querySelector<HTMLInputElement>('[aria-label="Folder path"]')!
const selectedPath = () => document.querySelector('[aria-label="Selected folder"]')?.textContent

function listing(path = '/home', overrides: Partial<ProjectDirectoryListing> = {}): ProjectDirectoryListing {
  return { path, parent_path: '/', home_path: '/home', roots: [{ name: 'Disk', path: '/' }],
    directories: [{ name: 'Project', path: `${path}/project-link` }], ...overrides }
}
function browseError(code: ProjectDirectoryError['code']): ProjectDirectoryError {
  return { code, message: 'Could not browse this directory.', retryable: code === 'DIRECTORY_UNAVAILABLE' }
}
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  Element.prototype.getAnimations = (() => []) as never
  HTMLElement.prototype.scrollIntoView = vi.fn()
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})
function mountDialog(value = '/home') {
  const browse = vi.fn<(input: BrowseProjectDirectoriesInput) => Promise<ProjectDirectoryListing>>()
    .mockResolvedValue(listing())
  const transport = new TestApplicationTransport().handle('browseProjectDirectories', browse)
  const onSelect = vi.fn()
  const disabled = fromStore(writable(false))
  view = mount(ProjectDirectoryDialog, { target: document.body, props: {
    transport, value, onSelect, get disabled() { return disabled.current },
  } })
  return { browse, transport, onSelect, disabled }
}
async function openDialog() {
  await tick()
  trigger().focus()
  trigger().click()
  await vi.waitFor(() => expect(dialog()).not.toBeNull())
}
async function waitForPath(path: string) {
  await vi.waitFor(() => {
    expect(input().value).toBe(path)
    expect(selectedPath()).toBe(path)
    expect(button('Use this folder').disabled).toBe(false)
  })
}
async function editPath(path: string) {
  input().value = path
  input().dispatchEvent(new Event('input', { bubbles: true }))
  await tick()
}

describe('project directory browser', () => {
  it.each([
    { value: '  /home/../project  ', requested: '/home/../project' },
    { value: '', requested: null },
  ])('browses $requested on open and waits for a verified canonical directory', async ({ value, requested }) => {
    const { browse, onSelect } = mountDialog(value)
    const pending = deferred<ProjectDirectoryListing>()
    browse.mockReturnValueOnce(pending.promise)
    expect(browse).not.toHaveBeenCalled()
    await openDialog()
    expect(browse).toHaveBeenCalledExactlyOnceWith({ path: requested })
    expect(button('Use this folder').disabled).toBe(true)
    expect(input().disabled).toBe(true)
    expect(dialog()?.textContent).toContain('Loading folders…')
    await vi.waitFor(() => expect(document.activeElement).toBe(dialog()?.querySelector('[role="heading"]')))
    pending.resolve(listing('/canonical/project'))
    await waitForPath('/canonical/project')
    expect(onSelect).not.toHaveBeenCalled()
  })

  it('only browses when a folder is clicked and commits the server canonical path on confirmation', async () => {
    const { browse, onSelect } = mountDialog()
    await openDialog()
    await waitForPath('/home')
    const pending = deferred<ProjectDirectoryListing>()
    browse.mockReturnValueOnce(pending.promise)
    button('Project').click()
    await tick()
    expect(browse).toHaveBeenLastCalledWith({ path: '/home/project-link' })
    expect(onSelect).not.toHaveBeenCalled()
    expect(button('Use this folder').disabled).toBe(true)
    pending.resolve(listing('/canonical/project', { directories: [] }))
    await waitForPath('/canonical/project')
    expect(dialog()?.textContent).toContain('No subfolders. You can select this folder.')
    expect(onSelect).not.toHaveBeenCalled()
    button('Use this folder').click()
    await vi.waitFor(() => expect(dialog()).toBeNull())
    expect(onSelect).toHaveBeenCalledExactlyOnceWith('/canonical/project')
    await vi.waitFor(() => expect(document.activeElement).toBe(trigger()))
  })

  it.each(['Cancel', 'Escape'])('does not select a browsed directory when dismissed with %s and returns focus', async dismissal => {
    const { browse, onSelect } = mountDialog()
    await openDialog()
    await waitForPath('/home')
    browse.mockResolvedValueOnce(listing('/canonical/project'))
    button('Project').click()
    await waitForPath('/canonical/project')
    if (dismissal === 'Cancel') button('Cancel').click()
    else document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(dialog()).toBeNull())
    await vi.waitFor(() => expect(document.activeElement).toBe(trigger()))
    expect(onSelect).not.toHaveBeenCalled()
  })

  it('requires a typed path to be browsed before it can be confirmed', async () => {
    const { browse, onSelect } = mountDialog()
    await openDialog()
    await waitForPath('/home')
    await editPath('/somewhere/../project')
    expect(button('Use this folder').disabled).toBe(true)
    button('Use this folder').click()
    expect(onSelect).not.toHaveBeenCalled()
    expect(browse).toHaveBeenCalledTimes(1)
    browse.mockResolvedValueOnce(listing('/project'))
    input().closest('form')!.requestSubmit()
    await waitForPath('/project')
    expect(browse).toHaveBeenLastCalledWith({ path: '/somewhere/../project' })
    button('Use this folder').click()
    expect(onSelect).toHaveBeenCalledExactlyOnceWith('/project')
  })

  it('does not allow stale results to be selected after a failed browse and retries the failed path', async () => {
    const { browse, onSelect } = mountDialog()
    await openDialog()
    await waitForPath('/home')
    browse.mockRejectedValueOnce(browseError('DIRECTORY_NOT_FOUND'))
    await editPath('/missing')
    button('Go').click()
    await vi.waitFor(() => expect(dialog()?.querySelector('[role="alert"]')?.textContent).toBe('The folder was not found.'))
    expect(button('Use this folder').disabled).toBe(true)
    button('Use this folder').click()
    expect(onSelect).not.toHaveBeenCalled()
    browse.mockResolvedValueOnce(listing('/missing'))
    button('Retry').click()
    await waitForPath('/missing')
    expect(browse.mock.calls.map(([request]) => request.path)).toEqual(['/home', '/missing', '/missing'])
    expect(dialog()?.querySelector('[role="alert"]')).toBeNull()
  })

  it('can recover from an unavailable initial path through the device home directory', async () => {
    const { browse, onSelect } = mountDialog('/protected')
    browse.mockRejectedValueOnce(browseError('DIRECTORY_ACCESS_DENIED'))
    await openDialog()
    await vi.waitFor(() => expect(dialog()?.querySelector('[role="alert"]')?.textContent).toBe('You do not have permission to open this folder.'))
    expect(button('Use this folder').disabled).toBe(true)
    expect(button('Parent directory').disabled).toBe(true)
    button('Home directory').click()
    await waitForPath('/home')
    expect(browse.mock.calls.map(([request]) => request.path)).toEqual(['/protected', null])
    expect(onSelect).not.toHaveBeenCalled()
  })

  it.each(['success', 'failure'])('ignores a late %s from a folder superseded by location navigation', async outcome => {
    const { browse } = mountDialog()
    await openDialog()
    await waitForPath('/home')
    const slow = deferred<ProjectDirectoryListing>()
    const current = deferred<ProjectDirectoryListing>()
    browse.mockReturnValueOnce(slow.promise).mockReturnValueOnce(current.promise)
    button('Project').click()
    await tick()
    button('Disk').click()
    await tick()
    expect(browse).toHaveBeenLastCalledWith({ path: '/' })
    current.resolve(listing('/', { parent_path: null, directories: [] }))
    await waitForPath('/')
    if (outcome === 'success') slow.resolve(listing('/stale-project'))
    else slow.reject(browseError('DIRECTORY_NOT_FOUND'))
    await slow.promise.catch(() => {})
    await tick()
    expect(input().value).toBe('/')
    expect(selectedPath()).toBe('/')
    expect(dialog()?.querySelector('[role="alert"]')).toBeNull()
    expect(button('Use this folder').disabled).toBe(false)
    expect(button('Parent directory').disabled).toBe(true)
  })

  it('ignores a browse that completes after cancellation and a fresh open', async () => {
    const { browse, onSelect } = mountDialog()
    const old = deferred<ProjectDirectoryListing>()
    const current = deferred<ProjectDirectoryListing>()
    browse.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise)
    await openDialog()
    button('Cancel').click()
    await vi.waitFor(() => expect(dialog()).toBeNull())
    await openDialog()
    expect(button('Use this folder').disabled).toBe(true)
    current.resolve(listing('/new-home'))
    await waitForPath('/new-home')
    old.resolve(listing('/stale-home'))
    await old.promise
    await tick()
    expect(input().value).toBe('/new-home')
    expect(selectedPath()).toBe('/new-home')
    expect(onSelect).not.toHaveBeenCalled()
  })

  it('closes an open browser without selection if the session becomes locked', async () => {
    const { browse, disabled, onSelect } = mountDialog()
    const pending = deferred<ProjectDirectoryListing>()
    browse.mockReturnValueOnce(pending.promise)
    await openDialog()
    disabled.current = true
    await vi.waitFor(() => expect(dialog()).toBeNull())
    expect(trigger().disabled).toBe(true)
    pending.resolve(listing('/late'))
    await pending.promise
    await tick()
    expect(onSelect).not.toHaveBeenCalled()
    expect(dialog()).toBeNull()
  })
})
