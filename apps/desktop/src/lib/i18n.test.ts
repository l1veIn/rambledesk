import { describe, expect, it } from 'vitest'

import { t } from './i18n'

describe('update copy', () => {
  it('explains that launch checks show release notes', () => {
    expect(
      t(
        'zh-CN',
        'RambleDesk checks for updates after launch and shows what’s new when a version is available.',
      ),
    ).toContain('弹出更新说明')
    expect(t('zh-CN', "What's new")).toBe('更新内容')
    expect(t('zh-CN', 'Later')).toBe('稍后')
  })
})

describe('notification copy', () => {
  it('explains that unsigned Windows builds will not send system banners', () => {
    expect(
      t(
        'zh-CN',
        'Current unsigned Windows builds cannot show system banners. RambleDesk will not try to send them. Watch the inbox badge and use sound alerts instead.',
      ),
    ).toContain('不会尝试发送')
    expect(t('zh-CN', 'System notifications')).toBe('系统弹窗')
  })
})

describe('capability fallback copy', () => {
  it('translates one-time Desktop-only settings guidance', () => {
    expect(t('zh-CN', 'This settings section is available only in the desktop app.')).toBe(
      '此设置页面仅在桌面应用中可用。',
    )
    expect(t('zh-CN', 'Opening external links is available only in the desktop app.')).toBe(
      '打开外部链接仅在桌面应用中可用。',
    )
  })
})
