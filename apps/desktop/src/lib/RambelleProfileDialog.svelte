<script lang="ts">
  import { Archive, BookOpen, Heart, ShieldCheck, Sprout, X } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import cryo1 from '../assets/rambelle-cryo/cryo-1.png'
  import cryo2 from '../assets/rambelle-cryo/cryo-2.png'
  import cryo3 from '../assets/rambelle-cryo/cryo-3.png'
  import cryo4 from '../assets/rambelle-cryo/cryo-4.png'
  import rambelleArchived from '../assets/rambelle-states/archived.png'
  import * as Dialog from '$lib/components/ui/dialog'
  import { locale } from '$lib/preferences'
  import { rambelleProfile, type ProfileSection } from './rambelleProfile'

  export let open = false
  export let onClose: () => void = () => {}

  $: lang = $locale === 'zh-CN' ? 'zh' : 'en'

  const sectionIcons: Record<string, typeof Archive> = {
    world: Archive,
    personality: Heart,
    backstory: BookOpen,
    abilities: ShieldCheck,
    hobbies: Sprout,
  }

  const cryoFrames = [cryo1, cryo2, cryo3, cryo4]
  let viewport: HTMLElement | undefined
  let scrollProgress = 0

  function localize(text: { en: string; zh: string }) {
    return lang === 'zh' ? text.zh : text.en
  }

  function onViewportScroll() {
    const el = viewport
    if (!el) return
    const max = el.scrollHeight - el.clientHeight
    scrollProgress = max > 0 ? el.scrollTop / max : 0
  }

  /** Crossfade opacity for one cryo frame across the scroll progress. */
  function frameOpacity(index: number) {
    const position = scrollProgress * (cryoFrames.length - 1)
    return Math.max(0, Math.min(1, 1 - Math.abs(position - index)))
  }

  // Scroll-reveal: add [data-in] once a section enters the viewport.
  onMount(() => {
    const root = viewport
    if (!root) return
    const entries = Array.from(root.querySelectorAll<HTMLElement>('[data-reveal]'))
    if (typeof IntersectionObserver === 'undefined') {
      entries.forEach((el) => el.setAttribute('data-in', ''))
      return
    }
    const observer = new IntersectionObserver(
      (changes) => {
        for (const change of changes) {
          if (change.isIntersecting) {
            change.target.setAttribute('data-in', '')
            observer.unobserve(change.target)
          }
        }
      },
      { root, rootMargin: '0px 0px -8% 0px', threshold: 0.06 },
    )
    entries.forEach((el) => observer.observe(el))
    return () => observer.disconnect()
  })
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    class="h-[min(720px,calc(100vh-4rem))] w-[min(880px,calc(100vw-3rem))] max-w-none gap-0 overflow-hidden p-0 sm:max-w-none"
  >
    <Dialog.Header class="sr-only">
      <Dialog.Title>{rambelleProfile.name.zh} · Rambelle</Dialog.Title>
      <Dialog.Description>
        {localize(rambelleProfile.subtitle)}
      </Dialog.Description>
    </Dialog.Header>

    <div class="relative flex h-full flex-col">
      <!-- Header -->
      <header class="relative shrink-0 overflow-hidden border-b bg-gradient-to-br from-primary/10 via-background to-info/10 px-7 py-6">
        <div class="pointer-events-none absolute -right-8 -top-10 size-44 rounded-full bg-info/15 blur-3xl"></div>
        <div class="pointer-events-none absolute -bottom-12 left-1/3 size-36 rounded-full bg-primary/10 blur-3xl"></div>

        <div class="relative flex items-center gap-5">
          <div class="rambelle-float relative size-24 shrink-0">
            {#each cryoFrames as frame, index (index)}
              <img
                src={frame}
                alt=""
                draggable="false"
                class="absolute inset-0 size-full rounded-2xl object-cover drop-shadow-[0_14px_28px_rgba(59,130,246,0.28)]"
                style={`opacity: ${frameOpacity(index)}`}
              />
            {/each}
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <h2 class="m-0 text-2xl font-semibold tracking-tight">{localize(rambelleProfile.name)}</h2>
              <span class="rounded-full border border-info/30 bg-info/10 px-2 py-0.5 text-[10px] text-info">
                {localize(rambelleProfile.subtitle)}
              </span>
            </div>
            <p class="m-0 mt-1.5 text-sm text-muted-foreground">
              “{localize(rambelleProfile.catchphrase)}”
            </p>
          </div>
          <Dialog.Close
            onclick={onClose}
            class="grid size-8 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground"
            aria-label="Close"
          >
            <X class="size-4" />
          </Dialog.Close>
        </div>
      </header>

      <!-- Scrollable story -->
      <div
        bind:this={viewport}
        onscroll={onViewportScroll}
        class="min-h-0 flex-1 overflow-y-auto px-7 py-6"
      >
        <div class="mx-auto max-w-2xl space-y-7">
          {#each rambelleProfile.sections as section (section.key)}
            <section data-reveal class="profile-section">
              <div class="flex items-center gap-2">
                {#if sectionIcons[section.key]}
                  <svelte:component
                    this={sectionIcons[section.key]}
                    class="size-4 text-info"
                  />
                {/if}
                <h3 class="m-0 text-sm font-semibold tracking-tight">
                  {localize(section.title)}
                </h3>
              </div>
              {#each section.paragraphs as paragraph}
                <p class="m-0 mt-2 text-sm leading-6 text-muted-foreground">
                  {localize(paragraph)}
                </p>
              {/each}
            </section>
          {/each}

          <section data-reveal class="profile-section">
            <h3 class="m-0 text-sm font-semibold tracking-tight">
              {lang === 'zh' ? '小趣事' : 'Fun facts'}
            </h3>
            <ul class="m-0 mt-3 grid gap-2 pl-0">
              {#each rambelleProfile.facts as fact, index (index)}
                <li class="flex items-start gap-2.5 text-sm leading-6 text-muted-foreground">
                  <span class="mt-0.5 grid size-4 shrink-0 place-items-center rounded-full border border-info/30 bg-info/10 text-[9px] text-info">
                    {index + 1}
                  </span>
                  {localize(fact)}
                </li>
              {/each}
            </ul>
          </section>
        </div>
      </div>

      <!-- Footer -->
      <footer class="relative shrink-0 border-t bg-muted/20 px-7 py-4">
        <div class="flex items-center justify-between gap-4">
          <div class="flex min-w-0 items-center gap-3">
            <img
              src={rambelleArchived}
              alt=""
              draggable="false"
              class="size-9 shrink-0 rounded-lg object-contain"
            />
            <p class="m-0 truncate text-xs italic text-muted-foreground">
              “{localize(rambelleProfile.motto)}”
            </p>
          </div>
          <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
            No. 0001 · {rambelleProfile.name.zh}
          </span>
        </div>
      </footer>
    </div>
  </Dialog.Content>
</Dialog.Root>

<style>
  .rambelle-float {
    animation: rambelle-float 5s ease-in-out infinite;
  }

  @keyframes rambelle-float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-5px);
    }
  }

  .profile-section {
    opacity: 0;
    transform: translateY(14px);
    transition:
      opacity 0.55s cubic-bezier(0.22, 1, 0.36, 1),
      transform 0.55s cubic-bezier(0.22, 1, 0.36, 1);
  }

  /* data-in is toggled imperatively by IntersectionObserver. */
  :global(.profile-section[data-in]) {
    opacity: 1;
    transform: translateY(0);
  }
</style>
