#!/usr/bin/env node
/**
 * Pure smoke: assistant A/B/C quiz parser must fire on real model prose.
 *
 * Catches the regression where `**1. 标题？**` + MD hard-breaks made
 * parseAssistantQuiz return null → bubble stayed plain text (looks like
 * 「点选没做」). No DOM / no Tauri.
 *
 *   node scripts/chat-quiz-parse-smoke.mjs
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const enhancePath = path.join(
  ROOT,
  "web/js/features/chat/chatMsgEnhance.js"
);

const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond, detail: detail ? String(detail) : "" });
}

const { parseAssistantQuiz } = await import(pathToFileURL(enhancePath).href);

// Real session shape (test/9 · 宠物出海站) — bold titles + hard breaks + ---
const real = `收到：要做**宠物出海站**，主卖宠物食品 + 宠物工具。目录目前是空仓，适合从想法收成计划。

先把五件事对齐（每题选 A/B/C，或写「其他」；也可说「你定 / 直接出计划」我整包假设后写 plan）：

---

**1. 主要给谁用？**
A. 海外 C 端养宠人（浏览+下单）
B. 海外经销商 / 买手（看品类+询价/留资）
C. 先自己团队演示路演用
其他：…

**2. 最痛的一刻是什么？**
A. 海外买家找不到可信的中文品牌出海页
B. 品类多，官网乱，不知道先买什么
C. 有货但没法在线询价/下单闭环
其他：…

**3. 做成后外面能看见什么（V1 可观察结果）？**
A. 一页营销站：品类介绍 + 主 CTA（如「立即选购 / 获取报价」）
B. 小型电商：商品列表 + 详情 + 加购/结账（真或半真）
C. 目录站 + 留资表单（询价/WhatsApp/邮件），暂不自建支付
其他：…

**4. 明确不做哪些？（可多选）**
A. 不做会员体系 / 社区
B. 不做完整后台 ERP
C. 先不做多语言全站（只英文或中英一套）
D. 先不做真实支付（只演示加购）
其他：…

**5. 怎样算做完？**
A. 本机可打开，30 秒讲清「卖什么、给谁、怎么行动」
B. 能给运营同学按 README 起服预览
C. 有可勾选验收清单（门禁 + 主路径）
其他：…

---

你回选项即可。若说 **「你定」**，我会按默认假设落成：
**R-overseas + ecommerce 轻量 / 深度 A～B**，英文主语言，V1 商品目录 + 详情 + 主 CTA（加购或询价），暂不做完整支付与后台。`;

const quiz = parseAssistantQuiz(real);
check("real session parses", !!quiz, quiz ? `n=${quiz.questions.length}` : "null");
check(
  "real has 5 questions",
  quiz && quiz.questions.length === 5,
  quiz?.questions?.length
);
check(
  "Q4 is multi-select",
  quiz && quiz.questions[3]?.multi === true && quiz.questions[3]?.options?.length === 4,
  JSON.stringify(quiz?.questions?.[3] && {
    multi: quiz.questions[3].multi,
    nOpts: quiz.questions[3].options.length,
    title: quiz.questions[3].title,
  })
);
check(
  "Q1 title stripped of md",
  quiz && quiz.questions[0]?.title === "主要给谁用？",
  quiz?.questions?.[0]?.title
);
check(
  "single-select Q1 has ABC",
  quiz &&
    !quiz.questions[0]?.multi &&
    quiz.questions[0]?.options?.map((o) => o.key).join("") === "ABC",
  quiz?.questions?.[0]?.options?.map((o) => o.key).join("")
);

// Plain (no bold) still works
const plain = `对齐两问：

1. 给谁？
A. 自己
B. 客户
C. 团队

2. 做成啥？
A. 计划
B. 可跑
C. 演示`;
const p = parseAssistantQuiz(plain);
check("plain numbered still parses", !!p && p.questions.length === 2);

// **1.** / **A.** form
const boldKeys = `x

**1.** 谁？
**A.** 甲
**B.** 乙
**C.** 丙

**2.** 啥？
A. 一
B. 二
C. 三`;
const b = parseAssistantQuiz(boldKeys);
check(
  "bold number+key form",
  !!b && b.questions.length === 2 && b.questions[0].options[0].text === "甲",
  b?.questions?.[0]?.options?.[0]?.text
);

// Source must keep normalize helper (regression anchor)
const src = fs.readFileSync(enhancePath, "utf8");
check(
  "normalizeQuizSource present",
  /function normalizeQuizSource\s*\(/.test(src)
);
// Anchor: bold-wrapped numbered title rewrite lives in normalizeQuizSource
check(
  "bold title rewrite present",
  /normalizeQuizSource[\s\S]{0,800}\(\d\{1,2\}\)[\s\S]{0,200}\[\\.、．\)\]/.test(
    src
  ) || src.includes("**1. 标题")
);

const failed = results.filter((r) => !r.ok);
for (const r of results) {
  console.log(`${r.ok ? "OK" : "FAIL"}  ${r.name}${r.detail ? ` · ${r.detail}` : ""}`);
}
if (failed.length) {
  console.error(`\nchat-quiz-parse-smoke FAILED: ${failed.length}/${results.length}`);
  process.exit(1);
}
console.log(`\nchat-quiz-parse-smoke OK: ${results.length}/${results.length}`);
