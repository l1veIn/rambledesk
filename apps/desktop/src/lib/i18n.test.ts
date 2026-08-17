import { describe, expect, it } from 'vitest'

import { t } from './i18n'

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
