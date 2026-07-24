# 人话状态 + 可执行验收 · 双层展示落地计划

> 日期：2026-07-24  
> 角色：**已归档**（H0–H3 ✅ · **勿当缺口 · 勿继承勾选**）  
> 产品方向：[`../../PRODUCT.md`](../../PRODUCT.md)（完成 = 对照计划，不是 exit 0 · 业务语言优先 · 进度看得见）  
> 架构：[`../architecture-redesign-2026-07-20.md`](../architecture-redesign-2026-07-20.md)（confirm 唯一开跑 · MVVM · 文件硬上限）  
> 拆分行为短规则：[`../split-product-rules.md`](../split-product-rules.md)（**行为规则现行**）  
> 拆分存储真源：[`../cco-split-format-sqlite-2026-07-21.md`](../cco-split-format-sqlite-2026-07-21.md)（字段表含 `verify_cmd`；**本计划勾选不进 S2–S6**）  
> 并行 / 混跑参考：[`../multi-cli-collaboration-2026-07-18.md`](../multi-cli-collaboration-2026-07-18.md)（无自动 git merge）  
> 巡检对照：[`../plan-execute-inspect-rework-2026-07-19.md`](../plan-execute-inspect-rework-2026-07-19.md)（inspect 权威 · 清单不 auto-run）  
> 记忆地基：`project_last_summary` / `compose_last_summary`（P2-2 · archive pilotdeck）  
> 状态：**H0–H3 ✅ · 2026-07-24 迁入 archive/**

[PROTOCOL]: **历史勾选 · 勿再当任务表**。行为规则已进 split-product-rules / cco-split 字段表。禁止平行第二套「人话/状态」阶段表；禁止把计划验收 checklist 升为 host 自动真源；禁止 JS 里写 `looks_like_shell` / Mode B / 状态叙事策略；禁止 STATE.md 与 job/run 争真源；禁止 host 自动 git merge/PR。

---

## 0. 从评判收成什么

论坛对照 claude-flow 的三条短板，裁决为 **「部分能做」**：

```text
① 怎样算做完：主路径人话；底层可选 shell —— 先止血单字段双用，再拆字段
③ 现在卡在哪：机器态已有；缺跨 CLI/桌面/TUI 的统一人话状态句
④ 并行合并验：路径所有权已有；缺「拼在一起怎么验」浅白话术 DTO
```

**一句话产品形态**：两种展示、一种真源（job/run/SQLite）——投影给人看的大白话，和给 host/AI 跑的命令/路径门禁，**不得共用一个字符串槽**。

### 0.1 一句话目标

让非开发用户在拆分/执行/结束全程看见稳定人话（「怎样算做完」「卡在哪」「拼完怎么验」）；AI/host 仍跑路径门禁、可选 shell、inspect 终闸；**完成权威仍是对照计划 + 巡检，不是 exit 0**。

### 0.2 非目标

| 不做 | 原因 |
|------|------|
| 用命令+期望 stdout 取代 inspect / 计划对照 | PRODUCT：完成 = 对照计划 |
| 计划级 checklist host 自动逐条跑 | P2-1：清单 structure-only；inspect 权威 |
| STATE.md 第二真源 | 与 SQLite job / run.json 双真源 |
| 核心自动 git merge / 3-way / 自动 PR | multi-cli 明文禁令 |
| 在 JS 判定「像不像 shell」或拼业务状态句 | L1 #22 · 策略在 app/domain |
| 继续往 `live.rs` / 厚 facade 堆逻辑 | 硬上限 · 只抽新薄模块 |
| 把本计划勾选并进 S2–S6 或 architecture 已 ✅ 表 | 禁止平行 / 回灌 |

### 0.3 硬契约

1. **唯一业务开跑**仍是 `split::confirm` / `confirm_start`。  
2. **`done_when` / 人话「怎样算做完」永不 `sh -c`。**  
3. **仅显式机器字段**（过渡：`looks_like_shell` 纯函数在 domain；终态：`verify_cmd`）才进 `run_acceptance_soft`。  
4. **`outputs` + inspect 门禁**保持；acceptance 失败仍可标任务 Failed（不是「软过」业务语义）。  
5. **Presentation 只渲染 app 下发的人话串**；gateway 不泄露策略。  
6. **主路径第一句人话**：无 `run_id` / 裸 `VERDICT` / 引擎名。  
7. **同屏新概念 ≤ 3**：步骤 · 等待 · 怎样算做完；`verify` / 命令默认折叠。  
8. CLI 与桌面共用同一 app 投影 API。

### 0.4 现状锚点（实施前必读）

| 层 | 现状 | 问题 |
|----|------|------|
| 展示 | `CcoSplitTask.done_when` · 拆分台「怎样算做完」· P1-4 stub 黄条 | 正确方向 |
| 物化 | `convert.rs`：`acceptance: t.done_when.clone()` | 人话灌进 shell 槽 |
| 调度 | `tick.rs` `apply_post_done_gates`：有 `acceptance` 就 `sh -c` | 无形态判断；`system_post` 中文会假失败 |
| 机器门禁 | `outputs` 缺路径失败 · inspect VERDICT/ISSUES | 已是非 shell 机器层 |
| 状态 | `report_summary_line` · `compose_last_summary` · 桌面五态/stall | CLI `common.rs` 仍 `status: {:?}`；无共享 `StatusOneLiner` |
| 并行 | scope 重叠串行/硬错误 · integrate → inspect | 缺用户可见「拼完怎么验」一句 |

关键路径：

- [`src/domain/plan/cco_split/convert.rs`](../../src/domain/plan/cco_split/convert.rs)  
- [`src/runtime/acceptance.rs`](../../src/runtime/acceptance.rs) · [`src/runtime/scheduler/tick.rs`](../../src/runtime/scheduler/tick.rs)  
- [`src/report/mod.rs`](../../src/report/mod.rs) · [`src/state/project_memory.rs`](../../src/state/project_memory.rs) · [`src/app/memory.rs`](../../src/app/memory.rs)  
- [`src/cli/commands/common.rs`](../../src/cli/commands/common.rs) · [`src/cli/commands/status.rs`](../../src/cli/commands/status.rs)  
- [`src/plan/system_post.rs`](../../src/plan/system_post.rs)  
- [`web/js/features/split/`](../../web/js/features/split/) · [`web/js/features/run/`](../../web/js/features/run/) · [`web/js/features/result/`](../../web/js/features/result/)

---

## 1. 体感拐点

| # | 用户会说 | 波次 |
|---|----------|------|
| **T1** | 「步骤跑完后不会因为中文验收句莫名失败了」 | H0 |
| **T2** | 「CLI 跑完一行大白话：本轮状态 · 完成几步」 | H0 |
| **T3** | 「任何入口同一句：已拆成 6 步等确认 / 第 3 步在跑」 | H1 |
| **T4** | 「怎样算做完仍是人话；高级里才看到可跑检查」 | H2 |
| **T5** | 「并行那几步做完后，能看懂怎么一起验」 | H3 |

---

## 2. 波次总览

```text
H0  止血 + CLI 人话出口 + 回归测     ~1 人日        ← 优先，可单独合
H1  共享 StatusOneLiner（进行中+结束） ~1–1.5 人日
H2  双字段 done_when | verify_cmd     ~2–3 人日      ← 含 SQLite 列迁移/兼容
H3  并行/合并验人话（merge_check）      ~0.5–1 人日
```

依赖：H0 无阻塞；H1 不依赖 H2；H2 依赖 H0 防御语义（避免再写回双用）；H3 可与 H2 后半并行（文案）或串在 H2 后（DTO 字段）。

**推荐合入序**：H0 → H1 → H2 → H3。H0 单独可 ship。

### 2.1 深审补丁（2026-07-24 · 对照代码）

> 结论：**计划方向可继续用**；下列为必须写清的规格债，实施前先认，不必重写波次号。

| 级 | 问题 | 处理 |
|----|------|------|
| **必须** | H0 只改 scheduler 不够「地图」：`to_plan_ir` 仍 `acceptance: done_when.clone()`，Mode B 人话继续进字段；**靠 H0-2 跳过 sh 已止血**，但 acceptance.json/日志仍脏。H0 可接受；**H2 前**建议加 **H0-7** 回归测 + 可选 **H0-2b** convert 短路（人话不写 acceptance）。 | 见 H0-2b / H0-7 |
| **必须** | `system_post` **三处**中文 `acceptance`：inspect（约 L128）· **git-push「有变更则已 commit…」且 `outputs: []`**（L212）· open-pr（L288）。push **没有**路径门禁兜底，H0-2 跳过后 = **无 host 硬验**，只靠 worker 自觉。 | H0-3 写全三处；push/pr 选：显式 shell / 增 outputs / 保持 None+prompt |
| **必须** | SQLite **无** `ALTER` 迁移惯例（`CREATE IF NOT EXISTS` only）。H2 加列须写清：`ensure_column` 模式或 `meta_json.verify_cmd` 过渡，**禁止**假设空库重建。 | H2-1 步骤补迁移 |
| **必须** | H1 `StatusOneLiner` 双源（PlanJob ∥ Run）优先级未写：有 **active run** 时以 run 为准；仅 job 且 `planned` → 等你确认；`planning` → 规划中。 | H1-1 步骤钉死 |
| **必须** | H3 默认句勿写死 **MERGE.md**（仅部分 collab 示例有）。应：「整合产物 / 各步 SUMMARY / 计划验收」，有 integrate 再点名其 outputs。 | H3-1 改文案 |
| **必须** | H0/H2 回归：[`tests/acceptance_and_term.rs`](../../tests/acceptance_and_term.rs)（`exit 1` shell）+ system_post 注入后 fake 跑不得因中文 Failed。 | H0-7 |
| **建议** | H2 blast：`TaskIR.acceptance` 读点含 tick、live verification、report fallback、planner/view、cco_v1 adapter、convert；改名需兼容 serde `acceptance` 旧键。 | H2-1 列兼容表 |
| **建议** | H1-5 进行中写回 **默认砍**（标 ⛔ 或 N/A），避免与 finish writeback 抢、刷库。 | H1-5 |
| **建议** | 概念预算：状态句是 **壳层一条**，不算拆分台第四概念；`verify_cmd`/`merge_check` **禁止**进拆分台首屏三概念。 | §0.3 已有 · H1-3/H3 再钉 |
| **建议** | 工时：H2 含 SQLite+serde+convert+view+web 更接近 **2–3 人日**；H0 含三处 system_post + 测 **~1 人日** 更稳。 | §2 上调 |
| **可接受** | 启发式漏跑真命令（保守正确）；短规则已先写 `verify_cmd`（地图略超前）OK。 | — |
| **可接受** | 与 subjective-desire「计划级验收」正交：本计划 = 任务执行层双字段 + 状态句，不抢 D0 模板五节。 | — |

**不建议**：H0 用复杂 shell AST；H2 上 expected_stdout；H3 做自动 merge；为状态句新建 STATE.md。

---

## 3. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成

### 波次 H0 — 止血 + CLI 人话出口

#### H0-1 · domain：`looks_like_shell_acceptance` 纯函数 ✅

| 项 | 内容 |
|----|------|
| **落点** | 新薄文件优先：`src/domain/plan/verify.rs`（或 `domain/plan/acceptance_kind.rs`）· 单测同文件 |
| **步骤** | 1. 输入 trim 后字符串。 2. **像 shell**（启发式，可保守）：空 → 否；含换行中文叙述且无 `test `/`[`/`cargo `/`npm `/`pnpm `/`yarn `/`make `/`./`/`sh ` 前缀倾向 → 否；以 `test `、`[`、`cargo `、`npm `、`pnpm `、常见 `*.sh`、单行无空格中文长句否。 **宁可漏跑命令，不可误跑人话**。 3. 导出 `is_runnable_verify(s) -> bool`。 4. **禁止** web 复制此函数。 |
| **完成定义** | 单测：中文「存在 VERDICT…」→ false；`test -f MARKER.txt` → true；`system_post` 现用中文句 → false |
| **自测** | `cargo test -p cco looks_like` 或模块测 |
| **依赖** | 无 |

#### H0-2 · runtime：非 shell 跳过 `sh -c` ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/runtime/scheduler/tick.rs`](../../src/runtime/scheduler/tick.rs) `apply_post_done_gates` · 可选写 `acceptance.json` 注明 `skipped_not_shell` |
| **步骤** | 1. `if let Some(cmd) = &task.acceptance` 时先 `is_runnable_verify`。 2. false → 不 spawn shell、不因 acceptance 标 Failed；仍走 `enforce_outputs` / inspect。 3. true → 维持 `run_acceptance_soft`。 4. 日志一行 info：skipped reason。 5. **语义钉死**：skipped ≠ 验收通过；结果台/report 不得把「跳过 shell」写成 PASS。 |
| **完成定义** | 仅人话 acceptance 的任务 Done 后不再变 Failed（除非 outputs/inspect） |
| **自测** | 单测或 integration：fixture 中文 acceptance + 无 outputs → Done；`test -f` 缺文件 → Failed；[`tests/acceptance_and_term.rs`](../../tests/acceptance_and_term.rs) 仍红（`exit 1`） |
| **依赖** | H0-1 |

#### H0-2b ·（建议同 PR）convert 短路：人话不写入 `TaskIR.acceptance` ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/domain/plan/cco_split/convert.rs`](../../src/domain/plan/cco_split/convert.rs) `to_plan_ir` |
| **步骤** | 1. 替换 `acceptance: t.done_when.clone()`：仅当 `done_when`/`verify` 经 `is_runnable_verify` 为 true 时写入 `acceptance`；否则 `acceptance: None`（人话留在 split SoT `done_when`，执行靠 outputs/inspect）。 2. `from_plan_ir` 保持：旧 shell acceptance → done_when 展示可保留短句，但 H2 再拆 verify。 3. 单测：中文 done_when → PlanIR.acceptance is None。 |
| **完成定义** | Mode B confirm 后 resolved 计划不再把中文塞进 acceptance |
| **自测** | domain convert 测；可选与 H2 合并若怕两轮改 convert |
| **依赖** | H0-1 |

#### H0-3 · system_post：三处中文 acceptance 清零或改机器门禁 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/plan/system_post.rs`](../../src/plan/system_post.rs) |
| **步骤** | 1. **三处**现况：inspect ~L128 中文 + 有 outputs；**git-push L212 中文 + outputs 空**；open-pr L288 中文 + outputs 空。 2. 策略（选一写进 PR 说明）：**A（推荐 H0）** 三处 `acceptance: None`，验收入 prompt；inspect 保留 outputs；push/pr 仍靠 worker 约定（与今产品一致，只是去掉假 shell）。 **B** inspect/push 改为显式 `test -f` / `git` 探测命令（须可本地无副作用）。 3. **禁止**只改 inspect 漏掉 push/pr。 4. 人话完成标准留 prompt，H2 后再映射 done_when。 |
| **完成定义** | `rg 'acceptance: Some' src/plan/system_post.rs` 无纯中文；或全为 is_runnable 命令 |
| **自测** | 单测生成 TaskIR；开系统收尾的 fake/目视不因 acceptance 假 Failed |
| **依赖** | H0-1 更佳；可与 H0-2 同 PR |

#### H0-4 · CLI：结束打印人话摘要行 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/cli/commands/common.rs`](../../src/cli/commands/common.rs)（`finish_with_reports` 前后）· 复用 [`report_summary_line`](../../src/report/mod.rs) |
| **步骤** | 1. 在 `status: {:?}` **之上或替代为次要**：先 `println!` 人话 `report_summary_line(&st)`（加载 RunState 后）。 2. Debug 枚举可保留在下行或 `--verbose`。 3. 不改 exit code 语义。 |
| **完成定义** | `cco run` 结束 stdout 可见「本轮状态：**…** · 完成 n/m 项任务」类句子 |
| **自测** | fake provider 短跑；目视 stdout |
| **依赖** | 无（可与 H0-1 并行） |

#### H0-5 · CLI：`cco status` 首行人话 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/cli/commands/status.rs`](../../src/cli/commands/status.rs) |
| **步骤** | 1. 首行：`report_summary_line` 或 H1 暂用同构规则。 2. 其后可保留机读字段。 3. H1 落地后改调 `StatusOneLiner`。 |
| **完成定义** | 非开发打开终端第一行能懂「完成了没 / 进行中」 |
| **自测** | 对已有 run_id 执行 `cco status` |
| **依赖** | H0-4 复用；H1 可替换实现 |

#### H0-6 · 文档指针（本波收口） ✅

| 项 | 内容 |
|----|------|
| **落点** | 本文件状态 · [`docs/CLAUDE.md`](../CLAUDE.md) 已索引 · [`split-product-rules.md`](../split-product-rules.md) 一句「done_when ≠ shell」 |
| **步骤** | 勾选 H0 完成项；短规则补硬句（无第二阶段表）。 |
| **完成定义** | 新人只读短规则 + 本文件可知禁双用 |
| **自测** | 文档审阅 |
| **依赖** | H0-2 |

#### H0-7 · 回归测钉死「人话不炸 / 命令仍炸」 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`tests/acceptance_and_term.rs`](../../tests/acceptance_and_term.rs) 扩展或新测 · domain verify 单测 |
| **步骤** | 1. 保留/确认：`acceptance: "exit 1"` → 任务 Failed + 可有 acceptance.json。 2. 新增：`acceptance: "存在 VERDICT 与 ISSUES"`（或 system_post 原句）→ 任务 **Done**（无 outputs 要求时）。 3. 可选：`done_when` 经 convert 后 PlanIR.acceptance 为空（若做了 H0-2b）。 |
| **完成定义** | CI 红绿稳定表达双语义 |
| **自测** | `cargo test -p cco --test acceptance_and_term`（及 domain） |
| **依赖** | H0-2 |

---

### 波次 H1 — 共享 StatusOneLiner

#### H1-1 · domain/app：`StatusOneLiner` 类型 + 纯投影 ✅

| 项 | 内容 |
|----|------|
| **落点** | **新文件** `src/domain/run/status_line.rs`（纯）+ `src/app/run/status_line.rs` 或 `src/app/status_line.rs`（组 DTO）· **禁止**写入 `live.rs` 正文堆逻辑 |
| **步骤** | 1. 结构建议：`phase`（planning \| await_confirm \| running \| paused \| completed \| failed \| aborted）· `text: String`（唯一主句）· `done`/`total` · `current_title: Option<String>` · `waiting_hint: Option<String>`。 2. **双源优先级（钉死）**：① 存在未结束 RunState（Running/Paused/…）→ **只根据 run** 生成；② 否则若 PlanJob 为 planning →「规划中」；③ planned/待确认 →「已拆成 N 步，等你确认」；④ 结束 run → `report_summary_line` / `compose_last_summary` 同构。 3. 规则模板（无 LLM）；stall 时 text 可附「有步骤好像卡住了」但主句仍含完成比。 4. 单测：仅 job / 仅 run / 二者并存（run 赢）/ 结束态。 |
| **完成定义** | CLI/桌面可调同一函数得到同一 `text` |
| **自测** | domain/app 单测 |
| **依赖** | 无（可与 H0 并行设计；合入建议 H0 后） |

#### H1-2 · live / gateway：暴露 `status_one_liner` ✅

| 项 | 内容 |
|----|------|
| **落点** | 从 `services/live.rs` **抽出**组装调用到 app；`ProjectLiveView`（或等价）增字段 `status_one_liner: String`；Tauri/DTO 透传 |
| **步骤** | 1. 不在 live 内复制规则，只调 H1-1。 2. 若 live 行数仍超硬限，先拆文件再加字段。 3. gateway 已有 `getProjectLive` 则字段自然到前端。 |
| **完成定义** | 桌面 live 轮询带上稳定人话句 |
| **自测** | 桌面运行中顶栏/进度区见句（H1-3） |
| **依赖** | H1-1 |

#### H1-3 · 桌面：固定状态句出口 ✅

| 项 | 内容 |
|----|------|
| **落点** | `web/js/features/run` 进度条旁或 shell 次要条 · 拆分台 meta 可复用 job 态句 · **只绑 DTO 字符串** |
| **步骤** | 1. 主路径显示 `status_one_liner`。 2. 无字段时 fallback 现有五态（不自造业务句）。 3. 不引入第四主概念。 |
| **完成定义** | 用户从聊天/拆分/执行切换时，能看到与 CLI 同构的卡点句 |
| **自测** | 目视：规划中 / 待确认 / 跑第 N 步 / 结束 |
| **依赖** | H1-2 |

#### H1-4 · CLI/TUI：消费同一生成器 ✅

| 项 | 内容 |
|----|------|
| **落点** | `status.rs` · `common.rs` · TUI 状态行（若有简单 status 绘制） |
| **步骤** | 1. 替换 H0 临时 `report_summary_line` 为首选 `StatusOneLiner.text`（结束态可等价）。 2. TUI 观察层只读 app，不写策略。 |
| **完成定义** | 三入口文案一致（允许标点微差，语义同） |
| **自测** | 同 run_id：桌面 live vs `cco status` |
| **依赖** | H1-1 · H0-4/5 |

#### H1-5 · 进行中写回（可选加强） ⛔ 本轮默认不做

| 项 | 内容 |
|----|------|
| **说明** | finish/accept 已有 `writeback_from_run`；进行中刷 `project_last_summary` 易吵、与 H1-1 无强依赖。需要时另开波次。 |
| **状态** | ⛔ 默认不做（勿占 H1 工时） |

---

### 波次 H2 — 双字段 `done_when` \| `verify_cmd`

#### H2-1 · 领域模型：TaskIR / CcoSplitTask 拆字段 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/domain/plan/types.rs`](../../src/domain/plan/types.rs) `TaskIR` · [`src/domain/plan/cco_split/types.rs`](../../src/domain/plan/cco_split/types.rs) · SQLite [`src/state/sqlite.rs`](../../src/state/sqlite.rs) / `cco_split_store.rs` · desk [`plan/planner/view.rs`](../../src/plan/planner/view.rs) |
| **步骤** | 1. 保留 `done_when: Option<String>`（人话）。 2. 新增 `verify_cmd: Option<String>`（仅 shell 一行）。 3. `TaskIR`：新增 `verify_cmd`；**serde 兼容**：保留字段名 `acceptance` 作 **读别名**（`#[serde(alias)]` 或自定义）→ 填入 `verify_cmd` 或 `done_when`（`is_runnable_verify`）；写出时可继续写 `acceptance` 一版以免打碎旧工具，或双写一版后弃。 4. 目标终态：scheduler **只读 `verify_cmd`**（过渡可读 acceptance 若 verify 空且 runnable）。 5. **SQLite 迁移（钉死）**：现库 **无 ALTER 惯例**。采用 `ensure_column(conn, "cco_split_tasks", "verify_cmd", "TEXT")`（`PRAGMA table_info` + `ALTER TABLE … ADD COLUMN`）在 `with_conn`/init 路径执行；旧行 NULL。 **备选**：先只放 `meta_json.verify_cmd` 避免改表——若选备选须在本任务注明并改存储文。 6. **PlanTaskView** 增加 `verify_cmd`（可选序列化）；`done_when` 继续只给人话。 7. 触达清单：tick · convert · cco_v1 adapter · live/report `collect_task_acceptance_items`（task 行用 done_when）· system_post · examples YAML。 |
| **完成定义** | 类型编译 + 金样/单测：人话与命令可同时存在且语义不串；**旧 cco.db 升级不炸** |
| **自测** | convert 往返；serde 旧 `acceptance: "test -f x"` → verify_cmd；临时拷贝用户库 schema 跑 ensure_column |
| **依赖** | H0-1/2 |

#### H2-2 · convert / materialize / planner 写入 ✅

| 项 | 内容 |
|----|------|
| **落点** | `cco_split/convert.rs` · `humanize` · heuristic/split_agent 若写 acceptance |
| **步骤** | 1. **禁止** `acceptance: t.done_when.clone()`。 2. `done_when` → 仅 desk/inspect 叙述。 3. `verify_cmd` ← 显式字段或旧 acceptance 且 `is_runnable_verify`。 4. `collect_task_acceptance_items` / VerificationView 的 task 行优先 `done_when` 文案。 |
| **完成定义** | Mode B 拆出的中文完成定义不再进 shell |
| **自测** | 集成：ai/fast 拆分后 confirm → task 目录无错误 shell acceptance.json |
| **依赖** | H2-1 |

#### H2-3 · scheduler 只跑 `verify_cmd` ✅

| 项 | 内容 |
|----|------|
| **落点** | `tick.rs` · 文档 `runtime/CLAUDE.md` 一行 |
| **步骤** | 1. 门禁顺序：`verify_cmd` shell → `outputs` → inspect。 2. 删除对「人话 acceptance」路径依赖（兼容期可读旧字段仅当 verify 空且 is_runnable）。 |
| **完成定义** | 与 H0 行为一致且字段名正确 |
| **自测** | `examples/plans/with-acceptance.cco.yaml` 仍红/绿符合命令 |
| **依赖** | H2-1 |

#### H2-4 · 桌面：怎样算做完只绑人话；命令折叠 ✅

| 项 | 内容 |
|----|------|
| **落点** | `splitDetail.js` · `splitRender.js` · desk DTO `done_when` / `verify_cmd` |
| **步骤** | 1. 「怎样算做完」= `done_when`（或 summary 回落）。 2. 若有 `verify_cmd`，高级/折叠「自动检查」显示命令，**不进第一句**。 3. 不在 JS 猜 shell。 |
| **完成定义** | 甲受众只见人话；乙可展开见命令 |
| **自测** | 目视双受众 |
| **依赖** | H2-1 · planner view DTO |

#### H2-5 · 存储文 / 短规则字段表更新 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`cco-split-format-sqlite-2026-07-21.md`](../cco-split-format-sqlite-2026-07-21.md) §1.1 字段表加 `verify_cmd` · [`split-product-rules.md`](../split-product-rules.md) schema 行 |
| **步骤** | 1. 字段用途一行。 2. **勾选仍只在本计划 H2**，不把 H2 任务复制进 S2–S6。 3. materialize 映射写清。 |
| **完成定义** | 地图与地形同构 |
| **自测** | 文档审阅 |
| **依赖** | H2-1 合入时 |

#### H2-6 ·（可选）expected 结果 ☐ → 默认 **本轮不做**

| 项 | 内容 |
|----|------|
| **说明** | claude-flow 的 expected stdout 匹配。本轮 **仅 exit code + outputs + inspect**。若未来做：`verify_expect: Option<String>` 独立波次，不在 H2 扩 scope。 |
| **状态** | ⛔ 本计划明确不做 |

---

### 波次 H3 — 并行 / 合并验人话

#### H3-1 · app DTO：`merge_check` 一句 ✅

| 项 | 内容 |
|----|------|
| **落点** | desk/run/result DTO · domain 可按 `role=integrate\|inspect` 或 wave 末任务给默认文案 |
| **步骤** | 1. **通用默认句**（无文件名承诺）：「可以一起干的步骤都完成后，再对照各步说明与计划验收；有一步失败，先别当全部成功」。 2. **有 integrate 任务时**：「拼在一起怎么验：先看整合步骤的产出（见该步说明/outputs），再跑巡检对照计划」——**仅当** outputs 含具体路径时才点名路径；**禁止**默认写死 `MERGE.md`（除非本 run 图里真有该 output）。 3. 无 integrate 时结果台仍用 P2-1「原计划要验收」折叠，不假装有合并步。 4. soft_accept 串行提示见 H3-3（`soften` notes 已有英文 serialize 日志 → 投影中文）。 |
| **完成定义** | 并行波次或 inspect/integrate 旁可见浅白一句，且不虚构产物路径 |
| **自测** | 普通计划 + `examples/plans/mixed-claude-codex-inspect.cco.yaml` 各目视一次 |
| **依赖** | 无强依赖；建议 H1 后 |

#### H3-2 · handoff / inspect 前缀死话术（浅） ✅

| 项 | 内容 |
|----|------|
| **落点** | `runtime/handoff/prefix.rs` 或 runtime-prompts 中 inspect/integrate 段 |
| **步骤** | 1. 固定 3 条：先读各 SUMMARY；失败任务勿装成功；合并后整图对照计划。 2. 不引入自动 merge。 3. 文案 ≤ 概念预算。 |
| **完成定义** | worker 提示含浅白纪律；用户结果台与之一致 |
| **自测** | 读生成 prefix 快照测（若有）或目视 |
| **依赖** | H3-1 更佳 |

#### H3-3 · soft_accept 串行时的人话提示 ✅

| 项 | 内容 |
|----|------|
| **落点** | split desk critic/notes 或 job_view 提示条 |
| **步骤** | 1. 当 soft_accept 因 scope 重叠改串行：提示「为避免改同一处，已改为排队」。 2. 不 silent。 |
| **完成定义** | 用户理解为何没并行 |
| **自测** | 构造重叠 scope 拆分 |
| **依赖** | 现有 soft_accept |

#### H3-4 · 文档：与 multi-cli 对齐「无自动 merge」 ✅

| 项 | 内容 |
|----|------|
| **落点** | 本文件完成勾选 · multi-cli 文首可选互链一句 |
| **完成定义** | 无「cco 会自动合 git」误解 |
| **依赖** | H3-1 |

---

## 4. 推荐 DTO 形状（实施认这个）

### 4.1 任务双层

```text
CcoSplitTask / 展示
  done_when: Option<String>     // 人话「怎样算做完」· 永不 sh -c
  verify_cmd: Option<String>    // 可选 shell 一行 · 仅此进 run_acceptance

TaskIR（执行）
  verify_cmd: Option<String>    // scheduler 唯一 shell 验收源（终态）
  acceptance: 过渡兼容读旧 YAML → 分流
  outputs: Vec<String>          // 路径门禁（已有）
  role / require_inspect        // 巡检终闸（已有）
```

### 4.2 状态一句

```text
StatusOneLiner {
  phase: enum,
  text: String,                 // 唯一主路径人话
  done: u32,
  total: u32,
  current_title: Option<String>,
  waiting_hint: Option<String>,
}
```

### 4.3 合并验

```text
// 任务或 run 级可选
merge_check: Option<String>     // 浅白一句；默认规则生成，可空
```

### 4.4 展示规则

| 表面 | 显示 |
|------|------|
| 拆分台主路径 | title · summary · done_when · 依赖 |
| 拆分台高级 | verify · verify_cmd · provider |
| 执行中 | StatusOneLiner.text · 五态 badge |
| 结果台 | 对照计划 · 原计划要验收 · merge_check · 费用 |
| CLI | 首行 text；report.md 不变 |

---

## 5. 成功标准（本计划自身）

| # | 指标 | 验收 |
|---|------|------|
| S1 | 中文人话不再触发 shell 失败 | system_post + Mode B done_when 场景绿 |
| S2 | CLI 结束/status 首行人话 | 非开发可读 |
| S3 | 桌面与 CLI 卡点句同构 | 同 run 对比 |
| S4 | 双字段可同时存在 | 人话 + `test -f` 各司其职 |
| S5 | 主路径无命令第一句 | 拆分台/结果台目视 |
| S6 | 无自动 merge、无 STATE.md 真源 | 代码与文档审阅 |
| S7 | 体积门禁 | 新逻辑在薄文件；live 不继续胀 |

---

## 6. 风险与回滚

| 风险 | 缓解 |
|------|------|
| 启发式漏掉真命令 | H0 保守；H2 显式 `verify_cmd`；YAML 示例改文档 |
| 旧计划只写了 acceptance 人话 | 分流到 done_when，行为=今天修好后 |
| 旧计划只写了 acceptance 命令 | 分流到 verify_cmd，行为保持 |
| live 再超限 | H1 强制新文件 |
| 产品以为 exit 0 = 完成 | 文案与 P2-1 诚实句保留 |
| git-push 无 outputs，去 shell 后更「软」 | H0-3 接受 A 或补探测命令；产品本就靠 worker |
| 旧 SQLite 无列 | H2-1 `ensure_column`，禁止只改 CREATE |
| H3 写死 MERGE.md | H3-1 禁止；按 outputs 点名 |
| skipped shell 被 UI 当成已验收 | H0-2 语义钉死；P2-1 诚实句 |

回滚：H0 可单 PR 回滚 scheduler 判断；H2 schema 需兼容列 default null + ensure_column 可留列无害。

---

## 7. 明确不做（再钉一次）

- expected_stdout 匹配（本轮）  
- 计划 checklist 自动执行  
- STATE.md 落盘争真源  
- host 自动 git merge/PR  
- 前端策略复制  
- 新开跑入口  

---

## 8. 修订

| 日期 | 说明 |
|------|------|
| 2026-07-24 | 首版：论坛 ①③④ 裁决 → H0–H3 勾选真源；索引进 docs/CLAUDE.md |
| 2026-07-24 | **深审补丁 §2.1**：H0-2b/H0-7；system_post 三处；SQLite ensure_column；StatusOneLiner 双源优先级；H3 禁写死 MERGE.md；H1-5 默认不做；工时上调 |
| 2026-07-24 | **H0 ✅**：`domain/plan/verify` · tick 跳过非 shell · convert 短路 · system_post 三处 acceptance None · CLI finish/status 人话行 · acceptance_and_term 回归 |
| 2026-07-24 | **H1 ✅**：`domain/run/status_line` + `app/run/status_line` · live `status_one_liner`（`live_status` 薄模块）· 桌面 `#status-one-liner` · CLI/TUI 共用 · H1-5 仍 ⛔ |
| 2026-07-24 | **H2 ✅**：`verify_cmd` 字段 · SQLite `ensure_column` · convert 双轨 · `effective_verify_cmd` · tick 只跑 shell · desk 高级折叠 · H2-6 仍 ⛔ |
| 2026-07-24 | **H3 ✅**：`merge_check` domain+live+结果台 · soft_accept 中文排队 · handoff integrate/inspect 纪律 · multi-cli 互链无自动 merge |
| 2026-07-24 | **核验收口**：合入 `e6e1ddb`（H0–H3）+ `bb30704`（金样 `grain_hint`/`effort`）；`cargo test` domain verify/status_line/merge_check/cco_split · `acceptance_and_term` · `a0_behavior_golden` · `mode_b_golden` 全绿；H1-5 / H2-6 仍 ⛔ |
| 2026-07-24 | **归档**：`git mv` → [`docs/archive/`](./) · 索引 docs/CLAUDE + L1 + archive/README；交叉链 multi-cli / split-product-rules / cco-split 改 archive 路径；**行为规则**仍在短规则与字段表 |

---

## 9. 实施提示（Agent · 历史）

1. **先 H0 再 H2**，避免未止血就扩 schema。  
2. 每波结束：相关 L2 头部 PROTOCOL 一行 + 本表勾选。  
3. 测：`cargo test` 相关模块 + 可选 `examples/plans/with-acceptance.cco.yaml`。  
4. 桌面：`split` / `run` / `result` 只改展示绑定，不写策略。  
5. 合入信息建议：`feat(status): H0 shell-skip + CLI human summary` 等按波次拆 PR。
