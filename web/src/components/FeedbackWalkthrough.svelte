<script lang="ts">
  import { feedbackExamples } from '../content/feedback-example'
  let { lang = 'en' }: { lang: string } = $props()

  const instanceId = $props.id()
  let tabButtons = $state<HTMLButtonElement[]>([])
  let activeStep = $state(1)

  const content = {
    'zh-CN': {
      ...feedbackExamples.zh,
      example: '交互示例 · 非产品录屏',
      stepsLabel: '查看反馈流程的三个步骤',
      steps: ['体验单', '说话与截图', '提交反馈'],
      caseLabel: '同一条体验请求',
      requestLabel: '体验请求',
      requestNote: '从具体的体验开始，把判断留给你。',
      voiceLabel: '口述示例',
      voiceTitle: '先说出你看到的。',
      voiceNote: '说话、截图，也可以直接编辑文字。',
      illustrationLabel: '截图示意',
      illustrationAlt: '设置侧栏示意：底部内容受遮挡，并存在横向滚动条。',
      settings: '设置',
      settingsItems: ['账户', '通知', '外观', '通用', '关于'],
      issue: '底部内容未完整显示',
      saved: '示例：反馈已保存',
      packageTitle: '原话和附件，一起留存。',
      packageBody: '提交后，原始反馈、附件和可选整理稿保存为不可变反馈包。',
      original: '原始文字',
      attachment: '附件示意',
      attachmentName: '设置页侧栏',
      preserved: '原话完整保留',
      originalFile: '原始文字与提交正文',
      attachmentFile: '随反馈保存的附件',
      packageLabel: '反馈包示意',
      immutable: '提交时的内容快照，保存后不可变。',
      deliveryTitle: '送达状态另行确认',
      deliveryBody: '保存反馈后，仍需查看实际送达状态。本例未连接 Agent，不展示送达或继续运行的结果。',
      summaryLabel: '这次示例反馈的概要',
      summaryNote: '原始反馈与附件，保存为不可变反馈包。',
      detailToggle: '查看这次反馈',
      optionalTitle: '需要时，再整理',
      optionalState: '可选 · 默认关闭',
      optionalBody: '本例保留原稿，未生成整理稿。启用整理后，相应内容会发给你配置的模型服务；整理稿单独保存，原始反馈仍会保留。',
    },
    en: {
      ...feedbackExamples.en,
      example: 'Interactive example · not a recording',
      stepsLabel: 'Explore the three feedback steps',
      steps: ['Experience brief', 'Speak & capture', 'Submit feedback'],
      caseLabel: 'One experience request',
      requestLabel: 'Experience request',
      requestNote: 'Start with something concrete. The judgment is yours.',
      voiceLabel: 'Spoken example',
      voiceTitle: 'Say what you see.',
      voiceNote: 'Speak, capture a screenshot, or edit the text directly.',
      illustrationLabel: 'Screenshot illustration',
      illustrationAlt: 'Settings sidebar illustration: the bottom is obscured and a horizontal scrollbar remains.',
      settings: 'Settings',
      settingsItems: ['Account', 'Notifications', 'Appearance', 'General', 'About'],
      issue: 'Bottom content is obscured',
      saved: 'Example: feedback saved',
      packageTitle: 'Your words and attachments, kept together.',
      packageBody: 'On submission, the original feedback, attachments, and optional refined version are saved as an immutable feedback package.',
      original: 'Original words',
      attachment: 'Illustrative attachment',
      attachmentName: 'Settings sidebar',
      preserved: 'Original words preserved',
      originalFile: 'Original words and submitted text',
      attachmentFile: 'Attachments saved with the feedback',
      packageLabel: 'Feedback package illustration',
      immutable: 'A snapshot of what you submitted. Immutable once saved.',
      deliveryTitle: 'Delivery is a separate status',
      deliveryBody: 'After saving, check the actual delivery status. This example is not connected to an Agent and shows no delivery or continuation result.',
      summaryLabel: 'Summary of this example feedback',
      summaryNote: 'Original feedback and attachments, saved as an immutable package.',
      detailToggle: 'View this feedback',
      optionalTitle: 'Refine only when you need to',
      optionalState: 'Optional · off by default',
      optionalBody: 'This example keeps the original; no refined version was generated. When enabled, refinement sends the relevant content to your configured model service. The refined version is saved separately, and the original feedback is preserved.',
    },
  }

  const copy = $derived(lang === 'zh-CN' ? content['zh-CN'] : content.en)

  function handleTabKey(event: KeyboardEvent, index: number) {
    let next: number
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') next = (index + 1) % 3
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') next = (index + 2) % 3
    else if (event.key === 'Home') next = 0
    else if (event.key === 'End') next = 2
    else return

    event.preventDefault()
    activeStep = next
    tabButtons[next]?.focus()
  }
</script>

{#snippet settingsIllustration(compact: boolean)}
  <div class="settings-illustration" class:compact role="img" aria-label={copy.illustrationAlt}>
    <div class="settings-paper">
      <div class="settings-heading">
        <svg width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="m9 3-1 3-3 1v4l2 1-2 1v4l3 1 1 3h6l1-3 3-1v-4l-2-1 2-1V7l-3-1-1-3Z" /><circle cx="12" cy="12" r="3" /></svg>
        <span>{copy.settings}</span>
      </div>
      <div class="settings-list" aria-hidden="true">
        {#each copy.settingsItems as item, index}
          <span class:obscured={index === 4}><i></i>{item}</span>
        {/each}
      </div>
      <div class="illustrated-scrollbar" aria-hidden="true"><span></span></div>
    </div>
    {#if !compact}<span class="issue-note">{copy.issue}</span>{/if}
  </div>
{/snippet}

<div class="feedback-walkthrough">
  <div class="step-tabs" role="tablist" aria-label={copy.stepsLabel}>
    {#each copy.steps as step, index}
      <button
        bind:this={tabButtons[index]}
        id={`${instanceId}-tab-${index}`}
        type="button"
        role="tab"
        aria-selected={activeStep === index}
        aria-controls={`${instanceId}-panel-${index}`}
        tabindex={activeStep === index ? 0 : -1}
        onclick={() => { activeStep = index }}
        onkeydown={(event) => handleTabKey(event, index)}
      >
        <span class="step-number" aria-hidden="true">{index + 1}</span><span>{step}</span>
      </button>
    {/each}
  </div>

  <div class="workbench">
    <div class="workbench-titlebar">
      <span class="workbench-brand">
        <svg width="21" height="23" viewBox="0 0 24 26" aria-hidden="true"><path d="m12 2 10 6v10l-10 6L2 18V8Z" /><path d="m12 7 5 3v6l-5 3-5-3v-6Z" /></svg>
        RambleDesk
      </span>
      <span class="example-label">{copy.example}</span>
    </div>

    <div class="workbench-body">
      <aside class="case-sidebar" aria-label={copy.caseLabel}>
        <span class="case-index">RD / 001</span>
        <span class="sidebar-label">{copy.caseLabel}</span>
        <p>{copy.caseTitle}</p>
        <div class="sidebar-divider"></div>
        <span class="sidebar-current"><span aria-hidden="true">0{activeStep + 1}</span>{copy.steps[activeStep]}</span>
      </aside>

      <div class="workbench-content">
        <div id={`${instanceId}-panel-0`} class="step-panel brief-panel" role="tabpanel" aria-labelledby={`${instanceId}-tab-0`} tabindex="0" hidden={activeStep !== 0}>
          <span class="request-label"><i aria-hidden="true"></i>{copy.requestLabel}</span>
          <h3>{copy.caseTitle}</h3>
          <dl class="brief-fields">
            <div><dt><span aria-hidden="true">01</span>{copy.happened}</dt><dd>{copy.happenedBody}</dd></div>
            <div><dt><span aria-hidden="true">02</span>{copy.experience}</dt><dd>{copy.experienceBody}</dd></div>
          </dl>
          <p class="panel-note">{copy.requestNote}</p>
        </div>

        <div id={`${instanceId}-panel-1`} class="step-panel voice-panel" role="tabpanel" aria-labelledby={`${instanceId}-tab-1`} tabindex="0" hidden={activeStep !== 1}>
          <div class="spoken-copy">
            <span class="voice-label">
              <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><rect x="9" y="2" width="6" height="13" rx="3" /><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3m-3 0h6" /></svg>
              {copy.voiceLabel}
            </span>
            <h3>{copy.voiceTitle}</h3>
            <blockquote>{copy.quote}</blockquote>
            <p class="panel-note">{copy.voiceNote}</p>
          </div>
          <figure class="capture-figure">
            {@render settingsIllustration(false)}
            <figcaption>{copy.illustrationLabel}</figcaption>
          </figure>
        </div>

        <div id={`${instanceId}-panel-2`} class="step-panel package-panel" role="tabpanel" aria-labelledby={`${instanceId}-tab-2`} tabindex="0" hidden={activeStep !== 2}>
          <span class="saved-label"><svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12 4 4L19 6" /></svg>{copy.saved}</span>
          <h3>{copy.packageTitle}</h3>
          <p class="package-intro">{copy.packageBody}</p>
          <div class="package-files" aria-label={copy.packageLabel}>
            <div><span class="file-icon" aria-hidden="true">Aa</span><span><strong>{copy.original}</strong><small>{copy.originalFile}</small></span></div>
            <div><span class="file-icon" aria-hidden="true"><svg width="22" height="22" viewBox="0 0 24 24"><rect x="3" y="3" width="18" height="18" rx="3" /><circle cx="8" cy="8" r="1.5" /><path d="m3 18 6-6 4 4 3-3 5 5" /></svg></span><span><strong>{copy.attachment}</strong><small>{copy.attachmentFile}</small></span></div>
          </div>
          <p class="panel-note">{copy.immutable}</p>
          <aside class="delivery-note"><strong>{copy.deliveryTitle}</strong><p>{copy.deliveryBody}</p></aside>
        </div>
      </div>
    </div>
  </div>

  <div class="feedback-archive">
    <div class="result-summary" role="group" aria-label={copy.summaryLabel}>
      <blockquote class="summary-quote">{copy.quote}</blockquote>
      <figure class="summary-thumbnail">
        {@render settingsIllustration(true)}
        <figcaption>{copy.illustrationLabel}</figcaption>
      </figure>
      <div class="summary-saved">
        <span class="save-mark" aria-hidden="true"><svg width="21" height="21" viewBox="0 0 24 24"><path d="m5 12 4 4L19 6" /></svg></span>
        <strong>{copy.saved}</strong>
        <p>{copy.summaryNote}</p>
      </div>
    </div>

    <details class="feedback-details">
      <summary><span>{copy.detailToggle}</span><svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg></summary>
      <div class="detail-content">
        <div class="detail-original"><span class="detail-label">01 / {copy.original}</span><blockquote>{copy.quote}</blockquote><span class="preserved-note">{copy.preserved}</span></div>
        <figure class="detail-attachment"><span class="detail-label">02 / {copy.attachment}</span>{@render settingsIllustration(false)}<figcaption>{copy.attachmentName} · {copy.illustrationLabel}</figcaption></figure>
        <div class="detail-optional"><div class="optional-heading"><h4>{copy.optionalTitle}</h4><span>{copy.optionalState}</span></div><p>{copy.optionalBody}</p></div>
        <aside class="detail-delivery"><strong>{copy.deliveryTitle}</strong><p>{copy.deliveryBody}</p></aside>
      </div>
    </details>
  </div>
</div>

<style>
  .feedback-walkthrough {
    --demo-ink: #182b3b;
    --demo-muted: #526575;
    --demo-line: #d9e2ea;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    width: 100%;
    min-width: 0;
    color: var(--demo-ink);
  }
  .feedback-walkthrough :is(p, h3, h4, blockquote, figure, dl, dd) {
    margin: 0;
  }
  .feedback-walkthrough :is(svg, i) {
    flex: 0 0 auto;
  }
  .feedback-walkthrough svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .workbench {
    grid-row: 1;
    border: 1px solid #a8bfce;
    border-radius: 13px;
    padding: 9px;
    background: #dce7ef;
    box-shadow:
      0 26px 65px #07131f33,
      inset 0 1px 0 #ffffffcc;
  }
  .workbench-titlebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 19px 19px;
  }
  .workbench-brand {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-size: 14px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  .workbench-brand svg {
    color: #467ba5;
  }
  .example-label {
    font-size: 12px;
    line-height: 1.5;
    color: #415c71;
    text-align: right;
  }
  .workbench-body {
    display: grid;
    grid-template-columns: minmax(150px, 0.22fr) minmax(0, 1fr);
    border: 1px solid #cddae4;
    border-radius: 7px;
    background: #f9fbfd;
  }
  .case-sidebar {
    min-width: 0;
    padding: 34px 23px;
    border-right: 1px solid var(--demo-line);
    background: linear-gradient(155deg, #eaf1f6, #f3f7fa);
    border-radius: 6px 0 0 6px;
  }
  .case-index {
    display: block;
    color: #567991;
    font:
      11px ui-monospace,
      monospace;
    letter-spacing: 0.1em;
  }
  .sidebar-label {
    display: block;
    margin-top: 45px;
    color: var(--demo-muted);
    font-size: 11px;
    line-height: 1.6;
  }
  .case-sidebar p {
    margin-top: 9px;
    font-size: 15px;
    font-weight: 650;
    line-height: 1.65;
  }
  .sidebar-divider {
    margin: 25px 0;
    height: 1px;
    background: #cfdee8;
  }
  .sidebar-current {
    display: grid;
    gap: 10px;
    color: #315f82;
    font-size: 12px;
    line-height: 1.6;
  }
  .sidebar-current > span {
    font:
      27px ui-monospace,
      monospace;
    color: #4e7d9e;
  }
  .workbench-content {
    min-width: 0;
  }
  .step-panel {
    padding: clamp(26px, 4.5vw, 58px);
    outline-offset: -5px;
  }
  .step-panel[hidden] {
    display: none;
  }
  .step-panel h3 {
    max-width: 650px;
    font-size: clamp(23px, 2.55vw, 32px);
    line-height: 1.35;
    letter-spacing: -0.035em;
    font-weight: 650;
    text-wrap: balance;
  }
  .request-label,
  .voice-label,
  .saved-label {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 17px;
    font-size: 12px;
    line-height: 1.5;
    font-weight: 600;
  }
  .request-label {
    color: #835216;
  }
  .request-label i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #d78b2c;
  }
  .brief-fields {
    margin-top: 27px;
    border-top: 1px solid var(--demo-line);
  }
  .brief-fields > div {
    padding-top: 25px;
  }
  .brief-fields > div + div {
    padding-top: 29px;
  }
  .brief-fields dt {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 15px;
    line-height: 1.6;
    font-weight: 650;
  }
  .brief-fields dt > span {
    color: #557b97;
    font:
      11px ui-monospace,
      monospace;
  }
  .brief-fields dd {
    margin-top: 9px;
    max-width: 620px;
    font-size: clamp(16px, 1.65vw, 19px);
    line-height: 1.85;
    color: #3d5365;
  }
  .step-panel .panel-note {
    margin-top: 29px;
    font-size: 12px;
    line-height: 1.8;
    color: var(--demo-muted);
  }
  .voice-panel {
    display: grid;
    grid-template-columns: minmax(0, 1.3fr) minmax(190px, 0.8fr);
    align-items: center;
    gap: 36px;
  }
  .voice-label {
    color: #396e8e;
  }
  .spoken-copy blockquote {
    margin-top: 25px;
    padding-left: 19px;
    border-left: 2px solid #80b5d3;
    font-size: clamp(18px, 2vw, 23px);
    line-height: 1.85;
    letter-spacing: -0.02em;
  }
  .capture-figure {
    min-width: 0;
  }
  .capture-figure figcaption,
  .detail-attachment figcaption {
    margin-top: 11px;
    font-size: 11px;
    line-height: 1.65;
    color: var(--demo-muted);
  }
  .settings-illustration {
    padding: 19px;
    border: 1px solid #c2d1de;
    border-radius: 6px;
    background: linear-gradient(145deg, #a4b6c6, #c9d4df);
  }
  .settings-paper {
    border: 1px solid #dce5ed;
    border-radius: 5px;
    background: #f7fafc;
    box-shadow: 0 8px 20px #243f5326;
  }
  .settings-heading {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 16px 13px 12px;
    font-size: 13px;
    line-height: 1.4;
    font-weight: 650;
  }
  .settings-list {
    padding: 0 9px;
  }
  .settings-list > span {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 5px;
    border-top: 1px solid #e3eaf0;
    font-size: 11px;
    line-height: 1.3;
    color: #506475;
  }
  .settings-list i {
    width: 5px;
    height: 5px;
    border: 1px solid #93a9bb;
    border-radius: 1px;
  }
  .settings-list > span.obscured {
    padding-bottom: 3px;
    color: #687c8c;
    mask-image: linear-gradient(#000 35%, transparent 95%);
  }
  .illustrated-scrollbar {
    padding: 3px;
    border-top: 1px solid #d8b779;
    background: #f4e9d4;
  }
  .illustrated-scrollbar > span {
    display: block;
    width: 65%;
    height: 4px;
    border-radius: 3px;
    background: #b39059;
  }
  .issue-note {
    display: block;
    margin-top: 13px;
    font-size: 10px;
    font-weight: 600;
    line-height: 1.5;
    color: #294459;
  }
  .saved-label {
    color: #26736e;
  }
  .package-intro {
    margin-top: 17px !important;
    max-width: 610px;
    color: var(--demo-muted);
    font-size: 16px;
    line-height: 1.8;
  }
  .package-files {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin-top: 26px;
  }
  .package-files > div {
    display: flex;
    align-items: center;
    gap: 13px;
    padding: 17px;
    border: 1px solid var(--demo-line);
    border-radius: 5px;
    background: white;
  }
  .file-icon {
    display: grid;
    flex: 0 0 35px;
    place-items: center;
    height: 40px;
    border: 1px solid #c9dce8;
    border-radius: 4px;
    color: #467590;
    font-size: 14px;
  }
  .package-files strong {
    display: block;
    font-size: 13px;
    line-height: 1.5;
  }
  .package-files small {
    display: block;
    margin-top: 4px;
    color: var(--demo-muted);
    font-size: 11px;
    line-height: 1.6;
  }
  .delivery-note {
    margin-top: 27px;
    padding-top: 22px;
    border-top: 1px solid var(--demo-line);
  }
  .delivery-note strong {
    font-size: 12px;
  }
  .delivery-note p {
    margin-top: 6px;
    color: var(--demo-muted);
    font-size: 12px;
    line-height: 1.8;
  }
  .step-tabs {
    grid-row: 2;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 15px;
    width: min(100%, 710px);
    margin: 20px auto 0;
  }
  .step-tabs button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-width: 0;
    min-height: 48px;
    padding: 10px 5px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: transparent;
    color: #d3e0e9;
    font: inherit;
    font-size: 14px;
    line-height: 1.5;
    cursor: pointer;
    transition:
      color 150ms ease,
      background 150ms ease;
  }
  .step-tabs button:hover {
    color: #fff;
    background: #ffffff0a;
  }
  .step-tabs button[aria-selected="true"] {
    color: #93ded8;
  }
  .step-number {
    display: grid;
    place-items: center;
    flex: 0 0 29px;
    height: 29px;
    border: 1px solid #9bb2c3;
    border-radius: 50%;
    font:
      13px ui-monospace,
      monospace;
  }
  [aria-selected="true"] .step-number {
    border-color: #67c7c0;
    background: #57c6c017;
  }
  .feedback-archive {
    grid-row: 3;
    width: min(100%, var(--feedback-width));
    margin-top: 39px;
    margin-inline: auto;
  }
  .result-summary {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(160px, 0.72fr) minmax(0, 1fr);
    align-items: stretch;
    gap: 14px;
  }
  .summary-quote {
    align-content: center;
    padding: 28px;
    border: 1px solid #b3c5d4;
    border-radius: 7px;
    background: #f4f8fc;
    font-size: clamp(15px, 1.6vw, 18px);
    line-height: 1.85;
    box-shadow: 0 8px 25px #0b172219;
  }
  .summary-thumbnail {
    padding: 9px;
    border: 1px solid #647f94;
    border-radius: 7px;
    background: #3a5368;
  }
  .summary-thumbnail figcaption {
    margin-top: 8px;
    font-size: 10px;
    line-height: 1.6;
    color: #e1ebf2;
  }
  .compact {
    padding: 9px 20px;
  }
  .compact .settings-heading {
    padding: 9px 8px 7px;
    font-size: 10px;
  }
  .compact .settings-heading svg {
    width: 12px;
    height: 12px;
  }
  .compact .settings-list {
    padding: 0 7px;
  }
  .compact .settings-list > span {
    padding: 4px 2px;
    font-size: 8px;
  }
  .compact .settings-list i {
    width: 4px;
    height: 4px;
  }
  .compact .illustrated-scrollbar {
    padding: 2px;
  }
  .compact .illustrated-scrollbar > span {
    height: 3px;
  }
  .summary-saved {
    display: grid;
    grid-template-columns: 33px 1fr;
    align-content: center;
    align-items: center;
    gap: 14px 10px;
    padding: 24px;
    border: 1px solid #67b7b1;
    border-radius: 7px;
    background: #173440;
    color: #a0e6df;
  }
  .save-mark {
    display: grid;
    place-items: center;
    width: 31px;
    height: 31px;
    border: 1px solid #67c7c0;
    border-radius: 50%;
  }
  .summary-saved strong {
    font-size: 14px;
    line-height: 1.6;
  }
  .summary-saved p {
    grid-column: 1 / -1;
    color: #c9e0e3;
    font-size: 12px;
    line-height: 1.85;
  }
  .feedback-details {
    margin-top: 13px;
    color: #d7e6ee;
  }
  .feedback-details summary {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    width: fit-content;
    min-height: 44px;
    margin-left: auto;
    padding: 8px 2px 8px 12px;
    border-radius: 4px;
    color: #a0e6df;
    font-size: 13px;
    font-weight: 600;
    line-height: 1.5;
    list-style: none;
    cursor: pointer;
  }
  .feedback-details summary::-webkit-details-marker {
    display: none;
  }
  .feedback-details summary svg {
    transition: transform 160ms ease;
  }
  .feedback-details[open] summary svg {
    transform: rotate(180deg);
  }
  .detail-content {
    display: grid;
    grid-template-columns: minmax(0, 1.45fr) minmax(0, 1fr);
    gap: 30px 40px;
    margin-top: 14px;
    padding: clamp(23px, 4vw, 40px);
    border: 1px solid #5c788d;
    border-radius: 7px;
    background: #213b4e;
  }
  .detail-label {
    display: block;
    margin-bottom: 17px;
    font-size: 11px;
    color: #bdd4e3;
    letter-spacing: 0.025em;
  }
  .detail-original blockquote {
    font-size: clamp(17px, 1.8vw, 21px);
    line-height: 1.9;
  }
  .preserved-note {
    display: block;
    margin-top: 16px;
    font-size: 12px;
    color: #91d7cf;
  }
  .detail-attachment {
    max-width: 260px;
  }
  .detail-attachment figcaption {
    color: #c0d2e0;
  }
  .detail-optional,
  .detail-delivery {
    grid-column: 1 / -1;
    padding-top: 25px;
    border-top: 1px solid #526e82;
  }
  .optional-heading {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 12px;
  }
  .optional-heading h4,
  .detail-delivery strong {
    font-size: 14px;
    font-weight: 600;
  }
  .optional-heading > span {
    color: #afc5d5;
    font-size: 11px;
  }
  .detail-optional p,
  .detail-delivery p {
    margin-top: 10px;
    max-width: 760px;
    font-size: 13px;
    line-height: 1.9;
    color: #c0d2e0;
  }
  .step-tabs button:focus-visible,
  .feedback-details summary:focus-visible {
    outline: 2px solid #a0e6df;
    outline-offset: 4px;
  }
  .step-panel:focus-visible {
    outline: 2px solid #3b759b;
  }
  @media (max-width: 850px) {
    .workbench-body {
      grid-template-columns: minmax(126px, 0.22fr) minmax(0, 1fr);
    }
    .case-sidebar {
      padding: 28px 17px;
    }
    .step-panel {
      padding: 30px;
    }
    .voice-panel {
      grid-template-columns: 1fr;
      gap: 25px;
    }
    .capture-figure {
      width: min(100%, 270px);
    }
    .package-files {
      grid-template-columns: 1fr;
    }
    .result-summary {
      grid-template-columns: minmax(0, 1.4fr) minmax(150px, 0.8fr);
    }
    .summary-saved {
      grid-column: 1 / -1;
      grid-template-columns: 33px auto 1fr;
      padding: 18px 23px;
    }
    .summary-saved p {
      grid-column: auto;
    }
  }
  @media (max-width: 600px) {
    .workbench {
      padding: 5px;
      border-radius: 9px;
    }
    .workbench-titlebar {
      align-items: flex-start;
      flex-direction: column;
      gap: 8px;
      padding: 12px 12px 15px;
    }
    .workbench-brand {
      font-size: 13px;
    }
    .example-label {
      font-size: 11px;
      text-align: left;
    }
    .workbench-body {
      display: block;
    }
    .case-sidebar {
      display: none;
    }
    .step-panel {
      padding: 27px 20px;
    }
    .step-panel h3 {
      font-size: 24px;
    }
    .brief-fields dd {
      font-size: 16px;
    }
    .brief-fields dt {
      align-items: flex-start;
      font-size: 14px;
    }
    .brief-fields dt > span {
      margin-top: 5px;
    }
    .step-panel .panel-note {
      margin-top: 24px;
    }
    .spoken-copy blockquote {
      padding-left: 14px;
      font-size: 19px;
    }
    .capture-figure {
      width: 100%;
      max-width: 280px;
    }
    .package-files > div {
      padding: 14px;
    }
    .workbench {
      grid-row: 2;
    }
    .step-tabs {
      grid-row: 1;
      gap: 4px;
      margin: 0 auto 15px;
    }
    .step-tabs button {
      flex-direction: column;
      gap: 8px;
      padding: 9px 3px;
      font-size: 12px;
    }
    .step-number {
      flex-basis: 27px;
      width: 27px;
      height: 27px;
    }
    .feedback-archive {
      margin-top: 28px;
    }
    .result-summary {
      display: flex;
      flex-direction: column;
      gap: 12px;
    }
    .summary-quote {
      padding: 25px;
      font-size: 17px;
    }
    .summary-thumbnail {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
      align-items: center;
      gap: 17px;
      padding: 13px;
    }
    .summary-thumbnail figcaption {
      margin: 0;
      font-size: 11px;
    }
    .compact {
      padding: 8px;
    }
    .summary-saved {
      grid-template-columns: 33px 1fr;
      padding: 22px;
    }
    .summary-saved p {
      grid-column: 1 / -1;
    }
    .feedback-details summary {
      font-size: 14px;
    }
    .detail-content {
      grid-template-columns: minmax(0, 1fr);
      gap: 28px;
    }
    .detail-optional,
    .detail-delivery {
      grid-column: auto;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .step-tabs button,
    .feedback-details summary svg {
      transition: none;
    }
  }
</style>
