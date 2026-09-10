export type DiffLine = Readonly<{
  kind: 'header' | 'hunk' | 'context' | 'addition' | 'deletion'
  text: string
  oldLine: number | null
  newLine: number | null
}>

/** Parse our generated unified diff, retaining body lines that resemble file headers. */
export function diffLines(diff: string | null): DiffLine[] {
  if (!diff) return []
  let oldLine = 0
  let newLine = 0
  let inHunk = false
  return diff.split('\n').map((text) => {
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(text)
    if (hunk) {
      oldLine = Number(hunk[1])
      newLine = Number(hunk[2])
      inHunk = true
      return { kind: 'hunk', text, oldLine: null, newLine: null }
    }
    if (!inHunk) return { kind: 'header', text, oldLine: null, newLine: null }
    if (text.startsWith('+')) return { kind: 'addition', text, oldLine: null, newLine: newLine++ }
    if (text.startsWith('-')) return { kind: 'deletion', text, oldLine: oldLine++, newLine: null }
    return { kind: 'context', text, oldLine: oldLine++, newLine: newLine++ }
  })
}
