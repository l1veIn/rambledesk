<script lang="ts">
  import { Download, Play, ShieldCheck } from '@lucide/svelte'

  export let hero: {
    eyebrow: string
    headline: string
    subhead: string
    primaryCta: string
    secondaryCta: string
  }
  export let assets: {
    heroFallback: string
  }
  export let releaseUrl: string
</script>

<section class="hero-stage" aria-label={hero.eyebrow}>
  <img
    class="hero-plate"
    src={assets.heroFallback}
    alt=""
    loading="eager"
    decoding="async"
    fetchpriority="high"
    aria-hidden="true"
  />

  <div class="hero-content">
    <div class="hero-copy">
      <div class="system-badge">
        <ShieldCheck size={16} />
        <span>{hero.eyebrow}</span>
      </div>
      <h1>{hero.headline}</h1>
      <p>{hero.subhead}</p>
      <div class="hero-actions">
        <a class="primary-action" href={releaseUrl}>
          <Download size={18} />
          <span>{hero.primaryCta}</span>
        </a>
        <a class="secondary-action" href="#loop">
          <Play size={18} />
          <span>{hero.secondaryCta}</span>
        </a>
      </div>
    </div>
  </div>
</section>
