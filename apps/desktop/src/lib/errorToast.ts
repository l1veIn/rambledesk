import { get } from 'svelte/store'

import { toast } from '$lib/components/ui/sonner'
import { t } from '$lib/i18n'
import { locale } from '$lib/preferences'

/**
 * Error toast with a copy button: error objects are often impossible to read
 * (or to paste) from toast text, so offer one-click copy of the description.
 */
export function errorToastWithCopy(
  title: string,
  description: string,
): void {
  const text = description.trim() || title
  toast.error(title, {
    description,
    action: {
      label: t(get(locale), 'Copy error'),
      onClick: () => {
        void navigator.clipboard.writeText(text).catch(() => undefined)
      },
    },
  })
}
