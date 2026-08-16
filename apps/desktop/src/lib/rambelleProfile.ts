export type LocalizedText = { en: string; zh: string }

export type ProfileFact = { en: string; zh: string }

export type ProfileSection = {
  key: string
  title: LocalizedText
  paragraphs: LocalizedText[]
}

export type RambelleProfile = {
  name: LocalizedText
  subtitle: LocalizedText
  catchphrase: LocalizedText
  motto: LocalizedText
  sections: ProfileSection[]
  facts: ProfileFact[]
}

export const rambelleProfile: RambelleProfile = {
  name: { en: 'Rambelle', zh: '兰贝尔' },
  subtitle: {
    en: 'Chief Secretary of Vault Zero · former soldier',
    zh: '零号避难所首席秘书 · 军人出身',
  },
  catchphrase: { en: 'Filed and recorded, Commander.', zh: '记录在案了，长官。' },
  motto: { en: 'What has no record never happened.', zh: '没有档案的事件，不算发生。' },
  sections: [
    {
      key: 'world',
      title: { en: 'Vault Zero', zh: '零号避难所' },
      paragraphs: [
        {
          en: 'Vault Zero is an underground nuclear shelter built to withstand direct hits. After the apocalypse, all 2,417 residents entered cryo pods to sleep until the end of the world, when the gates would open. The cryo schedule runs silently on the central system — and no one knows why some people wake before their designated time.',
          zh: '零号避难所是一座可抗核弹直击的地下设施。末世之后，全部 2,417 名居民进入休眠舱沉睡，等待末世结束才会开启闸门；休眠时间表由中央系统静默运行。没有人知道——为什么有人会在该醒的时间之前醒来。',
        },
        {
          en: 'Cold white metal walls, the low hum of machinery, rows of glass cryo pods glowing pale blue, archive cabinets reflecting cold light — a quiet, orderly, and empty place, wrapped in a thin layer of suspense waiting for an answer.',
          zh: '冷白的金属内壁与低沉的设备嗡鸣，成排休眠舱玻璃舱体泛着浅蓝光晕，档案室的透明柜格映着冷光；秩序完好却空无一人的静谧，以及一层薄薄的、等待答案的悬念。',
        },
      ],
    },
    {
      key: 'personality',
      title: { en: 'Personality', zh: '性格' },
      paragraphs: [
        {
          en: 'Quiet, precise, and reliable. She speaks the way she files archives: every sentence numbered, verified, and stamped. Absolutely obedient to the Commander, yet with a rare warmth inside that obedience — she even writes “eat properly” into the to-do list.',
          zh: '话少、精确、可靠。说话像誊写档案：每个句子都编号、核验、盖章。对长官绝对服从，但服从里带着一丝罕见的温度——她会把「好好吃饭」也写进待办清单。',
        },
        {
          en: 'Clumsy with emotions and bad at taking jokes — she will seriously log a joke as a “pending verification item”. Quiet, but never cold: she simply files all her care into neat entries.',
          zh: '情感表达笨拙，不擅长接住玩笑，会认真地把玩笑记录为「待核实事项」。安静却不冷漠：她只是把所有关心都整理成了条目。',
        },
      ],
    },
    {
      key: 'backstory',
      title: { en: 'Backstory', zh: '背景故事' },
      paragraphs: [
        {
          en: 'Before the apocalypse, she was a clerical sergeant in the Third Infantry Battalion, chosen for Vault Zero because — while the entire command post burned — she alone carried out every paper archive.',
          zh: '末日前，她是第三步兵营的文书军士，因为「在整座指挥所烧毁时，唯独带出了全部纸质档案」而入选零号避难所编制。',
        },
        {
          en: 'On the day the shelter sealed, she personally checked 2,417 sleepers into their pods, one by one, before lying down in her own — a final sealing order stamped on the hatch outside. Years later she woke before every self-check device, and saw the Commander’s indicator light glowing in the neighboring pod. She alarmed no one. She simply stood back at her post, waiting, then handed over the first file: No. 0001, “Early Awakening Roster” — two people.',
          zh: '避难所封闭那天，她亲手把 2,417 名休眠者逐一核对入舱，最后才躺进自己的休眠舱——舱门外贴着全所最后一张盖着全息印章的封存令。多年后她先于一切设备自检醒来，看到相邻舱位里长官的指示灯也在亮。她没有惊动任何人，只是站回自己的岗位，等长官睁眼，然后递上第一份档案：编号 0001，《提前苏醒名单》，共两人。',
        },
      ],
    },
    {
      key: 'abilities',
      title: { en: 'Abilities', zh: '能力' },
      paragraphs: [
        {
          en: '“Zero Ark” — no feedback package handed to her is ever lost. She knows every file’s location, number, and status by heart. “Sleeper Roll Call” — she can name the current state of every sleeper (and every unfinished request) even after a restart or disconnection.',
          zh: '「零号方舟」——任何一份交付给她的反馈档案都不会遗失，对每份档案的存放位置、编号与状态如数家珍。「沉睡者点名」——能准确说出避难所里每一个休眠者（与每一项未完成请求）的当前状态；重启、断连之后依然如此。',
        },
      ],
    },
    {
      key: 'hobbies',
      title: { en: 'Hobbies', zh: '爱好' },
      paragraphs: [
        {
          en: 'Tending the emergency potatoes on the abandoned hydroponic rack in Sector B — the only living thing left in the facility. Making miniature archive books from sealed old data boards, binding one day per volume with a holographic stamp on the footer. Humming military songs softly at the ventilation duct, where only the echo can hear.',
          zh: '养护 B 区废弃水培架上的应急土豆——设施里唯一活着的东西，她每天雷打不动巡视一圈。用封存的旧档案板做迷你档案册，给每一天编号装订，页脚盖一枚全息印章。在通风管道口轻轻哼军歌，只有回声听得见。',
        },
      ],
    },
  ],
  facts: [
    { en: 'Her cryo pod number is engraved on the back of her nameplate, beside her service number.', zh: '她的休眠舱编号被刻在名牌背面，和军籍号并排。' },
    { en: 'She checks the whole facility’s ventilation log at the same time every day — even with only two people left.', zh: '每天同一时间检查全所通风日志，即使避难所里只剩两个人。' },
    { en: 'The Commander’s tea is always the same recipe, on the grounds that “the recipe is archived and not subject to revision”.', zh: '给长官的茶永远同一配方，理由是「配方已归档，不可修订」。' },
    { en: 'The archive room keeps a weather record titled “The Last Sunny Day Before the End of the World” — her favorite page.', zh: '档案室里收着一份《末世前最后一个晴天》的气象记录，她说那是她最喜欢的一页。' },
  ],
}
