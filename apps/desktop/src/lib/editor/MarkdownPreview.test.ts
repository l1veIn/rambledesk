// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

import MarkdownPreview from './MarkdownPreview.svelte'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 30))
}

describe('MarkdownPreview', () => {
  let host: HTMLElement

  afterEach(() => {
    host?.remove()
    vi.unstubAllGlobals()
  })

  it('renders what-happened style markdown as prose', async () => {
    host = document.createElement('div')
    document.body.append(host)
    const app = mount(MarkdownPreview, {
      target: host,
      props: {
        bare: true,
        markdown: [
          '## Root cause',
          '',
          'The tab strip never scrolled because **wheel** events were ignored.',
          '',
          '- first finding',
          '- second finding',
          '',
          '```',
          'pnpm dev',
          '```',
        ].join('\n'),
      },
    })
    await settle()

    expect(host.querySelector('h2')?.textContent).toBe('Root cause')
    expect(host.querySelectorAll('li')).toHaveLength(2)
    expect(host.querySelector('strong')?.textContent).toBe('wheel')
    expect(host.querySelector('pre')?.textContent).toContain('pnpm dev')
    await unmount(app)
  })

  it('keeps single newlines as line breaks like the previous plain-text view', async () => {
    host = document.createElement('div')
    document.body.append(host)
    const app = mount(MarkdownPreview, {
      target: host,
      props: { bare: true, markdown: 'first line\nsecond line' },
    })
    await settle()

    const paragraph = host.querySelector('p')
    expect(paragraph?.querySelectorAll('br')).toHaveLength(1)
    expect(paragraph?.textContent).toBe('first linesecond line')
    await unmount(app)
  })

  it('keeps the bare variant free of card chrome', async () => {
    host = document.createElement('div')
    document.body.append(host)
    const app = mount(MarkdownPreview, {
      target: host,
      props: { bare: true, markdown: 'plain' },
    })
    await settle()

    const root = host.firstElementChild as HTMLElement
    expect(root.className).toBe('min-h-0')
    expect(root.className).not.toContain('border')
    await unmount(app)
  })
})
