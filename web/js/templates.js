/**
 * [INPUT]: state · ccoGateway/ccoChat（chatSavePlan/readPlanMd）· planJob（S14）
 * [OUTPUT]: 冷启动模板落盘 · 聊天/欢迎入口 · 拆分摘要写回（可选 CTA）
 * [POS]: web/js 波次 5 T1/T2/S14；A5-2e IPC 经 gateway（禁止业务 invoke）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — plan templates + split summary write-back */

const PLAN_TEMPLATES = {
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
function planTemplateById(id) {
  if (!id) return null;
  return PLAN_TEMPLATES[id] || null;
}

/** Chat empty-state HTML (T2); keeps example chips + template one-click. */
function planTemplateChatEmptyHtml() {
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
function planTemplateWelcomeHtml() {
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

/**
 * T1/T2: write template markdown under plans/, bind draft, open for edit.
 * Does not start plan job / confirm_start.
 */
async function applyPlanTemplate(templateId) {
  const tpl = planTemplateById(templateId);
  if (!tpl) {
    toast("未知模板");
    return null;
  }
  if (!state.selectedPath) {
    toast("请先添加并选择项目，再使用模板");
    if (typeof openModal === "function") openModal();
    return null;
  }
  if (typeof hasActiveRun === "function" && hasActiveRun()) {
    if (typeof toastRunLocked === "function") toastRunLocked("使用模板");
    else toast("有任务在跑，稍后再用模板");
    return null;
  }

  const plansDir =
    typeof getPlansDir === "function" ? getPlansDir() : "plans";
  const sessionId =
    (state.chatSession && state.chatSession.session_id) || "default";

  try {
    if (typeof ensureChatState === "function") ensureChatState();
    // A5-2e: IPC via gateway / ccoChat（命令名集中在 shared/gateway.js）
    const savePlan =
      (window.ccoChat && typeof window.ccoChat.savePlan === "function"
        ? window.ccoChat.savePlan.bind(window.ccoChat)
        : null) ||
      ((args) => requireGateway().chatSavePlan(args));
    const resp = await savePlan({
      project: state.selectedPath,
      markdown: tpl.markdown,
      sessionId,
      title: tpl.title,
      planRel: null,
      plansDir,
    });
    const path = resp.plan_rel;
    state.chatDraftPlan = path;
    state.chatProjectPath = state.selectedPath;
    if (state.chatSession) {
      state.chatSession.draft_plan = {
        path,
        saved: true,
        markdown: tpl.markdown,
        title: tpl.title,
      };
    }
    if (typeof stashChatSession === "function") {
      stashChatSession(state.selectedPath);
    }
    try {
      if (typeof loadPlansForPicker === "function") await loadPlansForPicker();
    } catch (_) {}
    try {
      if (typeof loadPlanRail === "function") await loadPlanRail();
    } catch (_) {}
    if (typeof selectPlan === "function") {
      await selectPlan(path, { keepSession: true }).catch(() => {});
    }
    if (typeof showPage === "function") showPage("chat");
    if (typeof renderChatPage === "function") renderChatPage();
    // Open editor so user can tweak before 拆成步骤
    if (typeof openPlanFullView === "function") {
      await openPlanFullView(path).catch(() => {});
    }
    toast(`已落盘：${path} · 可改后点「拆成步骤」`);
    return resp;
  } catch (e) {
    toast(String(e?.message || e));
    return null;
  }
}

/* ── S14: append split step titles to plan markdown (never clobber body) ── */

const SPLIT_SUMMARY_START = "<!-- cco-split-summary:start -->";
const SPLIT_SUMMARY_END = "<!-- cco-split-summary:end -->";

function buildSplitSummaryBlock(job) {
  const tasks = job?.tasks || [];
  const layers = job?.layers || [];
  const byId = Object.fromEntries(tasks.map((t) => [t.id, t]));
  const date = new Date().toISOString().slice(0, 10);
  const lines = [
    SPLIT_SUMMARY_START,
    "",
    "## 拆分步骤摘要",
    "",
    `> 由拆分台生成 · ${date} · 可选写回 · **不替代**上方正文`,
    "",
  ];
  if (!tasks.length) {
    lines.push("_（当前无步骤）_", "");
  } else if (layers.length) {
    layers.forEach((layer, i) => {
      lines.push(`### 波次 ${i + 1}`);
      lines.push("");
      (layer || []).forEach((id) => {
        const t = byId[id] || { id, title: id };
        const opt = t.optional ? " · 可选" : "";
        const sys =
          String(t.id || "").startsWith("sys-post-") ||
          String(t.group || "") === "系统收尾"
            ? " · 系统"
            : "";
        lines.push(`- [ ] ${t.title || id}${opt}${sys}`);
      });
      lines.push("");
    });
    // Orphans not in layers
    const seen = new Set(layers.flat());
    const rest = tasks.filter((t) => !seen.has(t.id));
    if (rest.length) {
      lines.push("### 其他");
      lines.push("");
      rest.forEach((t) => lines.push(`- [ ] ${t.title || t.id}`));
      lines.push("");
    }
  } else {
    tasks.forEach((t) => {
      const opt = t.optional ? " · 可选" : "";
      lines.push(`- [ ] ${t.title || t.id}${opt}`);
    });
    lines.push("");
  }
  lines.push(SPLIT_SUMMARY_END);
  return lines.join("\n");
}

function mergeSplitSummaryIntoMarkdown(existing, block) {
  const body = String(existing || "").replace(/\s*$/, "");
  const re =
    /<!-- cco-split-summary:start -->[\s\S]*?<!-- cco-split-summary:end -->\n?/;
  if (re.test(body)) {
    return body.replace(re, block.trim() + "\n");
  }
  return body + "\n\n" + block.trim() + "\n";
}

/**
 * S14: optional CTA — write step titles to plan end; default off (must click).
 * Does not overwrite user prose; only replaces previous cco-split-summary block.
 */
/** Enable/disable optional writeback CTA on split desk (called from renderConfirmPanel). */
function refreshSplitWritebackBtn(runLocked, editing) {
  const btn = document.getElementById("btn-split-writeback");
  if (!btn) return;
  const hasJob = !!(state.planJob && (state.planJob.tasks || []).length);
  btn.disabled = !!runLocked || !!editing || !hasJob || !state.selectedPath;
  btn.hidden = false; // always visible on desk; default off means no auto-write
  btn.title = runLocked
    ? "运行中不可写回"
    : "把步骤标题追加到计划文末（不覆盖正文；需点击确认）";
}

async function writeSplitSummaryToPlan() {
  const job = state.planJob;
  if (!job || !state.selectedPath) {
    toast("当前没有可写回的拆分结果");
    return;
  }
  if (typeof hasActiveRun === "function" && hasActiveRun()) {
    if (typeof toastRunLocked === "function") toastRunLocked("写回步骤摘要");
    else toast("运行中不可写回");
    return;
  }
  const planPath =
    job.plan_path ||
    job.planPath ||
    state.selectedPlan ||
    state.chatDraftPlan;
  if (!planPath) {
    toast("找不到计划文件路径");
    return;
  }
  const rel =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(planPath) || planPath
      : planPath;

  try {
    const g = requireGateway();
    let existing = "";
    try {
      existing = await g.readPlanMd(state.selectedPath, rel);
    } catch (e) {
      toast(`读取计划失败：${e?.message || e}`);
      return;
    }
    const block = buildSplitSummaryBlock(job);
    const next = mergeSplitSummaryIntoMarkdown(existing, block);
    if (next === existing) {
      toast("摘要无变化");
      return;
    }
    const ok = window.confirm(
      `将把步骤标题清单写到计划文末（不覆盖正文）：\n${rel}\n\n确定写回？`
    );
    if (!ok) return;

    const savePlan =
      (window.ccoChat && typeof window.ccoChat.savePlan === "function"
        ? window.ccoChat.savePlan.bind(window.ccoChat)
        : null) || ((args) => g.chatSavePlan(args));
    await savePlan({
      project: state.selectedPath,
      markdown: next,
      sessionId:
        (state.chatSession && state.chatSession.session_id) || "default",
      title: null,
      planRel: rel,
      plansDir: null,
    });
    if (state.chatDraftPlan === rel && state.chatSession?.draft_plan) {
      state.chatSession.draft_plan.markdown = next;
    }
    toast(`已写回步骤摘要 → ${rel}`);
  } catch (e) {
    toast(String(e?.message || e));
  }
}
