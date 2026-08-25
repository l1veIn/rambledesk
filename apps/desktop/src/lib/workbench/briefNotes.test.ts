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
  capturedTranscriptMarkdown,
  replaceBlockNote,
  replaceLastBlock,
  replaceCapture,
  replaceNthBlock,
  replaceRambleClip,
  sameCaptureOccurrence,
  upsertCapture,
  hasCapture,
  wrapCapture,
  parseCaptures,
  extractNoteBody,
  nextSavedTranscript,
  blockNoteCaptureId,
  tooltipFixedStyle,
  mergeLiveTranscript,
  isCaptureTooltipEvent,
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

  it('keeps a processing placeholder clip with empty text', () => {
    expect(appendRambleClip([], '', 'ramble:pending', true)).toEqual([
      { id: 'ramble:pending', text: '', processing: true },
    ])
  })
})

describe('appendBlockNote', () => {
  it('keeps a single note per block and appends later speech to it', () => {
    const once = appendBlockNote({}, 'action:a1', 'too small')
    const twice = appendBlockNote(once, 'action:a1', 'still hidden')
    expect(twice['action:a1']).toEqual(['too small\nstill hidden'])
  })
})

describe('blockNoteCaptureId', () => {
  it('uses a stable id so later speech replaces the same capture', () => {
    expect(blockNoteCaptureId('action:a1')).toBe('note:action:a1:0')
    expect(blockNoteCaptureId('what_happened:0')).toBe('note:what_happened:0:0')
  })
})

describe('mergeLiveTranscript', () => {
  it('keeps the in-flight partial so a stop before VAD still has text', () => {
    expect(mergeLiveTranscript(['hello'], 'world')).toBe('hello\nworld')
    expect(mergeLiveTranscript([], 'only partial')).toBe('only partial')
    expect(mergeLiveTranscript(['hello'], '  ')).toBe('hello')
  })

  it('does not duplicate a partial that is already in the stables', () => {
    expect(mergeLiveTranscript(['hello world'], 'hello world')).toBe('hello world')
    expect(mergeLiveTranscript(['hello'], 'hello world')).toBe('hello world')
    expect(mergeLiveTranscript(['hello world'], 'world')).toBe('hello world')
  })
})

describe('mergeLiveTranscript at stop time', () => {
  it('drops the partial the closing stable segment refined', () => {
    expect(mergeLiveTranscript(['第一段', '第二段', '第三段。'], '第三段。')).toBe(
      '第一段\n第二段\n第三段。',
    )
  })

  it('keeps the partial when the recogniser never finalised it', () => {
    expect(mergeLiveTranscript(['第一段', '第二段'], '第三段')).toBe('第一段\n第二段\n第三段')
  })
})

describe('isCaptureTooltipEvent', () => {
  it('treats clicks inside a portaled tooltip as inside', () => {
    const inside = { closest: (selector: string) => (selector === '[data-capture-tooltip]' ? {} : null) }
    const outside = { closest: () => null }
    expect(isCaptureTooltipEvent(inside)).toBe(true)
    expect(isCaptureTooltipEvent(outside)).toBe(false)
    expect(isCaptureTooltipEvent(null)).toBe(false)
  })
})

describe('tooltipFixedStyle', () => {
  it('places a footer clip tooltip above the icon, outside overflow clipping', () => {
    expect(
      tooltipFixedStyle(
        { left: 40, top: 800, width: 32, height: 32 },
        'top',
        'left',
      ),
    ).toEqual({ top: 792, left: 40, transform: 'translateY(-100%)' })
  })

  it('places a block-note tooltip below the right edge of the icon', () => {
    expect(
      tooltipFixedStyle(
        { left: 900, top: 120, width: 24, height: 24 },
        'bottom',
        'right',
      ),
    ).toEqual({ top: 152, left: 924, transform: 'translateX(-100%)' })
  })
})

describe('capturedTranscriptMarkdown', () => {
  it('turns spoken line breaks into markdown paragraphs', () => {
    expect(capturedTranscriptMarkdown('hello\nworld')).toBe('hello\n\nworld')
  })
})

describe('parseCaptures', () => {
  it('rebuilds clips and notes from persisted capture markers', () => {
    const body = [
      wrapCapture('ramble:abc', 'first clip'),
      wrapCapture('note:action:a1:0', quotedNoteMarkdown('Open login', 'too small')),
    ].join('\n\n')
    expect(parseCaptures(body)).toEqual({
      clips: [{ id: 'ramble:abc', text: 'first clip' }],
      notes: { 'action:a1': ['too small'] },
    })
  })

  it('extracts the note body under a quote', () => {
    expect(extractNoteBody(quotedNoteMarkdown('Button is small', 'needs contrast'))).toBe(
      'needs contrast',
    )
  })
})

describe('wrapCapture', () => {
  it('replaces one wrapped clip without touching a duplicate sibling', () => {
    const first = wrapCapture('ramble:0', 'hello')
    const second = wrapCapture('ramble:1', 'hello')
    const body = `${first}\n\n${second}`
    const updated = replaceCapture(body, 'ramble:0', 'first')
    expect(updated).toContain(wrapCapture('ramble:0', 'first'))
    expect(updated).toContain(wrapCapture('ramble:1', 'hello'))
    expect(updated.indexOf('first')).toBeLessThan(updated.lastIndexOf('hello'))
  })
})

describe('upsertCapture', () => {
  it('appends a wrapped block the first time a note is spoken', () => {
    const body = upsertCapture('Existing notes.', 'note:action:a1:0', 'too small')
    expect(body).toBe(`Existing notes.\n\n${wrapCapture('note:action:a1:0', 'too small')}`)
    expect(hasCapture(body, 'note:action:a1:0')).toBe(true)
  })

  it('rewrites the same block in place when more speech is added', () => {
    const first = upsertCapture('', 'note:action:a1:0', 'too small')
    const second = upsertCapture(first, 'note:action:a1:0', 'too small\nand too pale')
    expect(second).toBe(wrapCapture('note:action:a1:0', 'too small\nand too pale'))
  })

  it('does not append a second copy when the text is unchanged', () => {
    const first = upsertCapture('', 'ramble:abc', 'hello')
    expect(upsertCapture(first, 'ramble:abc', 'hello')).toBe(first)
  })

  it('leaves a sibling capture alone', () => {
    const body = [wrapCapture('ramble:a', 'one'), wrapCapture('ramble:b', 'two')].join('\n\n')
    const updated = upsertCapture(body, 'ramble:a', 'ONE')
    expect(updated).toContain(wrapCapture('ramble:a', 'ONE'))
    expect(updated).toContain(wrapCapture('ramble:b', 'two'))
  })

  it('reports a missing capture so an edit can fall back to a text match', () => {
    expect(hasCapture('plain body', 'ramble:a')).toBe(false)
  })
})

describe('replaceNthBlock', () => {
  it('replaces the selected duplicate, not always the last one', () => {
    expect(replaceNthBlock('hello\n\nhello', 'hello', 'first', 0)).toBe('first\n\nhello')
    expect(replaceNthBlock('hello\n\nhello', 'hello', 'second', 1)).toBe('hello\n\nsecond')
  })
})

describe('sameCaptureOccurrence', () => {
  it('counts earlier clips that share the same markdown', () => {
    const clips = appendRambleClip(appendRambleClip([], 'hello'), 'hello')
    expect(sameCaptureOccurrence(clips.map((clip) => clip.text), 0)).toBe(0)
    expect(sameCaptureOccurrence(clips.map((clip) => clip.text), 1)).toBe(1)
  })
})

describe('replaceLastBlock', () => {
  it('replaces the latest matching transcript in the draft', () => {
    expect(replaceLastBlock('keep\n\nhello\n\nhello', 'hello', 'fixed')).toBe('keep\n\nhello\n\nfixed')
  })

  it('finds a clip even when the editor serialized extra paragraph breaks', () => {
    expect(replaceLastBlock('intro\n\nhello\n\nworld', 'hello\nworld', 'hello world')).toBe(
      'intro\n\nhello world',
    )
  })

  it('replaces a quoted note without touching earlier quotes', () => {
    const first = quotedNoteMarkdown('Button is small', 'too small')
    const second = quotedNoteMarkdown('Button is small', 'still small')
    const body = `${first}\n\n${second}`
    expect(replaceLastBlock(body, second, quotedNoteMarkdown('Button is small', 'needs contrast'))).toBe(
      `${first}\n\n${quotedNoteMarkdown('Button is small', 'needs contrast')}`,
    )
  })
})

describe('replaceRambleClip', () => {
  it('updates one clip by id', () => {
    const clips = appendRambleClip(appendRambleClip([], 'first'), 'second')
    expect(replaceRambleClip(clips, 'ramble:0', 'first fixed').map((clip) => clip.text)).toEqual([
      'first fixed',
      'second',
    ])
  })
})

describe('replaceBlockNote', () => {
  it('updates the single note on a block', () => {
    const notes = appendBlockNote({}, 'action:a1', 'too small')
    expect(replaceBlockNote(notes, 'action:a1', 0, 'still too small')['action:a1']).toEqual([
      'still too small',
    ])
  })
})

describe('nextSavedTranscript', () => {
  it('returns the trimmed draft when it differs from the saved text', () => {
    expect(nextSavedTranscript('  hello world  ', 'hello')).toBe('hello world')
  })

  it('returns null when the draft is blank or unchanged', () => {
    expect(nextSavedTranscript('   ', 'hello')).toBeNull()
    expect(nextSavedTranscript('hello', 'hello')).toBeNull()
    expect(nextSavedTranscript('  hello  ', 'hello')).toBeNull()
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
