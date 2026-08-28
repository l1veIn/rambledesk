import { describe, expect, it } from 'vitest'

import { acceptCleanupResult, parseLabeledOutput } from './speechCleanupPolicy'

describe('parseLabeledOutput', () => {
  it('requires [n] labels even for a single block', () => {
    expect(parseLabeledOutput('tidied only', 1)).toBeNull()
    expect(parseLabeledOutput('[1] tidied only', 1)).toEqual(['tidied only'])
  })

  it('rejects missing, duplicate, or reordered labels', () => {
    expect(parseLabeledOutput('[1] one\n\n[3] three', 2)).toBeNull()
    expect(parseLabeledOutput('[1] one\n\n[1] again', 2)).toBeNull()
    expect(parseLabeledOutput('[2] two\n\n[1] one', 2)).toBeNull()
    expect(parseLabeledOutput('[1] one\n\n[2] two\n\n[3] three', 2)).toBeNull()
  })

  it('returns blocks in order when the contract holds', () => {
    expect(parseLabeledOutput('[1] 第一段\n\n[2] 第二段', 2)).toEqual(['第一段', '第二段'])
  })
})

describe('acceptCleanupResult', () => {
  it('rejects answers that grew far beyond the original utterance', () => {
    expect(acceptCleanupResult('按钮太小', '按钮太小，另外我建议重构整个导航并补测试。'.repeat(4))).toBeNull()
    expect(acceptCleanupResult('按钮太小', '按钮太小了。')).toBe('按钮太小了。')
  })
})
