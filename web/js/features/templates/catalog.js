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

## 目标

为一款面向目标市场的产品，产出可上线的落地页（或等价静态页），让访客在 30 秒内看懂价值并愿意留下线索。

## 范围

- 语言：先做 **一语**（请改成：日语 / 英语 / …）
- 渠道：自然搜索 + 投放落地（共用一页即可）
- 不包含：完整后台、支付闭环（除非你改写本段）

## 成功标准（怎样算做完）

- [ ] 主标题 + 副标题说清「给谁 · 解决什么 · 凭什么信」
- [ ] 至少 3 个利益点 / 场景，避免纯功能罗列
- [ ] SEO：title / description / 主 H1 与关键词一致；需要时注明 hreflang 策略
- [ ] 主 CTA + 次 CTA 清晰；表单字段 ≤ 5，有提交成功反馈文案
- [ ] 移动端首屏可读；关键按钮可点
- [ ] 合规：隐私政策入口、Cookie/地区提示（按目标市场勾选）

## 建议步骤（拆分时可调整）

1. 定受众与一句话卖点
2. 信息架构与线框（首屏 / 证据 / FAQ / 表单）
3. 文案成稿（含 SEO 元信息）
4. 页面实现（静态或项目内既有栈）
5. 表单与追踪事件（可标可选）
6. 自检清单对照「成功标准」

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
    hint: "背景 · 目标 · 范围 · 验收 · 风险",
    markdown: `# 通用需求大纲

> 模板：可改 · 保存后点「拆成步骤」进入拆分台核对

## 背景

用 2–4 句说明：为什么现在做这件事、不解决会怎样。

## 目标

- 业务目标：________
- 用户能完成的结果：________

## 范围

### 做

- …

### 不做

- …

## 用户与场景

| 角色 | 场景 | 期望结果 |
|------|------|----------|
|      |      |          |

## 成功标准（怎样算做完）

- [ ] …
- [ ] …
- [ ] …

## 依赖与约束

- 依赖系统 / 数据 / 人：________
- 时间或资源上限：________

## 风险与开放问题

- 风险：________
- 待确认：________

## 建议拆法（给人看，非强制 DAG）

1. 对齐目标与范围
2. 设计 / 方案要点
3. 实现主路径
4. 验收对照成功标准
5. （可选）文档与交接
`,
  },
};

/** @returns {object|null} */
export function planTemplateById(id) {
  if (!id) return null;
  return PLAN_TEMPLATES[id] || null;
}

/** Chat empty-state HTML (T2); keeps example chips + template one-click. */
export function planTemplateChatEmptyHtml() {
  const tplBtns = Object.values(PLAN_TEMPLATES)
    .map(
      (t) =>
        `<button type="button" class="chat-example-chip chat-tpl-chip" data-plan-template="${esc(
          t.id
        )}" title="${esc(t.hint || "一键落盘到 plans/")}">${esc(t.short)}</button>`
    )
    .join("");
  return `
      <div class="chat-empty muted">
        <p>用自然语言说明你要做什么。AI 会先帮你写成一份<strong>计划文档</strong>，保存后再点「拆成步骤」进入拆分台核对。</p>
        <p class="chat-hint">点示例填入输入框，改完再发送：</p>
        <div class="chat-example-chips">
          <button type="button" class="chat-example-chip" data-chat-example="优化登录与注册体验，写清范围和验收">优化登录体验</button>
          <button type="button" class="chat-example-chip" data-chat-example="排查并修复 flaky 测试，列出可疑用例与步骤">修 flaky 测试</button>
          <button type="button" class="chat-example-chip" data-chat-example="为当前模块补用户文档与上手步骤">补模块文档</button>
        </div>
        <p class="chat-hint chat-hint-tpl">或从模板一键落盘到 plans/，改完再「拆成步骤」：</p>
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
