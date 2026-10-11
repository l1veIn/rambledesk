import { fromMarkdown } from 'mdast-util-from-markdown'
import { gfmFromMarkdown } from 'mdast-util-gfm'
import { parse, postprocess, preprocess } from 'micromark'
import { gfm } from 'micromark-extension-gfm'
import { decodeString } from 'micromark-util-decode-string'
import type { ReviewAnnotation } from '../../generated/feedback'

interface AstNode {
  type: string; value?: string; children?: AstNode[]; depth?: number; ordered?: boolean | null; start?: number | null
  checked?: boolean | null; url?: string; alt?: string | null; identifier?: string
  position?: { start: { offset?: number }; end: { offset?: number } }
}
/** One rendered scalar (or indivisible decoded entity) and its raw source range. */
export interface SourceUnit { text: string; start: number; end: number }
export interface ReviewLeaf { id: string; text: string; units: SourceUnit[]; fallback: boolean }
export interface ReviewNode {
  tag: string; children: ReviewNode[]; leaf?: ReviewLeaf; href?: string; start?: number | null
  checked?: boolean | null; placeholder?: string; sourceStart?: number; sourceEnd?: number
}
interface InlineWrapper { start: number; end: number; visibleStart: number; visibleEnd: number }
export interface ReviewDocument { markdown: string; nodes: ReviewNode[]; leaves: Map<string, ReviewLeaf>; wrappers: InlineWrapper[]; fallback: boolean }
interface SourceToken { type: string; start: number; end: number; value: string }

/** Parser/DOM positions are UTF-16; only Unicode scalar positions cross the wire. */
function scalarOffsets(source: string): number[] {
  const offsets = Array<number>(source.length + 1).fill(-1)
  let utf16 = 0, scalar = 0
  for (const character of source) { offsets[utf16] = scalar++; utf16 += character.length }
  offsets[utf16] = scalar
  return offsets
}

function literalUnits(text: string, start: number, offsets: number[]): SourceUnit[] {
  const units: SourceUnit[] = []
  for (const character of text) {
    units.push({ text: character, start: offsets[start], end: offsets[start + character.length] })
    start += character.length
  }
  return units
}

/** Render and map from the same grammar. Never locate repeated text with indexOf. */
export function parseReviewMarkdown(markdown: string): ReviewDocument {
  try { return parseMappedMarkdown(markdown) } catch {
    // An unsupported/deeply nested document must remain reviewable as safe source text.
    return rawDocument(markdown)
  }
}

function rawDocument(markdown: string): ReviewDocument {
  const leaf: ReviewLeaf = { id: 'source-0', text: markdown, units: literalUnits(markdown, 0, scalarOffsets(markdown)), fallback: true }
  return { markdown, nodes: [{ tag: 'pre', children: [], leaf, sourceStart: 0, sourceEnd: [...markdown].length }], leaves: new Map([[leaf.id, leaf]]), wrappers: [], fallback: true }
}

function parseMappedMarkdown(markdown: string): ReviewDocument {
  const options = { extensions: [gfm()], mdastExtensions: [gfmFromMarkdown()] }
  const ast: AstNode = fromMarkdown(markdown, options)
  // Bound render complexity before a second parse or thousands of Svelte components.
  // The input remains valid and reviewable; only its presentation falls back to source.
  const pending = [{ node: ast, depth: 0 }]
  let nodeCount = 0
  while (pending.length) {
    const { node, depth } = pending.pop()!
    if (++nodeCount > 10_000 || depth > 128) return rawDocument(markdown)
    for (const child of node.children ?? []) pending.push({ node: child, depth: depth + 1 })
  }
  const events = postprocess(parse(options).document().write(preprocess()(markdown, undefined, true)))
  const visibleTokenTypes = new Set(['data', 'characterEscape', 'characterReference', 'lineEnding', 'autolinkProtocol', 'autolinkEmail', 'literalAutolink', 'codeTextData', 'codeFlowValue'])
  const tokens: SourceToken[] = events.filter(([kind, token]) => kind === 'exit' && visibleTokenTypes.has(token.type)).map(([, token, context]) => ({
    type: token.type, start: token.start.offset, end: token.end.offset, value: context.sliceSerialize(token),
  })).sort((a, b) => a.start - b.start || a.end - b.end)
  const offsets = scalarOffsets(markdown)
  const leaves = new Map<string, ReviewLeaf>(), definitions = new Map<string, string>()
  const wrappers: InlineWrapper[] = []
  const visitDefinitions = (node: AstNode) => {
    if (node.type === 'definition' && node.identifier && node.url && !definitions.has(node.identifier)) definitions.set(node.identifier, node.url)
    node.children?.forEach(visitDefinitions)
  }
  visitDefinitions(ast)
  let fallback = false
  function containedTokens(start: number, end: number, allowed: string[]): SourceToken[] {
    let low = 0, high = tokens.length
    while (low < high) { const mid = (low + high) >>> 1; if (tokens[mid].start < start) low = mid + 1; else high = mid }
    const result: SourceToken[] = []
    for (let i = Math.max(0, low - 1); i < tokens.length && tokens[i].start < end; i++) {
      const token = tokens[i]
      if (!allowed.includes(token.type) || token.end <= start) continue
      if (token.start >= start && token.end <= end) result.push(token)
      // GFM task-list compilation removes the separating space from the text node.
      else if (token.type === 'data' && markdown.slice(token.start, token.end) === token.value) {
        const from = Math.max(start, token.start), to = Math.min(end, token.end)
        result.push({ ...token, start: from, end: to, value: markdown.slice(from, to) })
      }
    }
    return result
  }
  function leaf(node: AstNode, raw = false): ReviewLeaf {
    const start = node.position?.start.offset ?? 0, end = node.position?.end.offset ?? start
    const source = markdown.slice(start, end)
    const allowed = node.type === 'code' ? ['codeFlowValue', 'lineEnding']
      : node.type === 'inlineCode' ? ['codeTextData', 'lineEnding']
      : ['data', 'characterEscape', 'characterReference', 'lineEnding', 'autolinkProtocol', 'autolinkEmail', 'literalAutolink']
    let chosen = containedTokens(start, end, allowed)
    if (node.type === 'code') {
      // MDAST excludes the opening fence's line ending and the final code line ending.
      if (/^ {0,3}(`{3,}|~{3,})/.test(source) && chosen[0]?.type === 'lineEnding') chosen = chosen.slice(1)
      if (chosen.at(-1)?.type === 'lineEnding') chosen = chosen.slice(0, -1)
    }
    const units = chosen.flatMap((token): SourceUnit[] => {
      const original = markdown.slice(token.start, token.end)
      if (token.type === 'characterEscape' || token.type === 'characterReference') {
        return [{ text: decodeString(original), start: offsets[token.start], end: offsets[token.end] }]
      }
      // Virtual indentation spaces and other rewrites must pass exact verification below.
      if (original !== token.value) return []
      return literalUnits(original, token.start, offsets)
    })
    const failed = raw || units.map((unit) => unit.text).join('') !== (node.value ?? '')
      || units.some((unit) => unit.start < 0 || unit.end < 0)
    const mapped = failed ? literalUnits(source, start, offsets) : units
    if (node.type === 'inlineCode' && !failed) {
      // CommonMark renders a code-span line ending as one space, including CRLF.
      for (let i = 0; i < mapped.length; i++) {
        if (mapped[i].text === '\r' && mapped[i + 1]?.text === '\n') {
          mapped[i] = { text: ' ', start: mapped[i].start, end: mapped[i + 1].end }; mapped.splice(i + 1, 1)
        } else if (/^[\r\n]$/.test(mapped[i].text)) mapped[i] = { ...mapped[i], text: ' ' }
      }
    }
    const result = { id: `source-${leaves.size}`, text: mapped.map((unit) => unit.text).join(''), units: mapped, fallback: failed }
    fallback ||= failed
    leaves.set(result.id, result)
    return result
  }
  function render(node: AstNode, header = false): ReviewNode[] {
    const children = () => (node.children ?? []).flatMap((child) => render(child, header))
    const wrap = (nodes: ReviewNode[]): ReviewNode[] => {
      const units: SourceUnit[] = []
      const collect = (child: ReviewNode) => { if (child.leaf) units.push(...child.leaf.units); else child.children.forEach(collect) }
      nodes.forEach(collect)
      if (units.length && node.position?.start.offset !== undefined && node.position.end.offset !== undefined) {
        wrappers.push({ start: offsets[node.position.start.offset], end: offsets[node.position.end.offset],
          visibleStart: units[0].start, visibleEnd: units.at(-1)!.end })
      }
      return nodes
    }
    const container = (tag: string, extra = {}, inline = false): ReviewNode[] => {
      const result = [{ tag, children: children(), ...extra }]
      return inline ? wrap(result) : result
    }
    switch (node.type) {
      case 'root': return children()
      case 'definition': return []
      case 'heading': return container(`h${node.depth}`)
      case 'paragraph': return container('p')
      case 'strong': return container('strong', {}, true)
      case 'emphasis': return container('em', {}, true)
      case 'delete': return container('del', {}, true)
      case 'blockquote': return container('blockquote')
      case 'list': return container(node.ordered ? 'ol' : 'ul', { start: node.start })
      case 'listItem': return container('li', { checked: node.checked })
      case 'link': case 'linkReference': return container('a', { href: node.url ?? definitions.get(node.identifier ?? '') }, true)
      case 'image': case 'imageReference': return [{ tag: 'image', children: [], placeholder: node.alt || 'Image' }]
      case 'table': return [{ tag: 'table', children: [
        { tag: 'thead', children: (node.children ?? []).slice(0, 1).flatMap((row) => render(row, true)) },
        { tag: 'tbody', children: (node.children ?? []).slice(1).flatMap((row) => render(row, false)) },
      ] }]
      case 'tableRow': return container('tr')
      case 'tableCell': return container(header ? 'th' : 'td')
      case 'break': return [{ tag: 'br', children: [] }]
      case 'thematicBreak': return [{ tag: 'hr', children: [] }]
      case 'code': return [{ tag: 'pre', children: [{ tag: 'code', children: [], leaf: leaf(node) }] }]
      case 'inlineCode': return wrap([{ tag: 'code', children: [], leaf: leaf(node) }])
      case 'text': return [{ tag: 'span', children: [], leaf: leaf(node) }]
      // Raw HTML is displayed literally, never executed.
      default: return [{ tag: 'span', children: [], leaf: leaf(node, true) }]
    }
  }
  const nodes = (ast.children ?? []).flatMap((node) => render(node).map((rendered) => ({ ...rendered,
    sourceStart: offsets[node.position?.start.offset ?? 0], sourceEnd: offsets[node.position?.end.offset ?? markdown.length],
  })))
  return { markdown, nodes, leaves, wrappers, fallback }
}

/** Include only invisible syntax of fully selected inline elements crossed by the range. */
export function completeInlineBoundaries(model: ReviewDocument, start: number, end: number): [number, number] {
  let from = start, to = end
  for (const wrapper of model.wrappers) {
    if (start <= wrapper.visibleStart && end >= wrapper.visibleEnd
      && (start < wrapper.visibleStart || end > wrapper.visibleEnd)) {
      from = Math.min(from, wrapper.start); to = Math.max(to, wrapper.end)
    }
  }
  return [from, to]
}

export interface LeafSegment { text: string; annotationIds: string[] }
/** Overlapping ranges share fragments rather than nesting or duplicating the source. */
export function highlightedLeaf(leaf: ReviewLeaf, annotations: ReviewAnnotation[]): LeafSegment[] {
  const segments: LeafSegment[] = []
  const ranges = annotations.filter((note) => note.start !== null && note.end !== null
    && note.start < (leaf.units.at(-1)?.end ?? 0) && note.end > (leaf.units[0]?.start ?? 0))
  for (const unit of leaf.units) {
    const annotationIds = ranges.filter((note) => note.start! < unit.end && note.end! > unit.start).map((note) => note.id)
    const previous = segments.at(-1)
    if (previous && previous.annotationIds.join(' ') === annotationIds.join(' ')) previous.text += unit.text
    else segments.push({ text: unit.text, annotationIds })
  }
  return segments
}

/** Surrogate pairs and multi-character decoded entities cannot be split. */
export function leafBoundary(leaf: ReviewLeaf, offset: number, edge: 'start' | 'end'): number | null {
  let position = 0
  for (const unit of leaf.units) {
    if (edge === 'start' && position === offset) return unit.start
    position += unit.text.length
    if (edge === 'end' && position === offset) return unit.end
    if (position > offset) return null
  }
  return null
}
