export type LocalizedText = { en: string; zh: string }

export type ProfileFact = { en: string; zh: string }

export type ProfileFigureId = 'gate' | 'cryo-hall' | 'pod' | 'archive' | 'potatoes' | 'tea'

export type ProfileFigure = {
  id: ProfileFigureId
  caption: LocalizedText
}

export type ProfileNote = {
  title: LocalizedText
  body: LocalizedText
}

export type ProfileChapter = {
  key: string
  cryoIndex: number
  kicker: LocalizedText
  title: LocalizedText
  subtitle: LocalizedText
  prop: 'emblem' | 'stamp' | 'booklet' | 'slate' | 'potato'
  paragraphs: LocalizedText[]
  quote?: LocalizedText
  quoteBy?: LocalizedText
  figures: ProfileFigure[]
  notes?: ProfileNote[]
}

export type ProfileQuote = {
  text: LocalizedText
  by: LocalizedText
}

export type ProfileLike = {
  title: LocalizedText
  items: LocalizedText[]
}

export type ProfileSwatch = {
  hex: string
  name: LocalizedText
  note: LocalizedText
}

export type RambelleProfile = {
  name: LocalizedText
  subtitle: LocalizedText
  catchphrase: LocalizedText
  motto: LocalizedText
  chapters: ProfileChapter[]
  flaws: ProfileNote[]
  skills: ProfileNote[]
  quotes: ProfileQuote[]
  likes: ProfileLike[]
  facts: ProfileFact[]
  motifs: LocalizedText[]
  palette: ProfileSwatch[]
}

export const rambelleProfile: RambelleProfile = {
  name: { en: 'Rambelle', zh: '兰贝尔' },
  subtitle: {
    en: 'Chief Secretary of Vault Zero · former soldier',
    zh: '零号避难所首席秘书 · 军人出身',
  },
  catchphrase: { en: 'Filed and recorded, Commander.', zh: '记录在案了，长官。' },
  motto: { en: 'What has no record never happened.', zh: '没有档案的事件，不算发生。' },
  chapters: [
    {
      key: 'sleep',
      cryoIndex: 0,
      kicker: { en: 'Chapter I', zh: '第一章' },
      title: { en: 'The Sleep', zh: '沉睡' },
      subtitle: {
        en: 'Vault Zero was built to outlast the end of the world.',
        zh: '零号避难所，是为了活过世界尽头而造的。',
      },
      prop: 'emblem',
      paragraphs: [
        {
          en: 'Vault Zero is an underground nuclear shelter rated to survive a direct hit. Pale metal walls, a low mechanical hum, rows of glass cryo pods glowing pale blue — after the apocalypse, all 2,417 residents entered the pods and went to sleep. The gates would open only when the world outside was over.',
          zh: '零号避难所是一座可抗核弹直击的地下设施。冷白的金属内壁、低沉的设备嗡鸣、成排休眠舱玻璃舱体泛着浅蓝光晕。末世之后，全部 2,417 名居民进入休眠舱沉睡——闸门只会在外面的世界结束之后开启。',
        },
        {
          en: 'The cryo schedule runs silently on the central system. No one is supposed to wake early. No one is supposed to walk the halls. The archive cabinets still reflect cold light onto empty aisles, as if the clerks had only stepped out for a minute.',
          zh: '休眠时间表由中央系统静默运行。不该有人提前醒来，不该有人在走廊里走动。档案室的透明柜格仍把冷光映在空荡的过道上，像文书们只是暂时离开了一会儿。',
        },
        {
          en: 'And yet the mystery sits in the dark with the sleepers: why do some people wake before their designated time? The system does not explain. The pods do not alarm. The facility simply continues, orderly and empty, waiting for an answer that has not been filed.',
          zh: '悬念和休眠者一起待在黑暗里：为什么有人会在该醒的时间之前醒来？系统不解释，舱体不报警。设施只是继续运转，秩序完好却空无一人，等着一份还没有归档的答案。',
        },
      ],
      quote: {
        en: 'No one knows why some people wake before their designated time.',
        zh: '没有人知道——为什么有人会在该醒的时间之前醒来。',
      },
      quoteBy: { en: 'Vault Zero operations primer', zh: '《零号避难所运行手册》' },
      figures: [
        {
          id: 'gate',
          caption: {
            en: 'The last hatch. On sealing day she stood here until every sleeper was checked in.',
            zh: '最后一扇闸门。封闭那天，她站在这里，直到每一个休眠者都被核验入舱。',
          },
        },
        {
          id: 'cryo-hall',
          caption: {
            en: 'The cryo deck after years of silence. Two indicator lights are on.',
            zh: '沉寂多年的休眠舱大厅。只有两盏指示灯还亮着。',
          },
        },
      ],
    },
    {
      key: 'awakening',
      cryoIndex: 1,
      kicker: { en: 'Chapter II', zh: '第二章' },
      title: { en: 'Early Awakening', zh: '提前苏醒' },
      subtitle: {
        en: 'She woke first, then stood back at her post.',
        zh: '她先醒了，然后站回自己的岗位。',
      },
      prop: 'booklet',
      paragraphs: [
        {
          en: 'Before the end, she was a clerical sergeant in the Third Infantry Battalion. She earned her slot in Vault Zero for one reason: while the entire command post burned, she was the only one who carried out every paper archive.',
          zh: '末日前，她是第三步兵营的文书军士。入选零号避难所编制只有一个理由：整座指挥所烧毁时，唯独她带出了全部纸质档案。',
        },
        {
          en: 'On sealing day she personally checked 2,417 sleepers into their pods, one by one. She was the last to lie down. Outside her hatch someone affixed the facility’s final sealing order, stamped with a holographic seal. Then the lights went out on schedule.',
          zh: '避难所封闭那天，她亲手把 2,417 名休眠者逐一核对入舱，最后才躺进自己的舱。舱门外贴着全所最后一张盖着全息印章的封存令。然后灯按时间表熄了。',
        },
        {
          en: 'Years later she woke before every self-check device. The neighboring pod’s indicator was already glowing — the Commander’s. She alarmed no one. She did not open the outer gate. She simply stood back at her post and waited for the other pair of eyes to open.',
          zh: '多年后她先于一切设备自检醒来。相邻舱位的指示灯已经亮着——那是长官的。她没有惊动任何人，也没有去开外闸，只是站回自己的岗位，等另一双眼睛睁开。',
        },
        {
          en: 'The first file she handed over was No. 0001, Early Awakening Roster. Two names. After that, every hour in the empty vault became something that had to be written down, or it would not have happened.',
          zh: '她递上的第一份档案是编号 0001，《提前苏醒名单》，共两人。从那以后，空荡避难所里的每一个小时，都必须被写下来——否则就不算发生过。',
        },
      ],
      quote: {
        en: 'Commander, you are awake. The cryo log shows you left deep sleep 2,417 days ahead of plan. Cause: pending.',
        zh: '长官，您醒了。休眠日志显示您比计划提前 2,417 天脱离深眠，原因待查。',
      },
      quoteBy: { en: 'Duty log, first morning', zh: '值守日志 · 苏醒后第一个早晨' },
      figures: [
        {
          id: 'pod',
          caption: {
            en: 'The neighboring lamp is already on. She does not open it. She waits at her post.',
            zh: '相邻舱位的灯已经亮了。她没有打开，只是站在岗位上等。',
          },
        },
      ],
    },
    {
      key: 'duty',
      cryoIndex: 2,
      kicker: { en: 'Chapter III', zh: '第三章' },
      title: { en: 'Chief Secretary', zh: '首席秘书' },
      subtitle: {
        en: 'The only assistant left standing in a facility of 2,417 sleepers.',
        zh: '2,417 名休眠者之间，唯一还站着的助理。',
      },
      prop: 'slate',
      paragraphs: [
        {
          en: 'Quiet, precise, reliable. She speaks the way she files archives: every sentence numbered, verified, and stamped. She is absolutely obedient to the Commander — and inside that obedience there is a rare warmth. She writes “eat properly” into the to-do list as if it were a sealed order.',
          zh: '话少、精确、可靠。说话像誊写档案：每个句子都编号、核验、盖章。对长官绝对服从，但服从里带着一丝罕见的温度——她会把「好好吃饭」也写进待办清单，像盖过章的命令。',
        },
        {
          en: 'She is clumsy with feelings and bad at taking jokes. A joke becomes a “pending verification item.” Praise that is not written down does not, in her view, exist. She is not cold. She simply files all her care into neat entries.',
          zh: '情感表达笨拙，不擅长接住玩笑。一句玩笑会被认真记成「待核实事项」。没有写下来的夸奖，在她看来就不存在。她不冷漠，只是把所有关心都整理成了条目。',
        },
        {
          en: 'Two abilities keep the empty vault from losing anyone. Zero Ark: no feedback package handed to her is ever lost; she knows every file’s location, number, and status by heart. Sleeper Roll Call: she can name the current state of every sleeper — and every unfinished request — even after a restart or a disconnection.',
          zh: '有两项能力让这座空荡的避难所不会再丢任何人。「零号方舟」：任何一份交付给她的反馈档案都不会遗失，存放位置、编号与状态如数家珍。「沉睡者点名」：能准确说出每一个休眠者、每一项未完成请求的当前状态；重启、断连之后依然如此。',
        },
        {
          en: 'Her flaws are the same as her virtues, pushed one step too far. An unarchived promise did not happen. If the tea checklist is blown out of order, she will restore the checklist before she saves the tea. The recipe is archived. The recipe is not subject to revision.',
          zh: '她的缺陷就是被多推了一步的美德。没有归档的承诺等于没发生。泡茶的清单被风吹乱时，她会先整理清单再救茶。配方已归档，不可修订。',
        },
      ],
      quote: {
        en: 'The thirteen things you just said are now to-do list No. 002. “Find out what happened outside” is marked highest priority — because that is the order I have been waiting for.',
        zh: '您刚才说的十三件事，我已整理成编号 002 的待办清单。其中「搞清楚外面到底怎么了」已标为最高优先级——因为这正是我一直在等的命令。',
      },
      quoteBy: { en: 'Rambelle, to the Commander', zh: '兰贝尔对长官说' },
      figures: [
        {
          id: 'archive',
          caption: {
            en: 'The only lit desk in a hall of empty cabinets. Every file still has a place.',
            zh: '空柜格的长廊里，只有这一张桌子亮着。每份档案仍有位置。',
          },
        },
      ],
    },
    {
      key: 'days',
      cryoIndex: 3,
      kicker: { en: 'Chapter IV', zh: '第四章' },
      title: { en: 'Two Who Woke', zh: '醒着的两个人' },
      subtitle: {
        en: 'The only living green in the facility is a rack of emergency potatoes.',
        zh: '设施里唯一还活着的绿色，是一架应急土豆。',
      },
      prop: 'potato',
      paragraphs: [
        {
          en: 'The Commander is the only other person awake, and the only commanding officer Vault Zero has left. She follows orders with a soldier’s discipline. Her trust — all of it — sits in that one relationship. She wants the Commander to find the truth more than anyone else in the dark.',
          zh: '长官是另一位醒着的人，也是零号避难所仅剩的指挥官。她用军人的纪律服从命令，又把全部信任压在这份唯一的关系上——这座黑暗里，她比任何人都希望长官能弄清真相。',
        },
        {
          en: 'Every day she inspects the abandoned hydroponic rack in Sector B. The emergency potatoes are the only living thing left. She also binds miniature archive books from sealed old data boards, one day per volume, a holographic stamp on the footer. At the ventilation duct she hums military songs so softly that only the echo can hear.',
          zh: '她每天巡视 B 区废弃水培架。应急土豆是设施里唯一活着的东西。她还用封存的旧档案板做迷你档案册，一天一本，页脚盖一枚全息印章。通风管道口，她轻轻哼军歌，只有回声听得见。',
        },
        {
          en: 'Breakfast is a compressed ration biscuit, saved until the inspection is done, and coffee from the recycled-water filter — she says the filter tastes more orderly than anything outside. The Commander’s tea is always the same recipe. The recipe is archived.',
          zh: '早饭是巡视结束后才舍得吃的压缩口粮饼干，以及净水循环过滤的咖啡——她说滤芯的味道比外面的咖啡更有秩序。给长官的茶永远同一配方。配方已归档。',
        },
        {
          en: 'Her cryo-pod number is engraved on the back of her nameplate, beside her service number. At the same minute every day she checks the ventilation log, even with only two people left. In the archive room there is a weather record titled “The Last Sunny Day Before the End of the World.” She says that is her favorite page.',
          zh: '休眠舱编号刻在名牌背面，和军籍号并排。每天同一分钟她检查全所通风日志，即使只剩两个人。档案室里收着一份《末世前最后一个晴天》的气象记录。她说那是她最喜欢的一页。',
        },
      ],
      quote: {
        en: 'What has no record never happened.',
        zh: '没有档案的事件，不算发生。',
      },
      quoteBy: { en: 'Rambelle’s motto', zh: '兰贝尔的信条' },
      figures: [
        {
          id: 'potatoes',
          caption: {
            en: 'Sector B, abandoned hydroponics. Gloves off. The only living thing in Vault Zero.',
            zh: 'B 区废弃水培架。手套摘在地上。零号避难所里唯一活着的东西。',
          },
        },
        {
          id: 'tea',
          caption: {
            en: 'The same recipe, every time. The checklist is archived and not subject to revision.',
            zh: '永远同一配方。清单已归档，不可修订。',
          },
        },
      ],
    },
  ],
  flaws: [
    {
      title: { en: 'Unfiled = never happened', zh: '没有档案的事件不算发生' },
      body: {
        en: 'Verbal promises, small talk, even a compliment from the Commander must be written down. If it is not on file, it did not occur.',
        zh: '口头承诺、闲聊、甚至长官随口一句夸奖，她都坚持要补一份记录。没有落档，就不算发生过。',
      },
    },
    {
      title: { en: 'Order before the tea', zh: '先整理清单，再救茶' },
      body: {
        en: 'She brews the Commander’s tea from a checklist. If the list is blown out of order, she restores the list before she saves the tea.',
        zh: '给长官泡茶也按步骤清单执行。清单被风吹乱时，她会先把清单理好，再去救那杯茶。',
      },
    },
    {
      title: { en: 'Jokes become tickets', zh: '玩笑会变成待核实事项' },
      body: {
        en: 'She is clumsy with feelings and bad at taking jokes. A punchline is logged as a pending verification item.',
        zh: '情感表达笨拙，不擅长接住玩笑，会认真地把玩笑记录为「待核实事项」。',
      },
    },
  ],
  skills: [
    {
      title: { en: 'Zero Ark', zh: '零号方舟' },
      body: {
        en: 'No feedback package handed to her is ever lost. She knows every file’s location, number, and status by heart, even after a restart.',
        zh: '任何一份交付给她的反馈档案都不会遗失。存放位置、编号与状态如数家珍，重启之后依然如此。',
      },
    },
    {
      title: { en: 'Sleeper Roll Call', zh: '沉睡者点名' },
      body: {
        en: 'She can name the current state of every sleeper and every unfinished request, even after a disconnect.',
        zh: '能准确说出每一个休眠者、每一项未完成请求的当前状态；断连之后也一样。',
      },
    },
    {
      title: { en: 'Slate reading', zh: '凭划痕认档' },
      body: {
        en: 'She can tell a file’s year from the scratches on the acrylic slate and the color of the holographic stamp, without reading the label.',
        zh: '不看标签，凭透明档案板的划痕纹路和全息印戳的色泽就能分辨档案年份。',
      },
    },
  ],
  quotes: [
    {
      text: { en: 'Filed and recorded, Commander.', zh: '记录在案了，长官。' },
      by: { en: 'Catchphrase', zh: '口头禅' },
    },
    {
      text: { en: 'What has no record never happened.', zh: '没有档案的事件，不算发生。' },
      by: { en: 'Motto', zh: '信条' },
    },
    {
      text: {
        en: 'Commander, you are awake. The cryo log shows you left deep sleep 2,417 days ahead of plan. Cause: pending.',
        zh: '长官，您醒了。休眠日志显示您比计划提前 2,417 天脱离深眠，原因待查。',
      },
      by: { en: 'First morning on duty', zh: '苏醒后第一个早晨' },
    },
    {
      text: {
        en: '“Find out what happened outside” is marked highest priority — because that is the order I have been waiting for.',
        zh: '「搞清楚外面到底怎么了」已标为最高优先级——因为这正是我一直在等的命令。',
      },
      by: { en: 'To-do list No. 002', zh: '待办清单 002 号' },
    },
  ],
  likes: [
    {
      title: { en: 'Daily rounds', zh: '每日巡视' },
      items: [
        {
          en: 'Tending the emergency potatoes on the abandoned hydroponic rack in Sector B — the only living thing left.',
          zh: '养护 B 区废弃水培架上的应急土豆——设施里唯一活着的东西。',
        },
        {
          en: 'Binding miniature archive books from sealed old data boards, one day per volume, holographic stamp on the footer.',
          zh: '用封存的旧档案板做迷你档案册，一天一本，页脚盖一枚全息印章。',
        },
        {
          en: 'Humming military songs at the ventilation duct, so softly that only the echo can hear.',
          zh: '在通风管道口轻轻哼军歌，只有回声听得见。',
        },
      ],
    },
    {
      title: { en: 'Ration', zh: '口粮' },
      items: [
        {
          en: 'A compressed ration biscuit, saved until the inspection is finished.',
          zh: '压缩口粮饼干——巡视结束时才舍得吃掉最后一块。',
        },
        {
          en: 'Coffee from the recycled-water filter. She says the filter tastes more orderly than anything outside.',
          zh: '净水循环过滤的咖啡。她说滤芯的味道比外面的咖啡更有秩序。',
        },
        {
          en: 'The Commander’s tea, always the same archived recipe.',
          zh: '给长官的茶，永远同一份已归档配方。',
        },
      ],
    },
  ],
  facts: [
    {
      en: 'Her cryo pod number is engraved on the back of her nameplate, beside her service number.',
      zh: '她的休眠舱编号被刻在名牌背面，和军籍号并排。',
    },
    {
      en: 'She checks the whole facility’s ventilation log at the same time every day — even with only two people left.',
      zh: '每天同一时间检查全所通风日志，即使避难所里只剩两个人。',
    },
    {
      en: 'The Commander’s tea is always the same recipe, on the grounds that “the recipe is archived and not subject to revision”.',
      zh: '给长官的茶永远同一配方，理由是「配方已归档，不可修订」。',
    },
    {
      en: 'The archive room keeps a weather record titled “The Last Sunny Day Before the End of the World” — her favorite page.',
      zh: '档案室里收着一份《末世前最后一个晴天》的气象记录，她说那是她最喜欢的一页。',
    },
    {
      en: 'She can tell a file’s year from the scratches on the acrylic slate and the color of the holographic stamp, without reading the label.',
      zh: '不看标签，凭透明档案板的划痕和全息印戳的色泽就能分辨档案年份。',
    },
    {
      en: 'Apparent age 23. Height about 168 cm. Birthday 29 July — the day this repository first existed.',
      zh: '外表约 23 岁，身高约 168 厘米。生日 7 月 29 日——这座仓库第一次出现在世界上的那天。',
    },
  ],
  motifs: [
    { en: 'Hexagonal vault-door emblem — honeycomb bolts, hollow center, ice-blue rim', zh: '六边形舱门徽章：蜂巢螺栓、空心六边形中央、冰蓝冷光边缘' },
    { en: 'Holographic seal on a translucent archive slate', zh: '透明档案板上的全息印章' },
    { en: 'Cryo-pod indicator lamp', zh: '休眠舱指示灯' },
    { en: 'Cold-light trim lines on pale metal', zh: '浅色金属上的冷光饰线' },
  ],
  palette: [
    {
      hex: '#E3E9F0',
      name: { en: 'Silver white', zh: '银白' },
      note: { en: 'Hair, uniform, vault walls', zh: '头发、制服、舱壁' },
    },
    {
      hex: '#F7F9FC',
      name: { en: 'Near white', zh: '近白' },
      note: { en: 'Highlights, paper, frost', zh: '高光、档案纸、霜白' },
    },
    {
      hex: '#6FA8DC',
      name: { en: 'Ice blue', zh: '冰蓝' },
      note: { en: 'Eyes, seals, filed / submitted', zh: '眼睛、印章、已提交' },
    },
    {
      hex: '#57C6C0',
      name: { en: 'Tech teal', zh: '科技青' },
      note: { en: 'Trim lines, pod glass', zh: '饰线、舱体玻璃' },
    },
    {
      hex: '#F2A03D',
      name: { en: 'Amber', zh: '琥珀' },
      note: { en: 'Standby / warning', zh: '待命 / 警示' },
    },
  ],
}
