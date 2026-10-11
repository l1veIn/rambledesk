/** A bounded, literal change preview. Source text and replacement are never patched. */
export function suggestionDiff(before: string, after: string) {
  const original = [...before], replacement = [...after]
  let prefix = 0, suffix = 0
  while (prefix < Math.min(original.length, replacement.length) && original[prefix] === replacement[prefix]) prefix++
  while (suffix < original.length - prefix && suffix < replacement.length - prefix
    && original[original.length - suffix - 1] === replacement[replacement.length - suffix - 1]) suffix++
  return { prefix: original.slice(0, prefix).join(''), removed: original.slice(prefix, original.length - suffix).join(''),
    added: replacement.slice(prefix, replacement.length - suffix).join(''), suffix: suffix ? original.slice(-suffix).join('') : '' }
}
