// Adapted from xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
// SPDX-License-Identifier: Apache-2.0
// Upstream src/lib/message-quote.ts; quote text and parsing semantics retained.
/** One CommonMark quote marker; tabs remain visible user indentation. */
const QUOTE_MARKER_RE = /^ {0,3}> ?/

/** Shared by the composer decoration and transcript so their marker widths agree. */
export function quoteMarkerLength(line: string): number {
  return QUOTE_MARKER_RE.exec(line)?.[0].length ?? 0
}

/**
 * A run of a user message's text: literal prose, or a blockquote holding further
 * blocks (so `> > x` nests, which is exactly what quoting an agent message that
 * already contains a quote produces).
 */
export type QuoteBlock =
  | { kind: "text"; text: string }
  | { kind: "quote"; children: QuoteBlock[] }

/**
 * Split plain text into prose runs and blockquote runs.
 *
 * Deliberately NOT a Markdown parser: only a line-leading `>` means anything
 * here. `#`, `**`, `- `, code fences and everything else stay literal prose,
 * matching the plain-text composer — typed or pasted `> ` markers are painted as quote rules instead of shown raw.
 *
 * Strictness beyond CommonMark, on purpose: lazy continuation is not
 * implemented, so an unmarked line always ends the quote. Being conservative
 * keeps prose that merely happens to sit under a quote from being absorbed.
 *
 * ONE blank line is consumed at each quote boundary. It's the structural
 * separator between a quote and the prose around it (`> a\n\nmy question`), not
 * content — leaving it in would stack a blank line on top of the block gap.
 * Extra blank lines survive, so deliberate spacing still shows.
 *
 * Text with no quoted line at all yields exactly one `text` block holding it
 * verbatim, which callers use as a zero-risk fast path.
 */
export function parseQuoteBlocks(text: string): QuoteBlock[] {
  const lines = text.split("\n")
  const blocks: QuoteBlock[] = []
  let prose: string[] = []
  // Whether the prose being accumulated directly follows a quote block, i.e.
  // whether its leading blank line is a boundary separator to swallow.
  let afterQuote = false

  const flushProse = (beforeQuote: boolean) => {
    let start = 0
    let end = prose.length
    if (afterQuote && start < end && prose[start] === "") start += 1
    if (beforeQuote && end > start && prose[end - 1] === "") end -= 1
    if (end > start) {
      blocks.push({ kind: "text", text: prose.slice(start, end).join("\n") })
    }
    prose = []
  }

  let index = 0
  while (index < lines.length) {
    const marker = quoteMarkerLength(lines[index])
    if (marker === 0) {
      prose.push(lines[index])
      index += 1
      continue
    }
    flushProse(true)
    // Strip ONE level off every line of the run, then recurse: a second `>` on
    // those lines becomes a nested quote, anything else becomes prose.
    const inner: string[] = []
    let width = marker
    while (width > 0) {
      inner.push(lines[index].slice(width))
      index += 1
      width = index < lines.length ? quoteMarkerLength(lines[index]) : 0
    }
    blocks.push({ kind: "quote", children: parseQuoteBlocks(inner.join("\n")) })
    afterQuote = true
  }
  flushProse(false)
  return blocks
}
