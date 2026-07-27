/**
 * [INPUT]: 无（纯数据 + HTML 片段）
 * [OUTPUT]: PLAN_TEMPLATES · planTemplateById · chat/welcome 空态 HTML
 * [POS]: P-ship-D features/templates/catalog.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

function esc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Built-in cold-start plan templates (markdown body). */
export const PLAN_TEMPLATES = {
  "overseas-landing": {
    id: "overseas-landing",
    title: "出海落地页",
    short: "出海落地页",
    hint: "文案 · SEO · 表单 · 多语言要点",
    markdown: `# 出海落地页计划

> 模板：可改 · 保存后点「拆成步骤」进入拆分台核对
> 选型与体验规则真源：软件内 \`docs/runtime-prompts/\`（可被 ~/.cco/runtime-prompts 覆盖）

## 目标

为一款面向目标市场的产品，产出可上线的落地页（或等价静态页），让访客在 30 秒内看懂价值并愿意留下线索。

## 非目标

- 完整后台、会员体系、支付闭环（除非你改写本段）
- 微服务 / 中台 / 为「以后百万访问」上的重架构

## 会失去什么

- 本轮不做多语全站与复杂增长实验；先一语一页可上线
- 不做完整转化漏斗后台，线索以表单/外链为主

## 范围

### 做

- 语言：先做 **一语**（请改成：英语 / 日语 / …）
- 渠道：自然搜索 + 投放落地（共用一页即可）
- 页面：首屏价值 + 证据 + FAQ + 主 CTA / 表单

### 不做

- 同「非目标」；此处可再列本轮边界细节

## 建议技术（默认 · 可改）

- **形态**：静态页或 SSG（优先简单可部署）
- **语言/框架**：HTML 或 Astro（仓库已有前端则冻结现有栈，勿另起炉灶）
- **部署**：Cloudflare Pages / Vercel / GitHub Pages 一类
- **图标**：开源线标（Lucide 等），禁止 emoji 当按钮图标
- **为什么**：出海个人/增长页要快上线、可改文案；复杂栈拖慢交付

## 验收

- [ ] 主标题 + 副标题说清「给谁 · 解决什么 · 凭什么信」
- [ ] 至少 3 个利益点 / 场景，避免纯功能罗列
- [ ] SEO：title / description / 主 H1 与关键词一致；需要时注明 hreflang 策略
- [ ] 主 CTA + 次 CTA 清晰；表单字段 ≤ 5，有提交成功 / 失败人话反馈
- [ ] 移动端首屏可读；关键按钮可点；空态/加载有说明
- [ ] 合规：隐私政策入口、Cookie/地区提示（按目标市场勾选）

## 风险

- 文案空心 / 无证据 → 访客不信；先补 1–2 条可核验证据
- 表单无反馈 → 线索丢失；必须有成功/失败人话

## 建议步骤（拆分时可调整）

1. 定受众与一句话卖点
2. 信息架构与线框（首屏 / 证据 / FAQ / 表单）
3. 文案成稿（含 SEO 元信息）
4. 页面实现（按「建议技术」；静态或项目内既有栈）
5. 表单与追踪事件（可标可选）
6. 自检清单对照「验收」

## 约束与备注

- 文案语气：________（专业 / 轻松 / 技术）
- 品牌色与禁止用语：________
- 参考竞品：________
`,
  },
  "req-outline": {
    id: "req-outline",
    title: "通用需求大纲",
    short: "通用需求大纲",
    hint: "目标 · 非目标 · 会失去什么 · 验收 · 风险",
    markdown: `# 通用需求大纲

> 模板：可改 · 保存后点「拆成步骤」进入拆分台核对

## 背景

用 2–4 句说明：为什么现在做这件事、不解决会怎样。

## 目标

- 业务目标：________
- 用户能完成的结果：________
- 首批给谁用：________

## 非目标

这轮**明确不做**（可改；写清比空着好）：

- …
- …

## 会失去什么

若只做本轮范围，暂缓或放弃的：

- …
- …

## 用户与场景

| 角色 | 场景 | 期望结果 |
|------|------|----------|
|      |      |          |

## 验收

怎样算做完（可观察、可勾选）：

- [ ] …
- [ ] …
- [ ] …

## 风险

- 风险：________
- 待确认 / 未决：________

## 建议拆法（给人看，非强制 DAG）

1. 对齐目标与非目标
2. 设计 / 方案要点
3. 实现主路径
4. 对照「验收」自检
5. （可选）文档与交接
`,
  },
};

/** @returns {object|null} */
export function planTemplateById(id) {
  if (!id) return null;
  return PLAN_TEMPLATES[id] || null;
}

/** Chat empty-state HTML (B1): coach line + 3 examples + 2 templates; no eng jargon. */
export function planTemplateChatEmptyHtml() {
  const tplBtns = Object.values(PLAN_TEMPLATES)
    .slice(0, 2)
    .map(
      (t) =>
        `<button type="button" class="chat-example-chip chat-tpl-chip" data-plan-template="${esc(
          t.id
        )}" title="${esc(t.hint || "从模板开始")}">${esc(t.short)}</button>`
    )
    .join("");
  return `
      <div class="chat-empty muted">
        <p class="chat-empty-coach">懒得打字？点一个例子，填进下面再改两句就行。</p>
        <div class="chat-example-chips">
          <button type="button" class="chat-example-chip" data-chat-example="做一个提醒浇水的小工具，自己先用，先不做社区">浇水提醒小工具</button>
          <button type="button" class="chat-example-chip" data-chat-example="把产品官网改成双语落地页，含表单与 SEO 要点">出海落地页</button>
          <button type="button" class="chat-example-chip" data-chat-example="优化登录与注册体验，写清范围和怎样算做完">优化登录体验</button>
        </div>
        <p class="chat-hint chat-hint-tpl">想用完整模板起笔：</p>
        <div class="chat-example-chips chat-tpl-chips">${tplBtns}</div>
      </div>`;
}

/** Welcome / multi-project empty template row HTML. */
export function planTemplateWelcomeHtml() {
  const btns = Object.values(PLAN_TEMPLATES)
    .map(
      (t) =>
        `<button class="btn ghost sm" type="button" data-plan-template="${esc(
          t.id
        )}" title="${esc(t.hint || "")}">${esc(t.short)}</button>`
    )
    .join("");
  return (
    `<div class="welcome-templates" id="welcome-templates">` +
    `<p class="muted welcome-tpl-label">从模板开始（需先选中项目）</p>` +
    `<div class="welcome-template-row">${btns}</div>` +
    `</div>`
  );
}
