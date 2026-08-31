<script lang="ts">
  import cryo1 from '../assets/rambelle-cryo/cryo-1.webp'
  import cryo2 from '../assets/rambelle-cryo/cryo-2.webp'
  import cryo3 from '../assets/rambelle-cryo/cryo-3.webp'
  import cryo4 from '../assets/rambelle-cryo/cryo-4.webp'
  import propBooklet from '../assets/rambelle-props/booklet.webp'
  import propCrystal from '../assets/rambelle-props/crystal.webp'
  import propEmblem from '../assets/rambelle-props/emblem.webp'
  import propGloves from '../assets/rambelle-props/gloves.webp'
  import propKeychain from '../assets/rambelle-props/keychain.webp'
  import propMug from '../assets/rambelle-props/mug.webp'
  import propPotato from '../assets/rambelle-props/potato.webp'
  import propSlate from '../assets/rambelle-props/slate.webp'
  import propStamp from '../assets/rambelle-props/stamp.webp'
  import sceneArchive from '../assets/rambelle-scenes/archive.webp'
  import sceneCryoHall from '../assets/rambelle-scenes/cryo-hall.webp'
  import sceneGate from '../assets/rambelle-scenes/gate.webp'
  import scenePod from '../assets/rambelle-scenes/pod.webp'
  import scenePotatoes from '../assets/rambelle-scenes/potatoes.webp'
  import sceneTea from '../assets/rambelle-scenes/tea.webp'
  import rambelleArchived from '../assets/rambelle-states/archived.webp'
  import { locale } from '$lib/preferences'
  import { rambelleProfile, type ProfileFigureId } from './rambelleProfile'

  $: lang = $locale === 'zh-CN' ? 'zh' : 'en'

  const cryoFrames = [cryo1, cryo2, cryo3, cryo4]

  const propSrc = {
    emblem: propEmblem,
    stamp: propStamp,
    booklet: propBooklet,
    slate: propSlate,
    potato: propPotato,
  }

  const figureSrc: Record<ProfileFigureId, string> = {
    gate: sceneGate,
    'cryo-hall': sceneCryoHall,
    pod: scenePod,
    archive: sceneArchive,
    potatoes: scenePotatoes,
    tea: sceneTea,
  }

  const personalEffects = [
    { src: propPotato, en: 'Emergency potato', zh: '应急土豆' },
    { src: propStamp, en: 'Holographic seal', zh: '全息印章' },
    { src: propBooklet, en: 'Mini archive', zh: '迷你档案册' },
    { src: propSlate, en: 'Data slate', zh: '档案板' },
    { src: propMug, en: 'Archived coffee', zh: '配方咖啡' },
    { src: propEmblem, en: 'Vault emblem', zh: '舱门徽章' },
    { src: propGloves, en: 'Dress gloves', zh: '浅色手套' },
    { src: propKeychain, en: 'Cryo keychain', zh: '休眠钥匙链' },
    { src: propCrystal, en: 'Last sunny day', zh: '晴天气象晶' },
  ]

  let viewport: HTMLElement | undefined
  let activeChapter = 0

  $: chapter = rambelleProfile.chapters[activeChapter] ?? rambelleProfile.chapters[0]

  function localize(text: { en: string; zh: string }) {
    return lang === 'zh' ? text.zh : text.en
  }

  function syncChapterFromScroll() {
    const el = viewport
    if (!el) return
    const marker = el.scrollTop + el.clientHeight * 0.28
    const nodes = el.querySelectorAll<HTMLElement>('[data-chapter]')
    let next = 0
    nodes.forEach((node, index) => {
      if (node.offsetTop <= marker + 12) next = index
    })
    if (next !== activeChapter) activeChapter = next
  }

  function goToChapter(index: number) {
    const target = viewport?.querySelector<HTMLElement>(`[data-chapter="${index}"]`)
    target?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

</script>

<div class="relative flex h-full min-h-0 flex-col gap-0 overflow-hidden bg-background">
    <div class="sr-only">
      <h1>{rambelleProfile.name.zh} · Rambelle</h1>
      <p>{localize(rambelleProfile.subtitle)}</p>
    </div>

    <div class="flex min-h-0 flex-1">
      <aside class="relative hidden w-[42%] shrink-0 overflow-hidden bg-[#7eb6de] min-[760px]:block">
        {#each cryoFrames as frame, index (index)}
          <img
            src={frame}
            alt=""
            draggable="false"
            class="absolute inset-0 size-full object-cover transition-opacity duration-700 ease-out"
            style={`opacity: ${activeChapter === index ? 1 : 0}`}
          />
        {/each}
        <div class="pointer-events-none absolute inset-x-0 top-0 bg-gradient-to-b from-black/35 to-transparent px-6 pb-16 pt-5">
          <p class="m-0 text-[10px] font-medium uppercase tracking-[0.22em] text-white/70">
            {localize(chapter.kicker)}
          </p>
        </div>
        <div class="pointer-events-none absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/75 via-black/30 to-transparent px-6 pb-6 pt-24 text-white">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="m-0 text-2xl font-semibold tracking-tight">{localize(rambelleProfile.name)}</h2>
            <span class="rounded-full border border-white/25 bg-white/15 px-2 py-0.5 text-[10px] text-white/90">
              {localize(rambelleProfile.subtitle)}
            </span>
          </div>
          <p class="m-0 mt-2 text-sm leading-6 text-white/80">
            {localize(chapter.title)} · “{localize(rambelleProfile.catchphrase)}”
          </p>
          <ol class="m-0 mt-4 flex list-none gap-2 p-0">
            {#each rambelleProfile.chapters as item, index (item.key)}
              <li
                class={`h-1 flex-1 rounded-full ${index === activeChapter ? 'bg-white' : 'bg-white/30'}`}
              ></li>
            {/each}
          </ol>
        </div>
      </aside>

      <div class="flex min-h-0 min-w-0 flex-1 flex-col bg-background">
        <header class="flex shrink-0 items-center gap-3 border-b py-3 pl-5 pr-14">
          <div class="relative size-12 shrink-0 overflow-hidden rounded-xl min-[760px]:hidden">
            {#each cryoFrames as frame, index (index)}
              <img
                src={frame}
                alt=""
                draggable="false"
                class="absolute inset-0 size-full object-cover transition-opacity duration-700"
                style={`opacity: ${activeChapter === index ? 1 : 0}`}
              />
            {/each}
          </div>
          <div class="min-w-0 flex-1">
            <p class="m-0 text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
              {localize(chapter.kicker)}
            </p>
            <h2 class="m-0 truncate text-sm font-semibold">{localize(chapter.title)}</h2>
          </div>
          <nav class="flex shrink-0 gap-1" aria-label={lang === 'zh' ? '章节' : 'Chapters'}>
            {#each rambelleProfile.chapters as item, index (item.key)}
              <button
                type="button"
                class={`grid size-7 place-items-center rounded-md text-[10px] font-medium ${
                  index === activeChapter
                    ? 'bg-info/15 text-info'
                    : 'text-muted-foreground hover:bg-muted'
                }`}
                aria-current={index === activeChapter ? 'true' : undefined}
                title={localize(item.title)}
                onclick={() => goToChapter(index)}
              >
                {index + 1}
              </button>
            {/each}
          </nav>
        </header>

        <div
          bind:this={viewport}
          onscroll={syncChapterFromScroll}
          class="min-h-0 flex-1 overflow-y-auto overscroll-contain px-6 py-5"
        >
          <div class="mx-auto flex max-w-xl flex-col gap-10">
            {#each rambelleProfile.chapters as item, index (item.key)}
              <article data-chapter={index} class="scroll-mt-3 min-h-[38rem]">
                <div class="flex items-start gap-3">
                  <img
                    src={propSrc[item.prop]}
                    alt=""
                    draggable="false"
                    class="size-11 shrink-0 object-contain"
                  />
                  <div class="min-w-0">
                    <p class="m-0 text-[10px] font-medium uppercase tracking-[0.18em] text-info">
                      {localize(item.kicker)}
                    </p>
                    <h3 class="m-0 mt-1 text-xl font-semibold tracking-tight">{localize(item.title)}</h3>
                    <p class="m-0 mt-1 text-sm text-muted-foreground">{localize(item.subtitle)}</p>
                  </div>
                </div>

                {#each item.paragraphs as paragraph}
                  <p class="m-0 mt-4 text-sm leading-7 text-foreground/85">
                    {localize(paragraph)}
                  </p>
                {/each}

                {#if item.quote}
                  <blockquote class="mt-5 border-l-2 border-info/40 bg-info/5 px-4 py-3">
                    <p class="m-0 text-sm leading-6 italic">“{localize(item.quote)}”</p>
                    {#if item.quoteBy}
                      <footer class="mt-2 text-[10px] uppercase tracking-wide text-muted-foreground">
                        {localize(item.quoteBy)}
                      </footer>
                    {/if}
                  </blockquote>
                {/if}

                {#if item.notes?.length}
                  <div class="mt-5 grid gap-3 sm:grid-cols-2">
                    {#each item.notes as note}
                      <div class="rounded-xl border bg-muted/20 px-3 py-3">
                        <strong class="block text-xs">{localize(note.title)}</strong>
                        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{localize(note.body)}</p>
                      </div>
                    {/each}
                  </div>
                {/if}

                {#if item.key === 'duty'}
                  <section class="mt-7">
                    <h4 class="m-0 text-sm font-semibold">{lang === 'zh' ? '性格弱点' : 'Character flaws'}</h4>
                    <div class="mt-3 grid gap-3">
                      {#each rambelleProfile.flaws as flaw}
                        <div class="rounded-xl border bg-muted/20 px-3 py-3">
                          <strong class="block text-xs">{localize(flaw.title)}</strong>
                          <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{localize(flaw.body)}</p>
                        </div>
                      {/each}
                    </div>
                  </section>

                  <section class="mt-7">
                    <h4 class="m-0 text-sm font-semibold">{lang === 'zh' ? '招牌技能' : 'Signature skills'}</h4>
                    <div class="mt-3 grid gap-3 sm:grid-cols-1">
                      {#each rambelleProfile.skills as skill}
                        <div class="rounded-xl border bg-muted/20 px-3 py-3">
                          <strong class="block text-xs">{localize(skill.title)}</strong>
                          <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{localize(skill.body)}</p>
                        </div>
                      {/each}
                    </div>
                  </section>

                  <section class="mt-7">
                    <h4 class="m-0 text-sm font-semibold">{lang === 'zh' ? '名台词' : 'Signature lines'}</h4>
                    <ul class="m-0 mt-3 grid gap-3 pl-0">
                      {#each rambelleProfile.quotes as line}
                        <li class="rounded-xl border-l-2 border-info/40 bg-info/5 px-4 py-3">
                          <p class="m-0 text-sm leading-6 italic">“{localize(line.text)}”</p>
                          <p class="m-0 mt-1.5 text-[10px] uppercase tracking-wide text-muted-foreground">
                            {localize(line.by)}
                          </p>
                        </li>
                      {/each}
                    </ul>
                  </section>
                {/if}

                {#each item.figures as figure (figure.id)}
                  <figure class="m-0 mt-5">
                    <img
                      src={figureSrc[figure.id]}
                      alt={localize(figure.caption)}
                      draggable="false"
                      class="w-full rounded-xl object-cover ring-1 ring-border"
                    />
                    <figcaption class="mt-2 text-[11px] leading-5 text-muted-foreground">
                      {localize(figure.caption)}
                    </figcaption>
                  </figure>
                {/each}

                {#if item.key === 'days'}
                  <section class="mt-7">
                    <h4 class="m-0 text-sm font-semibold">{lang === 'zh' ? '喜好' : 'Likes'}</h4>
                    <div class="mt-3 grid gap-4">
                      {#each rambelleProfile.likes as like}
                        <div>
                          <strong class="block text-xs">{localize(like.title)}</strong>
                          <ul class="m-0 mt-2 grid gap-2 pl-0">
                            {#each like.items as entry}
                              <li class="flex items-start gap-2 text-xs leading-5 text-muted-foreground">
                                <span class="mt-1.5 size-1 shrink-0 rounded-full bg-info/60"></span>
                                {localize(entry)}
                              </li>
                            {/each}
                          </ul>
                        </div>
                      {/each}
                    </div>
                  </section>

                  <section class="mt-7">
                    <h4 class="m-0 text-sm font-semibold">{lang === 'zh' ? '趣闻档案' : 'Archive curios'}</h4>
                    <ul class="m-0 mt-3 grid gap-2 pl-0">
                      {#each rambelleProfile.facts as fact, factIndex (factIndex)}
                        <li class="flex items-start gap-2.5 text-sm leading-6 text-muted-foreground">
                          <span class="mt-0.5 grid size-4 shrink-0 place-items-center rounded-full border border-info/30 bg-info/10 text-[9px] text-info">
                            {factIndex + 1}
                          </span>
                          {localize(fact)}
                        </li>
                      {/each}
                    </ul>
                  </section>

                  <section class="mt-7">
                    <h4 class="m-0 text-sm font-semibold">{lang === 'zh' ? '招牌元素和配色' : 'Motifs and palette'}</h4>
                    <ul class="m-0 mt-3 grid gap-2 pl-0">
                      {#each rambelleProfile.motifs as motif}
                        <li class="flex items-start gap-2 text-xs leading-5 text-muted-foreground">
                          <span class="mt-1.5 size-1 shrink-0 rounded-full bg-info/60"></span>
                          {localize(motif)}
                        </li>
                      {/each}
                    </ul>
                    <ul class="m-0 mt-4 grid gap-2 pl-0">
                      {#each rambelleProfile.palette as swatch}
                        <li class="flex items-center gap-3">
                          <span
                            class="size-8 shrink-0 rounded-full ring-1 ring-border"
                            style={`background: ${swatch.hex}`}
                          ></span>
                          <div class="min-w-0 flex-1">
                            <strong class="block text-xs">{localize(swatch.name)}</strong>
                            <span class="text-[10px] text-muted-foreground">{localize(swatch.note)}</span>
                          </div>
                          <span class="shrink-0 font-mono text-[10px] text-muted-foreground">{swatch.hex}</span>
                        </li>
                      {/each}
                    </ul>
                  </section>

                  <section class="mt-7">
                    <h4 class="m-0 text-sm font-semibold">
                      {lang === 'zh' ? '随身物件' : 'Personal effects'}
                    </h4>
                    <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
                      {lang === 'zh'
                        ? '档案室里每天会碰到的小东西。'
                        : 'The small things she files beside every day.'}
                    </p>
                    <ul class="m-0 mt-3 grid grid-cols-3 gap-3 pl-0">
                      {#each personalEffects as effect (effect.en)}
                        <li class="flex flex-col items-center gap-1.5 text-center">
                          <img src={effect.src} alt="" draggable="false" class="size-14 object-contain" />
                          <span class="text-[10px] leading-4 text-muted-foreground">
                            {lang === 'zh' ? effect.zh : effect.en}
                          </span>
                        </li>
                      {/each}
                    </ul>
                  </section>
                {/if}
              </article>
            {/each}
          </div>
        </div>

        <footer class="shrink-0 border-t bg-muted/20 px-5 py-3">
          <div class="flex items-center justify-between gap-4">
            <div class="flex min-w-0 items-center gap-3">
              <img
                src={rambelleArchived}
                alt=""
                draggable="false"
                class="size-10 shrink-0 object-contain"
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
    </div>

</div>
