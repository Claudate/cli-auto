# 多窗口可并发拆分 · 落地实施计划

> 日期：2026-07-22  
> 角色：**实施勾选真源**（派工 / PR 边界 / 完成定义）  
> 读前背景（决策，不重复勾选）：[`split-quality-work-style-2026-07-22.md`](./split-quality-work-style-2026-07-22.md)  
> 展示层（他窗可并行）：[`split-desk-dual-audience-landing-2026-07-22.md`](./split-desk-dual-audience-landing-2026-07-22.md)  
> 壳层样例计划：[`shell-chrome-simplify-2026-07-22.md`](./shell-chrome-simplify-2026-07-22.md)  
> Agent 既有：[`openhands-style-split-agent-landing-2026-07-21.md`](./openhands-style-split-agent-landing-2026-07-21.md) · [`src/plan/split_agent/`](../src/plan/split_agent/)  
> 产品：[`../PRODUCT.md`](../PRODUCT.md) · 架构：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)  
> 范围：拆分 **生产质量**（prompt · parse · soft_accept · 金样 · 来源可见 · 可选 work_style 旋钮）  
> 状态：**W1–W4 ✅ 收口**（2026-07-22 · 含 W4-2 · 与 dual-audience / shell-chrome 窗口 C 汇合；本机 §6 目视仍建议）

[PROTOCOL]: **勾选只认本文件 §4**。决策叙事听 work-style 文档；展示听双受众；**禁止**平行第二套「领域中台」。不旁路 `confirm_start`。落地后回写 `docs/CLAUDE.md` + `src/plan/CLAUDE.md` 一行。

---

## 0. 大白话：我们要实现什么

用户拿一份落地计划（例如 shell-chrome），点「拆成步骤」后，应得到：

```text
不是：瞬间出现 10 个标题，全排队
而是：
  · 每一步是一张「工单」（做什么 / 改哪些文件 / 怎样算完 / 先等谁）
  · 不抢同一文件的步骤可以同一波一起跑（多窗口 / 多 worker）
  · 真有先后的才排队
  · 人在拆分台看懂后点「执行规划」才开跑
```

**实现路径（一句话）：**  
修好 **专用拆分 Agent 的输入输出**（提示词 + 解析 scope + 软校验）→ 用 **金样**锁质量 → 拆分台 **标明是否真智能拆** → 执行侧已有 Scheduler/worktree 接着跑。

---

## 1. 目标 / 非目标

### 1.1 目标

| # | 目标 | 用户可感知 |
|---|------|------------|
| G1 | 默认走 **智能拆分**（`plan_mode=ai`） | 拆分要等一会儿；不是毫秒假完成 |
| G2 | 每条任务带 **文件地界** `scope_paths` | 执行少撞车；高级/乙可读「改哪」 |
| G3 | 每条 **body** 是可派工提示词 | 开窗口就能干，不用再读整份计划 |
| G4 | **depends_on 只连真依赖**；可并行的在同一波 | 拆分台波次数 ≈ 可同时干的批 |
| G5 | 来源常显：智能 / 本地规则 | 不再误以为假拆是真拆 |
| G6 | （可选薄）工作习惯只调粗细/并行建议，**不**把默认打回 fast | 见 §4 波次 W4 |

### 1.2 非目标

| 不做 | 原因 |
|------|------|
| 换 LangGraph / CrewAI / 新前端栈 | 架构已收口 |
| 开机领域问卷 | 方案 C；work-style 文档 |
| 旁路 confirm / 改 Mode B | L1 |
| 重写 Scheduler 内核 | 执行并行已有；本轮补 **拆分质量** |
| 用 heuristic 当主路径 | 仅兜底 + 显式 fast |
| 与双受众 S0 抢勾选 | 展示归双受众文档 |
| 一次做完「从聊天猜习惯」 | W4 后置 |

### 1.3 硬契约

1. 开跑只经 `confirm_start` / `gateway.confirmStart`。  
2. optional 仍可勾选；禁止静默跳过。  
3. IPC 只经 gateway；策略在 Rust Application / domain。  
4. 文件软 400 / 硬 600；prompt 字符串可集中在 `split_agent/prompt.rs`（若胀则拆 `prompt_rules.rs`）。  
5. 图标/文案主路径人话；`VERDICT` / adapter 名不进第一句（来源用「智能拆分/本地规则」）。

---

## 2. 现状盘点（实现前必读）

### 2.1 已有（勿重做）

| 能力 | 位置 |
|------|------|
| `plan_mode=ai` → ModelSplitAgent → cco-split/v1 | `src/plan/split_agent/` · `planner/job.rs` |
| soft_accept / waves / sanitize deps | `domain/plan/cco_split/` |
| SQLite SoT | `state/cco_split_store` |
| 拆分台三栏 + confirm | `web/features/split` · `app/split` |
| 执行并行 + worktree 门 | Scheduler · `domain/plan/soften` · validate |
| **桌面默认 ai**（Q0） | `web/index.html` `#pp-plan-mode` · `jobPoll.js` `\|\| "ai"` |
| CLI 默认 ai | `src/cli/mod.rs` |

### 2.2 缺口（本文件要补）→ **W1–W4 主项已收**（下表为历史证据，勿当新缺口）

| 缺口（历史） | 落地（2026-07-22） |
|--------------|-------------------|
| Agent prompt 禁 scope / body 无模板 | `split_agent/prompt.rs` 要求 scope + 六段 body |
| parse 写死 `scope_paths: vec![]` | `parse.rs` 映射 + `normalize_scope_paths` |
| 同文件并行 | `soft_accept` `serialize_scope_overlaps` |
| 无金样 | `tests/fixtures/cco_split/shell_chrome_sample.json` ≥6 步 |
| 来源不醒目 | `splitFillMeta` 常显 + 本地「用智能再拆」 |
| work_style 未产品化 | `workStyle.js`（**planMode 固定 ai**；W4-1/W4-2 ✅ 薄落地） |

### 2.3 数据流（改完后）

```text
用户点「拆成步骤」
  → plan_mode 默认 ai（已 ✅）
  → ModelSplitAgent
       system/user prompt（W1：工单规则 + 要求 scope + body 模板）
       → JSON cco-split/v1
  → parse（W1：读入 scope_paths / can_parallel）
  → soft_accept（W2：重叠 scope 不并行；空 body 填；禁 worker 首行可加强）
  → SQLite + desk DTO
  → 拆分台（双受众 S0 展示；W3 来源条）
  → 人确认 → materialize PlanIR → Scheduler（多 worker / worktree）
```

---

## 3. 已完成项（勿重复开工）

| ID | 内容 | 状态 |
|----|------|------|
| **Q0** | 桌面默认 `plan_mode=ai`；fast 去掉「推荐」 | **✅** `web/index.html` · `jobPoll.js` |

验证：选计划 → 拆成步骤 → `~/.cco/plan_jobs/最新/planner.log` **不得**首行 `using fast local splitter`（除非用户显式选快速拆分）。

---

## 4. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成  
> 建议 PR 切片：W1 → W2 → W3；W4 可选另 PR。

---

### 波次 W1 — Agent 会「派工」（核心）✅

**目标：** 模型输出的每条任务 = 可派给一个窗口的工单；`scope_paths` 进库。

#### W1-1 · 重写拆分 system / user prompt ✅

- **文件：** [`src/plan/split_agent/prompt.rs`](../src/plan/split_agent/prompt.rs)（胀则抽 `prompt_rules.rs`）  
- **步骤：**  
  1. **删除**「不要输出 scope」类禁令。  
  2. **要求**每条任务含：  
     - `scope_paths`: 字符串数组（仓库相对路径或目录）；纯文案任务用 `[]` 并在 body 写「无代码路径」。  
     - `body` 固定小节（可用中文标签）：  
       ```text
       【做什么】
       【改哪里】
       【怎样算做完】
       【先等谁】无则写「无」
       【不要做什么】
       【自测】
       ```  
     - `done_when`: 一句可观察完成标准（可与「怎样算做完」同义压缩）。  
     - `depends_on`: **仅**真先后；禁止为凑波次串线。  
     - `can_parallel`: 与同波兄弟无硬依赖且 scope 不重叠时 true。  
  3. **禁止**拆成任务：非目标、PROTOCOL、修订历史、纯目录/索引、空话。  
  4. **禁止** body 以「你是执行…worker」开头。  
  5. user_prompt 增加：`max_parallel`、一句「并行单位=文件所有权，同文件写者不得并行」。  
  6. 若计划正文含「文件 / 完成定义 / 依赖」表，指示 **优先信表**，勿发明冲突依赖。  
- **完成定义：** 对 fixture 计划跑一次（或单测注入假模型 JSON）能解析出带 scope 的任务；人工读 body 像工单。  
- **自测：** `cargo test -p cco split_agent`（及本波新增测）。  
- **依赖：** 无（Q0 已 ✅）。

#### W1-2 · parse 接入 scope_paths（修写死空数组）✅

- **文件：** [`src/plan/split_agent/parse.rs`](../src/plan/split_agent/parse.rs) · 必要时 [`types.rs`](../src/domain/plan/cco_split/types.rs) 已有字段无需改  
- **步骤：**  
  1. `AgentTask` 增加 `scope_paths: Vec<String>`（serde default 空）。  
  2. 映射到 `CcoSplitTask.scope_paths`（**禁止**再 `vec![]` 写死）。  
  3. 规范化：trim、去空、去重；非法路径字符可保留但 strip `..` 前缀危险段（保守：只 trim + 拒绝对盘符，细节实现自定）。  
  4. `can_parallel` 已入 meta 则保留；若后续 soft_accept 需要，可升为字段或继续 meta。  
- **完成定义：** 解析含 `"scope_paths":["web/index.html"]` 的 JSON 后，job 内 task.scope_paths 非空。  
- **自测：** parse 单测扩展（现有 `parse.rs` tests）。  
- **依赖：** W1-1 可并行写测；合并前两者都绿。

#### W1-3 · convert → PlanIR 带上 scope（执行侧能看见）✅

- **文件：** [`src/domain/plan/cco_split/convert.rs`](../src/domain/plan/cco_split/convert.rs) · 查 `TaskIR.scope` / soft-fill  
- **步骤：**  
  1. `to_plan_ir`：`scope_paths` → 现有 `TaskScope` / paths 字段（与 multi-cli 路由一致）。  
  2. 确认 desk DTO / job_view 已透出 paths（乙技术说明用；甲默认可不展示）。  
  3. 若 convert 丢 scope，补测。  
- **完成定义：** confirm 后 materialize 的任务带 scope；不改变 confirm 语义。  
- **依赖：** W1-2。

#### W1-4 · 金样 fixture：shell-chrome 期望拆分 ✅

- **文件：** 建议 `tests/fixtures/split/shell-chrome-simplify.expected.json`（或 `src/plan/split_agent/testdata/`）  
- **内容：** 手写「理想工单图」子集（不必 14 条全满，**至少 6 条**覆盖 A 并行 + 一处真依赖 a3→a4）：  

| id | title（示意） | depends_on | scope_paths（示意） |
|----|---------------|------------|---------------------|
| a1 | 去掉顶栏阶段条 | [] | `web/index.html`, `web/js/features/project/shellChrome.js` |
| a2 | 拆分台两键 CTA | [] | `web/index.html`, `web/js/features/split/splitDetail.js` |
| a3 | 去掉编辑任务 | [] | `web/index.html`, `web/js/features/project/projectPicker.js` |
| a4 | 顶栏三键 icon | [a3] | 同上顶栏按钮区 |
| a5 | 完整说明默认展开 | [] | `web/js/features/split/splitDetail.js` |
| b1 | 侧栏移除项目 | [] | `web/js/shared/shellUi.js`, `web/js/features/project/projectCrud.js` |

- **步骤：**  
  1. fixture JSON 合法 cco-split/v1。  
  2. 单测：`parse_agent_output(fixture)` soft_accept 后：  
     - a1/a2/a5/b1 无互依赖（或 depends 空）  
     - a4 depends 含 a3  
     - 各 scope 非空（b1 与 a1 不强制重叠检测在 W2）  
  3. 可选：`CCO_SPLIT_AGENT_FIXTURE` 指向此文件做桌面目视。  
- **完成定义：** CI 测锁结构；改 prompt 不小心丢掉 scope 会红。  
- **依赖：** W1-2。

---

### 波次 W2 — 并行要「敢并且不撞」✅

**目标：** soft_accept / sanitize 把「同文件并行」压成串行；信计划依赖列。

#### W2-1 · scope 重叠 → 不可并行 ✅

- **文件：** [`src/domain/plan/cco_split/accept.rs`](../src/domain/plan/cco_split/accept.rs) · 或 `soften` 路径与 PlanIR 对齐的 [`domain/plan/soften.rs`](../src/domain/plan/soften.rs)  
- **步骤：**  
  1. 对无 depends、且 `scope_paths` 路径前缀重叠的任务对：加边或强制不同 wave（策略选一种，文档化）。  
  2. 目录 vs 文件：`web/js/features/split/` 与 `splitDetail.js` 视为重叠。  
  3. 两边 scope 皆空：不因此串行（文案任务可并行）。  
  4. notes 写入 soft_accept 说明（供 log，主路径可不展示技术细节）。  
- **完成定义：** 单测两任务同改 `index.html`、无 depends → 接受后不会同 wave 并行。  
- **依赖：** W1-2。

#### W2-2 · 与双受众 S2 对齐：信计划「依赖：无」✅

- **文件：** heuristic 已有 S2 方向（`src/plan/CLAUDE.md`）；Agent 路径靠 prompt；sanitize 假边  
- **步骤：**  
  1. 确认 `sanitize_cco_split_deps` / accept 假边规则：body 无「依赖原因/等待」且计划写无依赖时不保留幽灵边。  
  2. 不与双受众文档双轨勾选：若 S2 已合入则本项 ✅ 引用即可。  
- **完成定义：** 有「依赖：无」的包不被强行 t1→t2→t3。  
- **依赖：** 可与 W2-1 同 PR。

#### W2-3 · body / summary 去工人腔（执行提示词）✅

- **文件：** [`humanize.rs`](../src/domain/plan/cco_split/humanize.rs) · convert 写 PlanIR.prompt 处  
- **步骤：**  
  1. strip「你是执行任务…worker」脚手架（展示 S0 可能已做则复用函数）。  
  2. convert 时 prompt = body（已 humanize），勿再包一层 worker 设定。  
- **完成定义：** 执行 worker 收到的 prompt 第一行是【做什么】或人话标题，不是 worker 宣言。  
- **依赖：** 可与 W1 并行；合入前与双受众不冲突。

---

### 波次 W3 — 用户知道「这是真拆」✅

#### W3-1 · 拆分台结果摘要第一行：来源 ✅

- **文件：** [`web/js/features/split/splitFillMeta.js`](../web/js/features/split/splitFillMeta.js) · [`flow.js`](../web/js/flow.js) 已有「本地规则拆分/智能拆分」  
- **步骤：**  
  1. 摘要条常显：`智能拆分` 或 `本地规则拆分（未调用模型）`。  
  2. 本地时副文案可点「用智能再拆一次」（走现有 replan，确保 `#pp-plan-mode` 为 ai 或临时传 ai）。  
  3. 禁止只把来源藏在折叠 critic 芯片。  
- **完成定义：** fast 拆完第一眼知道没走模型；ai 拆完知道走了。  
- **依赖：** Q0 ✅。  
- **与双受众：** 文案人话；不暴露 adapter 内部名。

#### W3-2 · 规划中文案 ✅

- **文件：** `jobPoll.js` planning-sub · flowPlanningSub  
- **步骤：** ai 时：「正在智能拆分（会想依赖与并行，可能要几分钟）…」；fast 时标明本地。  
- **完成定义：** 等待时不觉得卡住无解释。  
- **依赖：** 无。

#### W3-3 · 文档回写 ✅

- **文件：** 本文件勾选 · [`docs/CLAUDE.md`](./CLAUDE.md) · [`src/plan/CLAUDE.md`](../src/plan/CLAUDE.md) · 修正 openhands 文档里「桌面默认 fast」过时句（可一行）  
- **依赖：** W1–W3 主项 ✅ 后。

---

### 波次 W4 — 工作习惯四选一（可选 · 薄）✅

> **不挡 W1–W3。** 决策真源：work-style 文档方案 C。

#### W4-1 · 存储 + 一次可跳过 UI ✅

- **文件：** `web/js/shared/workStyle.js`（若他窗已建则衔接）· 设置页一行 · 首次拆分前 modal（可跳过）  
- **映射 SplitPrefs：**  
  - copy_density → 双受众密度（不改调度）  
  - grain → 仅影响 **提示词**「偏粗/偏细」一句（W1 prompt 读取可选）  
  - parallel → **建议** max_parallel 种子（jobPoll 可已有）  
  - **planner 固定默认 ai**；禁止 profile 默认 fast  
- **完成定义：** 跳过=①；设置可改；不挡主路径。  
- **依赖：** 建议 W3 后；S0 展示稳定更佳。

#### W4-2 · 项目级覆盖 ✅（薄 · localStorage）

- **文件：** [`web/js/shared/workStyle.js`](../web/js/shared/workStyle.js) · settings 注入行 · jobPoll 传 `selectedPath`  
- **步骤：**  
  1. `cco.workStyle.byProject` map：项目 path → style id。  
  2. `resolvedWorkStyle(project)` / grain / max_parallel 读项目优先。  
  3. 设置页勾选「仅当前项目」保存覆盖；可清除。  
  4. **仍禁止** profile 把 plan_mode 改 fast。  
- **完成定义：** 同一全局习惯下，项目 A 可设 eng 覆盖；切项目 B 回全局。  
- **依赖：** W4-1。

---

## 5. 按波次的「实现清单」（给写代码的人）

### W1 最小 diff 集

```text
src/plan/split_agent/prompt.rs      # 规则重写
src/plan/split_agent/parse.rs       # scope_paths 接入 + 测
src/domain/plan/cco_split/convert.rs  # 若丢 scope 则补
tests/fixtures/split/...            # 金样
```

### W2 最小 diff 集

```text
src/domain/plan/cco_split/accept.rs  # scope 重叠
src/domain/plan/cco_split/humanize.rs / convert.rs
```

### W3 最小 diff 集

```text
web/js/features/split/splitFillMeta.js
web/js/flow.js 或 jobPoll.js
docs/*
```

---

## 6. 验收脚本（非开发 + 开发各一条）

### 6.1 产品验收（桌面）

1. 刷新 App；更多选项确认默认是 **智能拆分**。  
2. 选 `docs/shell-chrome-simplify-2026-07-22.md` → 拆成步骤。  
3. **应等待**（不是瞬间）；规划文案提到智能拆分。  
4. 拆分台：  
   - 第一行来源 = 智能拆分（W3 后）。  
   - 打开任一步：有「怎样算做完」；body 不像 worker 宣言（W2/双受众）。  
   - 至少两步显示可同一波或 depends 空且不同文件（W1/W2 后）。  
5. 更多选项改 **快速拆分** 再拆：瞬间完成 + 来源=本地规则 + 可点智能再拆。  
6. 点执行规划仍走 confirm，optional 行为不变。

### 6.2 工程验收

```bash
cargo test -p cco -- split_agent
cargo test -p cco -- cco_split
# 有 arch 门禁时
STRICT=0 ./scripts/check-arch.sh
```

- parse 测：scope 非空  
- accept 测：同文件不并行  
- fixture 测：a4 depends a3  

### 6.3 质量度量（抽检，可写 PR 描述）

| 指标 | 目标（shell-chrome 手拆对照） |
|------|------------------------------|
| 来源 | ai 且非 heuristic（失败兜底要标明） |
| scope 覆盖率 | ≥ 80% 代码任务有路径 |
| 假全串行 | 无依赖表时不应 10 波各 1 任务纯链（除非 scope 全撞） |
| body 工人腔 | 0 条以 worker 句开头 |

---

## 7. 风险与回滚

| 风险 | 缓解 |
|------|------|
| ai 慢 / 超时 | 保留 zombie reap、取消、文案；失败 fallback heuristic **必须** W3 标明 |
| 模型乱写 scope | soft_accept 重叠串行；人可在拆分台改（高级） |
| prompt 变长超上下文 | 计划过大时截断策略已有则复用；任务数 3–12 硬提示 |
| 与双受众 PR 冲突 | W3 文案协调；humanize 复用同一 strip 函数 |
| W4 习惯把 planner 改 fast | **禁止**；code review 门禁 |

回滚：按波次 revert；Q0 回滚会恢复「假拆」体感，不推荐。

---

## 8. 建议提交切片

```text
1) fix(web): default plan_mode=ai          # 已做 Q0
2) feat(split-agent): scope+body 工单 prompt + parse
3) feat(cco_split): soft_accept scope overlap + fixture
4) feat(web): split desk source line + replan ai CTA
5) docs: multi-window-split-landing 勾选 + L2
6) (opt) feat(web): work_style 四选一
```

---

## 9. 和「思路」的一一对应（防忘）

| 思路 | 落在哪一波 |
|------|------------|
| Plan ≠ Code，专用拆分 | 已有 Agent；W1 提高工单质量 |
| 任务图 + 真依赖 | W1 prompt + W2 sanitize |
| 并行单位 = 文件地界 | W1 scope + W2 重叠检测 |
| 工单提示词 | W1 body 模板 |
| 人闸 | 已有 confirm；不改 |
| 隔离执行 | 已有 worktree；本轮不重写 |
| 别标题刮表 | Q0 默认 ai + W3 来源 |
| 工作习惯 | W4 可选 |

---

## 10. 状态总表

| 波次 | 内容 | 状态 |
|------|------|------|
| Q0 | 默认 ai | **✅** |
| W1 | prompt + parse scope + convert + 金样 | **✅** |
| W2 | 重叠不并行 + 假边 + 去工人腔 | **✅** |
| W3 | 来源常显 + 等待文案 + 文档 | **✅** |
| W4 | 工作习惯四选一（可选） | **✅**（含 W4-2 项目级薄覆盖） |

**窗口 A 加固（2026-07-22）：**  
- parse `normalize_scope_paths` + **extract.rs** 拆出 stream-json（parse 硬上限）  
- convert / soft_accept `strip_worker_scaffold`（库内 body + 执行 prompt）  
- soft_accept scope 重叠（含目录∩文件）不同 wave · shell-chrome 金样 ≥6  
- heuristic 去工人腔脚手架 · workStyle `planMode` 全 **ai**（禁 profile 默认 fast）  
- W3：本地来源「用智能再拆一次」→ `state.forcePlanModeAi` + CSS · replan  
- W4-1 grain：`workStyle.grainHint` → `start_plan_job.grain_hint` → user_prompt「粒度偏好」  
- W4-2：`cco.workStyle.byProject` 项目覆盖（设置页勾选 · jobPoll 读 path）  
- **窗口 C 汇合（2026-07-22）：** 与 shell-chrome 对齐 CTA「重新规划/执行规划」· 完整说明不强制 open · 来源条 + 智能再拆 · confirm 未旁路 · 主路径无「重新拆分」主按钮文案  
- 仍建议：桌面 §6 目视
