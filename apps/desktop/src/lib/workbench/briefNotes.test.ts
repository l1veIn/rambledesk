import { describe, expect, it } from 'vitest'

import {
  appendBlockNote,
  appendRambleClip,
  briefBlocks,
  clipFlyTransform,
  findBriefBlock,
  joinTranscriptChunks,
  quotedNoteMarkdown,
  rambleRequestIdAfterIdleNote,
} from './briefNotes'

describe('briefBlocks', () => {
  it('splits what happened into paragraph blocks and keeps actions and context', () => {
    const blocks = briefBlocks({
      whatHappened: 'The login form broke.\n\nScreenshots are attached.',
      actions: [
        { id: 'a1', instruction: 'Open the login page' },
        { id: 'a2', instruction: 'Submit an empty form' },
      ],
      contextRefs: [{ label: 'PR', uri: 'https://example.com/pr/1' }],
    })

    expect(blocks.map((block) => ({ id: block.id, kind: block.kind, quote: block.quote }))).toEqual([
      { id: 'what_happened:0', kind: 'what_happened', quote: 'The login form broke.' },
      { id: 'what_happened:1', kind: 'what_happened', quote: 'Screenshots are attached.' },
      { id: 'action:a1', kind: 'action', quote: 'Open the login page' },
      { id: 'action:a2', kind: 'action', quote: 'Submit an empty form' },
      { id: 'context:0', kind: 'context', quote: 'PR\nhttps://example.com/pr/1' },
    ])
  })

  it('skips empty paragraphs', () => {
    expect(
      briefBlocks({
        whatHappened: '\n\nOnly this.\n\n  \n\n',
        actions: [],
        contextRefs: [],
      }),
    ).toEqual([
      expect.objectContaining({ id: 'what_happened:0', quote: 'Only this.' }),
    ])
  })
})

describe('findBriefBlock', () => {
  it('finds a block by id', () => {
    const blocks = briefBlocks({
      whatHappened: 'Hello',
      actions: [{ id: 'step', instruction: 'Click save' }],
      contextRefs: [],
    })
    expect(findBriefBlock(blocks, 'action:step')?.quote).toBe('Click save')
    expect(findBriefBlock(blocks, 'missing')).toBeUndefined()
  })
})

describe('rambleRequestIdAfterIdleNote', () => {
  it('uses the current workspace when ramble is idle, even if a leftover id remains', () => {
    expect(rambleRequestIdAfterIdleNote('idle', 'request-a', 'request-b')).toBe('request-b')
  })

  it('keeps the ramble session id while ramble is still engaged', () => {
    expect(rambleRequestIdAfterIdleNote('paused', 'request-a', 'request-b')).toBe('request-a')
  })
})

describe('clipFlyTransform', () => {
  it('starts the clip over the record button so it can rack to the left', () => {
    expect(
      clipFlyTransform(
        { left: 800, top: 40, width: 120, height: 32 },
        { left: 24, top: 40, width: 32, height: 32 },
      ),
    ).toEqual({
      x: 820,
      y: 0,
      scale: 2.2,
    })
  })
})

describe('joinTranscriptChunks', () => {
  it('joins spoken segments into one clip', () => {
    expect(joinTranscriptChunks(['  hello  ', '', 'world'])).toBe('hello\nworld')
  })
})

describe('appendRambleClip', () => {
  it('appends a clip for each start-stop cycle', () => {
    const first = appendRambleClip([], 'first ramble')
    const second = appendRambleClip(first, 'second ramble')
    expect(second).toEqual([
      { id: 'ramble:0', text: 'first ramble' },
      { id: 'ramble:1', text: 'second ramble' },
    ])
  })

  it('ignores blank clips', () => {
    expect(appendRambleClip([], '  ')).toEqual([])
  })
})

describe('appendBlockNote', () => {
  it('keeps notes on the same block together', () => {
    const once = appendBlockNote({}, 'action:a1', 'too small')
    const twice = appendBlockNote(once, 'action:a1', 'still hidden')
    expect(twice['action:a1']).toEqual(['too small', 'still hidden'])
  })
})

describe('quotedNoteMarkdown', () => {
  it('quotes the block and puts the note underneath', () => {
    expect(quotedNoteMarkdown('The submit button is hidden', 'I had to tab to find it.')).toBe(
      '> The submit button is hidden\n\nI had to tab to find it.',
    )
  })

  it('quotes every line of a multiline block', () => {
    expect(quotedNoteMarkdown('line one\nline two', 'note')).toBe('> line one\n> line two\n\nnote')
  })

  it('returns empty when the note is blank', () => {
    expect(quotedNoteMarkdown('quoted', '  ')).toBe('')
  })
})
