// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ApplicationTransport } from '../../application/applicationTransport'
import type { FeedbackWorkspaceView } from '../../feedback'
import type { InputTarget } from '../../domain/inputTarget'
import type { MediaReviewData } from '../../generated/feedback'
import { locale } from '../../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../../speech/voiceInputContext'
import { inputText, replaceInputText } from '../../../test/tiptap'
import { mediaReviewDefinition } from '../definitions/media_review/definition'
import type { MediaReviewState } from '../definitions/media_review/state'
import { createMediaReviewController } from './mediaController'
import MediaReviewWorkbench from './MediaReviewWorkbench.svelte'

const data: MediaReviewData = { title: 'Review motion', source_version: 'motion-v1', media_kind: 'video', media_file_name: 'clip.webm', duration_ms: 8000 }
const saved: MediaReviewState = { type: 'media_review', comments: [
  { id: 'later', start_ms: 6000, end_ms: null, body: 'Later comment' },
  { id: 'range', start_ms: 1000, end_ms: 2500, body: 'Saved range' },
] }
const documentTarget: InputTarget = { requestId: 'review', requestTitle: 'Review', destination: { kind: 'document', action: null } }
let view: ReturnType<typeof mount> | undefined
let dispose: (() => void) | undefined
beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => setTimeout(() => callback(0), 0))
  vi.stubGlobal('cancelAnimationFrame', (id: number) => clearTimeout(id))
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {})
  vi.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => {})
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue()
})
afterEach(async () => {
  if (view) await unmount(view)
  dispose?.(); view = undefined; dispose = undefined
  document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals()
})
async function open(state: MediaReviewState | null = null, input = data, readOnly = false) {
  const source = writable({ state, data: input, readOnly, disabled: false, onOpenExpanded: vi.fn() as (() => void) | undefined })
  const props = fromStore(source), onChange = vi.fn((next: MediaReviewState) => source.update((value) => ({ ...value, state: next })))
  const voiceState = writable<VoiceInputState>({ requestId: 'review', documentTarget, nextTarget: documentTarget, disabled: false, recording: false })
  const selectTarget = vi.fn(), call = vi.fn(async () => new ArrayBuffer(8)), revoke = vi.fn()
  const runtime = createMediaReviewController({ requestId: 'review', getState: () => props.current.state,
    getWorkspace: () => ({ request: { request_id: 'review' }, workbench: { type: 'media_review', version: 1, data: props.current.data },
      request_attachments: [{ attachment_id: 'source', file_name: props.current.data.media_file_name, media_type: `${props.current.data.media_kind}/webm` }] }) as unknown as FeedbackWorkspaceView,
    updateState: (next) => {
      if (next.type !== 'media_review') throw new Error(`Unexpected workbench state: ${next.type}`)
      onChange(next)
    }, isEditable: () => !props.current.readOnly && !props.current.disabled, setBusy: vi.fn(),
    runtime: { transport: { call } as unknown as ApplicationTransport } }, { create: () => 'blob:controlled-source', revoke })
  dispose = runtime.dispose
  view = mount(MediaReviewWorkbench, { target: document.body, props: { get state() { return props.current.state }, get data() { return props.current.data },
    get readOnly() { return props.current.readOnly }, get disabled() { return props.current.disabled }, get onOpenExpanded() { return props.current.onOpenExpanded },
    requestId: 'review', runtime, onChange }, context: new Map<symbol, unknown>([[VOICE_INPUT_CONTEXT,
    { state: voiceState, selectTarget, start: vi.fn(), stop: vi.fn() }]]) })
  await tick(); await vi.waitFor(() => expect(media().getAttribute('src')).toBe('blob:controlled-source'))
  return { source, props, onChange, voiceState, selectTarget, runtime, call, revoke }
}
const media = () => document.querySelector<HTMLMediaElement>('video,audio')!
const button = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((node) => node.textContent?.trim() === label)!
const editor = () => document.querySelector<HTMLElement>('[data-media-comment-editor] [contenteditable]')!
const seek = () => document.querySelector<HTMLInputElement>('[aria-label="Seek media"]')!
async function metadata(duration = 8) {
  Object.defineProperty(media(), 'duration', { configurable: true, value: duration })
  media().dispatchEvent(new Event('loadedmetadata')); await tick()
}
async function clock(ms: number) { media().currentTime = ms / 1000; media().dispatchEvent(new Event('timeupdate')); await tick() }

describe('media review interactions', () => {
  it('renders native controls without autoplay and creates a paused point comment through the shared editor and voice target', async () => {
    const app = await open(); await metadata(); await clock(1234)
    expect(media().controls).toBe(true); expect(media().autoplay).toBe(false)
    expect(media().play).not.toHaveBeenCalled(); expect(app.onChange).not.toHaveBeenCalled()
    button('Comment at current time').click(); await tick()
    expect(app.props.current.state?.comments[0]).toMatchObject({ start_ms: 1234, end_ms: null, body: '' })
    expect(media().pause).toHaveBeenCalled()
    replaceInputText(editor(), 'Make the sound quieter.'); await tick()
    expect(app.props.current.state?.comments[0].body).toBe('Make the sound quieter.')
    editor().dispatchEvent(new FocusEvent('focus'))
    expect(app.selectTarget.mock.lastCall?.[0].destination).toMatchObject({ kind: 'workbench_field', workbenchType: 'media_review', version: 1,
      field: 'comment_body', entityId: app.props.current.state?.comments[0].id, sourceVersion: data.source_version })
  })
  it('marks ranges with focused-timeline I/O, adjusts both ends and preserves the original source', async () => {
    const app = await open(), original = JSON.stringify(data); await metadata(); await clock(2000)
    seek().dispatchEvent(new KeyboardEvent('keydown', { key: 'i', bubbles: true, cancelable: true })); await tick(); await clock(4000)
    seek().dispatchEvent(new KeyboardEvent('keydown', { key: 'o', bubbles: true, cancelable: true })); await tick()
    const start = document.querySelector<HTMLInputElement>('[data-tour="media-selection"] input[min="0"]')!
    start.value = '1500'; start.dispatchEvent(new Event('input', { bubbles: true })); await tick()
    button('Add range comment').click(); await tick()
    expect(app.props.current.state?.comments[0]).toMatchObject({ start_ms: 1500, end_ms: 4000 })
    expect(document.querySelector('[data-media-marker]')?.getAttribute('style')).toContain('18.75%')
    expect(JSON.stringify(data)).toBe(original)
  })
  it('sorts saved comments for display and seeks on cards or markers without modifying the creation order', async () => {
    const app = await open(saved); await metadata()
    const ids = () => [...document.querySelectorAll('[data-media-comment]')].map((element) => element.getAttribute('data-media-comment'))
    expect(ids()).toEqual(['range', 'later'])
    const order = document.querySelector<HTMLSelectElement>('[aria-label="Sort comments"]')!
    order.value = 'oldest'; order.dispatchEvent(new Event('change', { bubbles: true })); await tick()
    expect(ids()).toEqual(['later', 'range'])
    order.value = 'newest'; order.dispatchEvent(new Event('change', { bubbles: true })); await tick()
    expect(ids()).toEqual(['range', 'later'])
    document.querySelector<HTMLButtonElement>('[data-media-comment="later"]')!.click(); await tick()
    expect(media().currentTime).toBe(6); expect(inputText(editor())).toBe('Later comment')
    document.querySelector<HTMLButtonElement>('[data-media-marker="range"]')!.click(); await tick()
    expect(media().currentTime).toBe(1); expect(inputText(editor())).toBe('Saved range')
    expect(app.props.current.state).toEqual(saved); expect(app.onChange).not.toHaveBeenCalled(); expect(media().play).not.toHaveBeenCalled()
  })
  it('keeps readonly audio playback and seek available while preventing edits, creation and deletion', async () => {
    const app = await open(saved, { ...data, media_kind: 'audio' }, true); await metadata()
    expect(media().tagName).toBe('AUDIO'); expect(media().controls).toBe(true)
    document.querySelector<HTMLButtonElement>('[data-media-comment="range"]')!.click(); await tick()
    expect(media().currentTime).toBe(1); expect(editor().getAttribute('contenteditable')).toBe('false')
    expect(document.querySelector('[data-media-add-comment]')).toBeNull(); expect(document.querySelector('[aria-label="Delete comment"]')).toBeNull()
    button('Full screen review').click(); expect(app.props.current.onOpenExpanded).toHaveBeenCalledOnce()
    expect(app.onChange).not.toHaveBeenCalled(); expect(app.call).toHaveBeenCalledOnce()
  })
  it('reveals a persisted generic voice destination without creating a new comment', async () => {
    const app = await open(saved); await metadata()
    app.voiceState.update((state) => ({ ...state, revealSequence: 1, revealTarget: { ...documentTarget, destination: {
      kind: 'workbench_field', workbenchType: 'media_review', version: 1, field: 'comment_body', entityId: 'range', sourceVersion: data.source_version, label: 'Range' } } }))
    await tick(); expect(media().currentTime).toBe(1); expect(inputText(editor())).toBe('Saved range')
    expect(app.onChange).not.toHaveBeenCalled()
  })
  it('reports duration mismatch and codec failures while preserving saved anchors and draft content', async () => {
    const app = await open(saved); await metadata(4)
    expect(document.body.textContent).toContain('duration differs'); expect(button('Comment at current time').disabled).toBe(true)
    document.querySelector<HTMLButtonElement>('[data-media-comment="later"]')!.click(); await tick()
    expect(document.body.textContent).toContain('outside the playable media')
    media().dispatchEvent(new Event('error')); await tick()
    expect(document.body.textContent).toContain('browser cannot play'); expect(document.body.textContent).toContain('draft is preserved')
    button('Retry loading').click(); await tick()
    expect(app.call).toHaveBeenCalledOnce(); expect(app.props.current.state).toEqual(saved); expect(app.onChange).not.toHaveBeenCalled()
  })
  it('exposes all inline guide landmarks and detaches playback on unmount while keeping the request URL until disposal', async () => {
    const app = await open()
    for (const step of mediaReviewDefinition.guide!.steps.filter((step) => step.target !== '[data-feedback-actions]')) {
      expect(document.querySelector(step.target), step.id).not.toBeNull()
    }
    const element = media(); await unmount(view!); view = undefined
    expect(element.pause).toHaveBeenCalled(); expect(element.getAttribute('src')).toBeNull()
    expect(app.revoke).not.toHaveBeenCalled(); expect(app.onChange).not.toHaveBeenCalled()
    app.runtime.dispose(); expect(app.revoke).toHaveBeenCalledWith('blob:controlled-source')
  })
})
