/**
 * [INPUT]: localStorage · 01b persona knobs (docs/path-depth-wave …/01b)
 * [OUTPUT]: PersonaId → opener/examples/primary_cta/direct_exec + scene chips
 * [POS]: features/chat — W0-7 分群可感知（与 pathMode L/M/H 正交）
 * [PROTOCOL]: 变更时对照 docs/.../01b-persona-path-profiles.md；禁止进门强制选职业
 *
 * 只影响 author 文案与按钮权重；不写 gateway、不开跑、不复制九套 App。
 */

const KEY = "cco.personaId";

/** In-memory fallback when localStorage is unavailable (tests / private mode). */
let memoryPersona = /** @type {PersonaId|null} */ (null);

/** In-memory tracking for clarify_depth and split_grain chips (not persisted yet). */
let currentChips = {
    clarify_depth: 'soft1',      // default per W1-6 profiles
    split_grain: 'balanced'     // matching W1-6 defaults
};

/**
 * @typedef {'founder'|'pm'|'designer'|'ops'|'creator'|'ecom'|'edu'|'admin'|'biz_lead'} PersonaId
 * @typedef {'offer'|'hide'|'danger_only'} DirectExec
 * @typedef {{
 *   id: PersonaId,
 *   label: string,
 *   pathBias: 'L'|'M'|'H',
 *   opener: string,
 *   coach: string,
 *   examples: Array<{ short: string, fill: string }>,
 *   primaryCta: string,
 *   directExec: DirectExec,
 *   doneWhenHint: string,
 *   nonGoalHint: string,
 *   default_clarify_depth?: string,
 *   default_split_grain?: string,
 * }} PersonaProfile
 */

/** @type {Record<PersonaId, PersonaProfile>} */
export const PERSONA_PROFILES = Object.freeze({
  founder: {
    id: "founder",
    label: "创业/小老板",
    pathBias: "L",
    opener: "用一句话说：谁、有多痛、你想先做成哪一个能看的结果",
    coach: "先做能给客户看的最小版本；别的写进「不做」。",
    examples: [
      {
        short: "报价可留资",
        fill: "做一个内部报价小工具，自己和销售先用，先不做客户登录和支付",
      },
      {
        short: "周末小验证",
        fill: "周末验证一个提醒类小工具，自己先用，三分钟能演示给别人看",
      },
    ],
    primaryCta: "生成并看看",
    directExec: "offer",
    doneWhenHint: "能打开链接 · 主按钮可点 · 能演示给客户 3 分钟",
    nonGoalHint: "先不做：支付/会员/复杂后台/多端",
    default_clarify_depth: 'none',
    default_split_grain: 'coarse',
  },
  pm: {
    id: "pm",
    label: "产品经理",
    pathBias: "M",
    opener: "要推进哪条需求？可粘 PRD/纪要；说清做成什么样、先不做啥",
    coach: "先收成可派工的计划，再拆开跑；做完按清单勾。",
    examples: [
      {
        short: "单需求可执行",
        fill: "把一条产品需求写成可执行计划：目标、不做、验收清单，再拆成步骤",
      },
      {
        short: "对照验收",
        fill: "整理一版需求计划，成功标准写成可勾选清单，方便对照巡检",
      },
    ],
    primaryCta: "拆成步骤",
    directExec: "hide",
    doneWhenHint: "验收条可勾 · 对照计划章节 · 遗漏可回补",
    nonGoalHint: "先不做：范围外 epic、未定调研当实现",
    default_clarify_depth: 'soft1',
    default_split_grain: 'balanced',
  },
  designer: {
    id: "designer",
    label: "设计/UI",
    pathBias: "M",
    opener: "第一眼用户看到什么？主按钮是什么？有没有不能动的视觉约束？",
    coach: "先定第一眼和主按钮，再拆结构与走查。",
    examples: [
      {
        short: "落地首屏",
        fill: "做落地页首屏三块：价值句、证据、主 CTA，预览要像设计意图",
      },
      {
        short: "表单走查",
        fill: "表单两步走查：空/错/成功态人话，主按钮位置清晰",
      },
    ],
    primaryCta: "拆成可预览的步骤",
    directExec: "hide",
    doneWhenHint: "预览像意图 · 主 CTA 位置对 · 关键态不崩",
    nonGoalHint: "先不做：设计系统大一统、动效全集、多主题",
    default_clarify_depth: 'soft1',
    default_split_grain: 'balanced',
  },
  ops: {
    id: "ops",
    label: "运营/出海",
    pathBias: "H",
    opener: "本波要上几件事？截止日期？哪些页必须一起亮、哪些可后补？",
    coach: "先排本波几页谁先谁并，再拆每页步骤。",
    examples: [
      {
        short: "大促三页",
        fill: "大促本波三页：活动主页、规则页、留资表，要写清截止与必上清单",
      },
      {
        short: "双语 + 表单",
        fill: "中英双语落地页 + 留资表单，本波一起亮，先不做完整后台",
      },
    ],
    primaryCta: "生成本波目录",
    directExec: "hide",
    doneWhenHint: "页可开 · 表可收 · 语种齐 · 活动日状态可指认",
    nonGoalHint: "先不做：年架重构、无关产品线",
    default_clarify_depth: 'soft2',
    default_split_grain: 'balanced',
  },
  creator: {
    id: "creator",
    label: "内容/自媒体",
    pathBias: "L",
    opener: "链接要放哪、读者是谁、看完要他点什么？",
    coach: "先做能放进简介的一页，语气不对再改一版。",
    examples: [
      {
        short: "简介铅封",
        fill: "做一页简介铅封：读者是谁、看完要点什么，语气像我平时说话",
      },
      {
        short: "报名加圈",
        fill: "做报名/加圈一页，主 CTA 清晰，能直接发链接",
      },
    ],
    primaryCta: "生成并看看",
    directExec: "offer",
    doneWhenHint: "链接能发 · 像你的语气 · 主 CTA 可点",
    nonGoalHint: "先不做：社区、复杂会员、全站 CMS",
    default_clarify_depth: 'none',
    default_split_grain: 'coarse',
  },
  ecom: {
    id: "ecom",
    label: "电商卖家",
    pathBias: "M",
    opener: "类目、卖点三句、必须出现的规格；禁止编造的参数有哪些？",
    coach: "先锁卖点与禁编造，再拆模块；多品分开计划防串。",
    examples: [
      {
        short: "单品可上架",
        fill: "做单品详情可上架：卖点三句、规格、FAQ；禁止编造参数，主图位说清",
      },
      {
        short: "活动 + 券",
        fill: "活动页 + 券说明，上架清单齐：必含词、模块齐、无违禁编造",
      },
    ],
    primaryCta: "拆成上架步骤",
    directExec: "danger_only",
    doneWhenHint: "上架清单：必含词、模块齐、无违禁编造、主图位说明清",
    nonGoalHint: "先不做：整站商城、支付中台",
    default_clarify_depth: 'soft1',
    default_split_grain: 'balanced',
  },
  edu: {
    id: "edu",
    label: "教培",
    pathBias: "M",
    opener: "学员是谁、学完能做什么、作业/报名怎么交？",
    coach: "按学员路径写：打开→学会→交作业。",
    examples: [
      {
        short: "单课 + 作业",
        fill: "单课说明 + 作业提交：学员是谁、学完能做什么、怎样交作业",
      },
      {
        short: "报名页",
        fill: "课程报名说明页：时间地点费用、报名表，做成后能发给学员打开",
      },
    ],
    primaryCta: "拆成开课步骤",
    directExec: "hide",
    doneWhenHint: "学员打开不懵 · 交作业/报名通 · 下学期能改一章",
    nonGoalHint: "先不做：直播中台、学分系统",
    default_clarify_depth: 'soft2',
    default_split_grain: 'balanced',
  },
  admin: {
    id: "admin",
    label: "行政/HR",
    pathBias: "M",
    opener: "文件给谁看、是否正式发布、哪几条表述不能 AI 擅改？",
    coach: "先出草案，你确认前系统不会当正式发布去跑。",
    examples: [
      {
        short: "制度说明",
        fill: "制度说明页：给谁看、哪些表述必审不能擅改，先出草案等人确认",
      },
      {
        short: "收集表 + 说明",
        fill: "内宣一页 + 收集确认表，可存档，先不做自动对外群发",
      },
    ],
    primaryCta: "生成草案并等人确认",
    directExec: "hide",
    doneWhenHint: "可存档 · 流程可走 · 必审句未乱改 · 表可收",
    nonGoalHint: "先不做：自动对外群发、改薪酬口径",
    default_clarify_depth: 'soft2',
    default_split_grain: 'balanced',
  },
  biz_lead: {
    id: "biz_lead",
    label: "业务负责人",
    pathBias: "M",
    opener: "这季度要推进哪件业务结果？成功时外面能看见什么？",
    coach: "像管项目：计划→分工→进度→齐了没。",
    examples: [
      {
        short: "业务结果计划",
        fill: "写一页业务结果计划：可观察结果、不做、怎样算完，方便汇报",
      },
      {
        short: "多部门本波",
        fill: "多部门本波目录：几条线谁先谁并，成功时外面能看见什么",
      },
    ],
    primaryCta: "拆成步骤",
    directExec: "hide",
    doneWhenHint: "业务结果可观察 · 负责人能汇报",
    nonGoalHint: "先不做：技术债大扫除当业务结果",
    default_clarify_depth: 'soft1',
    default_split_grain: 'coarse',
  },
});

export const DEFAULT_PERSONA = /** @type {PersonaId} */ ("founder");

/** Scene chips = weak persona entry (not job ID card). */
export const SCENE_CHIPS = Object.freeze([
  {
    id: "validate_idea",
    label: "验证想法",
    persona: /** @type {PersonaId} */ ("founder"),
  },
  {
    id: "write_req",
    label: "写需求计划",
    persona: /** @type {PersonaId} */ ("pm"),
  },
  {
    id: "campaign_multi",
    label: "大促多页",
    persona: /** @type {PersonaId} */ ("ops"),
  },
  {
    id: "listing",
    label: "上架详情",
    persona: /** @type {PersonaId} */ ("ecom"),
  },
  {
    id: "course",
    label: "开课说明",
    persona: /** @type {PersonaId} */ ("edu"),
  },
  {
    id: "policy",
    label: "制度发文",
    persona: /** @type {PersonaId} */ ("admin"),
  },
]);

// Apply persona defaults to scene chips when clicked
if (typeof document !== "undefined") {
    document.addEventListener("click", (e) => {
        const chip = e.target.closest("[data-scene-chip]");
        if (chip) {
            const persona = chip.dataset.persona;
            if (persona) {
                const p = PERSONA_PROFILES[persona];
                if (p) {
                    currentChips.clarify_depth = p.default_clarify_depth || 'soft1';
                    currentChips.split_grain = p.default_split_grain || 'balanced';
                }
            }
        }
    });
}

/** @returns {PersonaId} */
export function getPersonaId() {
  try {
    const raw = String(localStorage.getItem(KEY) || "").trim();
    if (raw && PERSONA_PROFILES[/** @type {PersonaId} */ (raw)]) {
      return /** @type {PersonaId} */ (raw);
    }
  } catch (_) {}
  if (memoryPersona && PERSONA_PROFILES[memoryPersona]) return memoryPersona;
  return DEFAULT_PERSONA;
}

/** Get current chip values. */
export function getChipValue(type) { return currentChips[type]; }

/** Set chip value (used when persona/chip clicked). */
export function setChipValue(type, value) { 
    currentChips[type] = value; 
}

/**
 * @param {PersonaId|string} id
 * @returns {PersonaId}
 */
export function setPersonaId(id) {
  const next =
    id && PERSONA_PROFILES[/** @type {PersonaId} */ (id)]
      ? /** @type {PersonaId} */ (id)
      : DEFAULT_PERSONA;
  memoryPersona = next;
  try {
    localStorage.setItem(KEY, next);
  } catch (_) {}
  // Set chip defaults based on persona profile
  const profile = PERSONA_PROFILES[next];
  if (profile) {
    currentChips.clarify_depth = profile.default_clarify_depth || 'soft1';
    currentChips.split_grain = profile.default_split_grain || 'balanced';
  }
  return next;
}

/** @param {PersonaId} [id] */
export function getPersonaProfile(id) {
  const pid = id || getPersonaId();
  return PERSONA_PROFILES[pid] || PERSONA_PROFILES[DEFAULT_PERSONA];
}

/** Scene chip row HTML for empty state. */
export function sceneChipsHtml() {
  const cur = getPersonaId();
  const btns = SCENE_CHIPS.map((c) => {
    const active = c.persona === cur ? " is-active" : "";
    return (
      `<button type="button" class="chat-scene-chip${active}"` +
      ` data-scene-chip="${escapeHtml(c.id)}"` +
      ` data-persona="${escapeHtml(c.persona)}"` +
      ` title="套用${escapeHtml(PERSONA_PROFILES[c.persona]?.label || "")}口吻（可改）">` +
      `${escapeHtml(c.label)}` +
      `</button>`
    );
  }).join("");
  return (
    `<div class="chat-scene-chips" role="group" aria-label="场景例子">` +
    `<p class="chat-scene-label">更像哪类事？（可选，不是选职业）</p>` +
    `<div class="chat-example-chips chat-scene-row">${btns}</div>` +
    `</div>`
  );
}

/** Example chips from active persona (≥2). */
export function personaExampleChipsHtml() {
  const p = getPersonaProfile();
  const chips = (p.examples || [])
    .map(
      (ex) =>
        `<button type="button" class="chat-example-chip" data-chat-example="${escapeHtml(
          ex.fill
        )}" data-persona-example="1">${escapeHtml(ex.short)}</button>`
    )
    .join("");
  return (
    `<div class="chat-persona-examples">` +
    `<p class="chat-empty-coach">${escapeHtml(p.coach)}</p>` +
    `<div class="chat-example-chips">${chips}</div>` +
    `<p class="chat-persona-lexicon muted">怎样算完（这路默认）：${escapeHtml(
      p.doneWhenHint
    )}</p>` +
    `</div>`
  );
}

/** Apply opener to #chat-input placeholder. */
export function applyPersonaOpener(id) {
  if (typeof document === "undefined") return;
  const input = document.getElementById("chat-input");
  if (!input) return;
  const p = getPersonaProfile(id);
  input.setAttribute("placeholder", p.opener);
  input.setAttribute("title", p.opener);
}

/**
 * Primary CTA label for plan card (path L may still prefer 生成并看看 when founder).
 * @param {PersonaId} [id]
 */
export function personaPrimaryCtaLabel(id) {
  return getPersonaProfile(id).primaryCta || "拆成步骤";
}

/**
 * @param {PersonaId} [id]
 * @returns {DirectExec}
 */
export function personaDirectExec(id) {
  return getPersonaProfile(id).directExec || "offer";
}

function escapeHtml(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
