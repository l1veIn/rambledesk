// @vitest-environment jsdom
import type { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, expect, it, vi } from 'vitest'
import TiptapInput from './TiptapInput.svelte'

let view: ReturnType<typeof mount> | undefined

afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
})

it('keeps one editing history while labels and readonly state change, and destroys its editor on unmount', async () => {
  const state = writable({ disabled: false, label: 'Answer', placeholder: 'Write here', field: 'first' })
  const props = fromStore(state)
  const created = vi.fn<(editor: Editor) => void>()
  const updated = vi.fn()
  view = mount(TiptapInput, { target: document.body, props: {
    options: { extensions: [StarterKit], content: '<p>Initial</p>', onCreate: ({ editor }) => created(editor), onUpdate: updated },
    get disabled() { return props.current.disabled },
    get label() { return props.current.label },
    get placeholder() { return props.current.placeholder },
    get attributes() { return { 'data-field': props.current.field } },
  } })
  await vi.waitFor(() => expect(created).toHaveBeenCalledOnce())
  const editor = created.mock.calls[0][0]
  const surface = document.querySelector<HTMLElement>('[role="textbox"]')!
  expect(surface.getAttribute('aria-label')).toBe('Answer')
  expect(surface.getAttribute('data-empty')).toBe('false')
  editor.commands.setTextSelection(8)
  editor.commands.insertContent(' changed')
  expect(editor.getText()).toBe('Initial changed')
  expect(updated).toHaveBeenCalledOnce()

  state.set({ disabled: true, label: 'Saved answer', placeholder: 'New prompt', field: 'second' })
  await tick()
  expect(editor.isEditable).toBe(false)
  expect(surface.getAttribute('contenteditable')).toBe('false')
  expect(surface.getAttribute('aria-label')).toBe('Saved answer')
  expect(surface.getAttribute('data-placeholder')).toBe('New prompt')
  expect(surface.getAttribute('data-field')).toBe('second')
  expect(created).toHaveBeenCalledOnce()
  expect(updated).toHaveBeenCalledOnce()

  state.update((current) => ({ ...current, disabled: false }))
  await tick()
  expect(editor.commands.undo()).toBe(true)
  expect(editor.getText()).toBe('Initial')
  editor.commands.clearContent()
  expect(surface.getAttribute('data-empty')).toBe('true')
  await unmount(view)
  view = undefined
  expect(editor.isDestroyed).toBe(true)
})
