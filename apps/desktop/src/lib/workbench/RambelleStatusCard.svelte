<script lang="ts">
  import { t } from '../i18n'
  import { locale } from '../preferences'

  export let portrait = ''
  export let feedbackDone = false
  export let rambleEngaged = false

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<section class="rambelle-note" aria-label={tr('Rambelle 状态')}>
  <div>
    <span>RAMBELLE · ONLINE</span>
    <p>
      {feedbackDone
        ? tr('记录在案了，长官。')
        : rambleEngaged
          ? tr('我正在整理这次 Ramble。')
          : tr('档案已就绪，随时可以继续。')}
    </p>
  </div>
  <img src={portrait} alt="Rambelle" />
</section>
