# 拆分质量 · 多窗口并发 · 工作习惯预设 · 决策与补正

> 日期：2026-07-22  
> 角色：**决策 / 分析补正**（不是第二套双受众勾选表）  
> 触发：  
> 1）用户要把 [`shell-chrome-simplify-2026-07-22.md`](./shell-chrome-simplify-2026-07-22.md) 拆成**可多窗口并发、有序依赖的执行提示词**（把人工 tech-lead 派工自动化）  
> 2）用户纠正：桌面默认 `fast` 不合理（像假拆）  
> 3）用户补正「画像」形态：不要开机选领域；要**可跳过的工作习惯四选一**  
> 关联（**不继承其勾选、不平行抢做**）：  
> - 展示层真源：[`split-desk-dual-audience-landing-2026-07-22.md`](./split-desk-dual-audience-landing-2026-07-22.md)（S0–S3 · 另一窗可在执行）  
> - 壳层减法：[`shell-chrome-simplify-2026-07-22.md`](./shell-chrome-simplify-2026-07-22.md)  
> - 拆分 Agent：[`openhands-style-split-agent-landing-2026-07-21.md`](./openhands-style-split-agent-landing-2026-07-21.md) · [`split-agent-model-path-2026-07-21.md`](./split-agent-model-path-2026-07-21.md)  
> 产品：[`../PRODUCT.md`](../PRODUCT.md)  
> 状态：**决策 ✅ · Q0–Q5 ✅（Q6 部分）· 2026-07-22 收口**（见 §8）

[PROTOCOL]: 本文是**决策与质量真源**。勾选实施时另开薄 PR / 挂进既有计划，**禁止**再写平行「领域中台」阶段表。展示层勾选听双受众文档；默认 `plan_mode` 听 §3。

---

## 0. 一句话

**好拆分 = 专用 Planner 产出「有依赖的可验收工作包 + 非重叠文件所有权 + 给执行 AI 的完整 body」→ 人确认 → 多窗口/worktree Worker 并行；默认必须走智能拆分，不能默认真标题刮表。**

个性化用**一次可跳过的工作习惯**（4 旋钮），不用开机领域问卷。  
展示用**双受众同屏分层**（甲默认 / 乙可进），工作习惯只调默认旋钮，不替代 S0 人话。

---

## 1. 你要的「人工 tech-lead」到底在干什么

把 `shell-chrome-simplify` 这类落地计划交给多个窗口跑时，人会做这些，而不是「按 `####` 标题抄成串行表」：

| 人工步骤 | 产出 |
|----------|------|
| 读目标 / 非目标 / 硬契约 | 哪些永远不能做（旁路 confirm、删磁盘…）写进每条 worker 约束 |
| 按**文件所有权**切块 | A1 顶栏条 / A2 CTA / B1 侧栏 / C1 chip… 尽量不抢同一文件 |
| 标**真依赖** vs 假顺序 | A3→A4 有序；A1∥A5 可并行；D 等 A 过 |
| 写**给执行者的提示词** | 改哪些文件、完成定义、自测、禁止事项 |
| 定**并发上限与隔离** | max_parallel + worktree，避免同文件打架 |
| 给人看一眼再开跑 | = cco 拆分台 confirm |

cco 要自动化的是**整条链**，不是其中「列出标题」一步。

---

## 2. 为什么现在体感「假」

| 事实 | 含义 |
|------|------|
| 桌面默认 `plan_mode=fast`（`#pp-plan-mode` selected + jobPoll `\|\| "fast"`） | **不调模型**，本地 heuristic |
| `planner.log`: `using fast local splitter (heuristic; no LLM)` | 毫秒级「成功」 |
| heuristic 见 `####` 就抄包 | 标题像真规划，依赖常被串成一条链 |
| 拆分 Agent prompt 明确禁止输出 scope | 并发缺**文件所有权**，执行期易撞车 |
| body 常被 scaffold 成「你是 worker」 | 展示层双受众文档已点名；与拆分质量叠加劝退 |

CLI 默认已是 `ai`；桌面反而 `fast` —— 产品不一致。  
**已纠：** UI selected + JS fallback → **`ai`**；`fast` 保留为显式「本地规则 · 不等模型」。

---

## 3. 默认 plan_mode（硬产品）

| 规则 | 说明 |
|------|------|
| **桌面默认 = `ai`** | ModelSplitAgent → 失败再 heuristic 兜底 |
| **`fast` = 高级/显式** | 文案禁止再写「推荐」 |
| 结果台第一行须可辨来源 | 「智能拆分」vs「本地规则拆分」常显（接双受众/S1，不藏折叠芯片） |
| 工作习惯 **不得**把主受众默认改回 fast | 见 §5 纠偏 |

这与「防卡死」不矛盾：保留 zombie reap / 超时 / 取消；**用反馈与超时治卡，不用假拆当分母。**

---

## 4. 开源与行业共识（可迁移到 cco）

| 来源 | 可借 | 不借 |
|------|------|------|
| [OpenHands Plan Mode](https://docs.openhands.dev/overview/plan-mode) | Plan≠Code；专用 Planning Agent；人确认再执行 | 整站 agent IDE |
| [Claude Code Agent Teams](https://code.claude.com/docs/en/agent-teams) | Lead 拆任务 + 共享任务表 + 依赖未完成不可 claim；**按文件分区防冲突** | 实验团队全家桶 UI |
| [Claude Code parallel agents / worktrees](https://code.claude.com/docs/en/agents) | 并行必隔离 checkout；同文件串行 | 绑定 Claude 专有产品壳 |
| MetaGPT / ChatDev | 角色 SOP、制品门禁 | 纯串行「假公司」流水线当并发 |
| SWE-agent 等单智能体强工具 | 单任务深执行质量 | 替代「拆分」本身 |
| Vibe Kanban / Conductor 类（2026 编排趋势） | 一任务一 worktree + 看板审 | 换栈 |
| 工程常识 WBS / 关键路径 | 只串真依赖；水平切（全改 CSS）易冲突，垂直可交付切片更好 | — |

**反复出现的成功条件：**

1. **Planner 与 Worker 分离**（cco 已有：split agent ≠ exec worker；confirm 闸）  
2. **任务 = DAG + 验收**，不是聊天角色扮演  
3. **并行单位 = 文件/模块所有权**，不是「波次数字」  
4. **隔离（worktree）+ 小步合并**  
5. **人闸在开跑前**（cco confirm），不是跑完再哭  
6. **验证门**（done_when / check 类任务）挡级联失败  

**反模式：** 标题刮表当拆分；全串行「安全」假波次；无 scope 的 blind 并行；用领域问卷挡冷启动。

---

## 5. 工作习惯预设（方案 C · 采纳 + 一处纠偏）

### 5.1 方向对，原「领域每次打开」为何翻车

完全同意用户补正：

- 首屏多一关 = 延迟价值  
- 行业 ≠ 角色 ≠ 任务类型 ≠ 深浅  
- 领域不该直接映射几十条规则  
- 绑「每次打开」难改  
- **盖不住**「你是 worker」——展示 S0 仍是前提  
- 概念超预算  

### 5.2 推荐形态（方案 C）

```text
不是：开 App → 考卷式选行业 → 才能用
而是：最多问一次「你更常拿它干什么」
      → 3～4 种工作习惯（UI 不出现 profile 一词）
      → 只影响：话怎么说 · 拆多细 · 敢不敢并行 ·（可选）是否偏好快拆
      → 设置可改 · 项目可盖 · 可跳过
      → 计划结构自动判断 > 用户自称
```

| 选项（人话） | 内部 SplitPrefs（示意） |
|--------------|-------------------------|
| ① 我主要写需求 / 管进度（**默认 / 跳过**） | copy=plain；grain=normal；parallel=balanced；**planner=`ai`** |
| ② 我做出海 / 运营 / 落地页 | 同 ① + template_set=go_to_market |
| ③ 我会看一点实现，要对齐验收 | copy=dual；更信依赖表；parallel=balanced；planner=`ai` |
| ④ 我主要改代码 / 工程落地 | copy=tech_lean；grain=fine；parallel=eager；planner=`ai`；高级更好找 |

**时机：** 首次有项目后 / 首次「拆成步骤」前可跳过 · 欢迎页弱链 · 设置可改 · 项目可覆盖。  
**禁止：** 每次打开、20 项行业树、多选题雷达。

### 5.3 对用户草稿的硬纠偏（重要）

草稿写：「多数 profile 保持 **fast**；仅 ④ 倾向 ai」。

**否。** 与用户明确判断「默认不能走快速拆」及「要多窗口真派工」冲突。

| 旋钮 D 修订 | |
|-------------|--|
| 产品默认 | **所有习惯默认 `plan_mode=ai`** |
| `fast` | 仅「更多选项」显式选；或设置里「尽量快速（本地）」高级开关 |
| profile 可调的 | grain / parallel 建议 max_parallel / copy_density / 模板排序 |
| profile **不**调的 | confirm 契约、optional 规则、旁路开跑 |

否则：PM 默认 fast → 继续假拆 → 多窗口并发质量无从谈起。

### 5.4 优先级（从高到低）

1. **计划结构**（有 `####` 任务表 / 依赖表 → 按表；散文 PRD → 大块）  
2. 项目级 work_style（若设）  
3. 用户级 work_style  
4. 产品默认 = ① + **ai**

---

## 6. 「好拆分」在 cco 里的目标形态

```text
计划 md
  → ModelSplitAgent（ai）
      · 读：目标/非目标/任务表/依赖/文件落点
      · 出：cco-split/v1
         id, title, summary, body, depends_on, optional, done_when,
         scope_paths[]（文件所有权）, can_parallel, kind
  → soft_accept / sanitize（去假边、scope 重叠则串行或拆开）
  → SQLite SoT + 拆分台（双受众 S0 人话）
  → 人：执行规划（confirm_start）
  → Scheduler：ready 集并行 · worktree 隔离 · handoff
```

### 6.1 每条执行提示词（body）最低结构

```text
【做什么】一句话结果
【改哪里】scope 文件/目录（互不抢）
【怎样算做完】可观察标准（对齐计划完成定义）
【先等谁】无则写「无；可与 … 并行」
【不要做什么】硬契约 / 非目标（confirm 旁路、删磁盘…）
【自测】计划里的自测句压缩成 2–4 条
```

### 6.2 shell-chrome 理想 DAG（人工级 · 给 Agent 当金样）

> 波次可并行度来自计划正文；下表是「tech-lead 派工」参考，不是 heuristic 抄标题。

| id | 标题 | depends | 并行组 | scope（示意） | 验收要点 |
|----|------|---------|--------|---------------|----------|
| a1 | 去掉顶栏阶段条 | [] | A | `index.html` flow-strip · `shellChrome.js` | 顶栏无写计划→…条 |
| a2 | 拆分台只留重新规划/执行规划 | [] | A | `index.html` split-actions · `splitDetail.js` | 仅两键；仍 confirmStart |
| a3 | 去掉顶栏编辑任务 | [] | A | `index.html` · `projectPicker.js` | 顶栏无编辑任务 |
| a4 | 顶栏三键 icon 化 | [a3] | A′ | `index.html` · icons · picker | 三 icon + aria |
| a5 | 完整说明默认展开 | [] | A | `splitDetail.js` | 新选中默认 open |
| b1 | 侧栏移除项目 | [] | B | `shellUi.js` · `projectCrud.js` · css | × 确认；不删磁盘 |
| b2 | click-outside 收起 | [a2] | B | shared helper · split details | 点外关闭 |
| b3 | 底角残字清理 | [a2] | B | workspace css/html | 主区无 ghost |
| c1 | 计划信息→查看拆分结果 | [a2] | C | shellChrome chip · planSelect | 文案与回跳 |
| c2 | 历史拆分仍可见 | [c1] | C | sessionEntry · planMeta | 执行后可再看 |
| c3 | 步骤白话小抄 | [a5] | C | split 文案 / 帮助 | 无新 AI |
| d1 | 非开发目视脚本 | [a1–a5 主路径] | D | — | §5 脚本过 |
| d2 | 文档回写 | [d1] | D | docs L2 | 勾选+索引 |
| d3 | 硬契约回归 | [d1] | D | check-arch · rg | 无 start_run 旁路 |

**并行直觉：**  
- 波次 A 内 a1/a2/a5/a3 文件冲突少可并行；a4 等 a3。  
- B 与 C 大部分可在 A 主路径后并行；b1 可与 A 后期重叠（侧栏 vs 拆分台）。  
- D 串在可感知路径之后。

**对 Agent 的约束（应进 system/user prompt）：**  
- 禁止把 PROTOCOL / 非目标 / 索引表拆成任务  
- 禁止伪造 depends 来「凑波次」  
- **必须**填 scope_paths（或明确「纯文案无路径」）  
- 同文件写者不得 can_parallel  
- optional 仅业务可选（本计划几乎全必做）

### 6.3 当前 Agent 缺口 → 质量杠杆

| 缺口 | 落点 |
|------|------|
| prompt 禁止 scope | [`split_agent/prompt.rs`](../src/plan/split_agent/prompt.rs) 改为**要求** scope_paths；parse 已有字段可接 |
| user_prompt 过瘦 | 注入：硬契约摘要、计划内「完成定义」、max_parallel、**并行=文件所有权** 规则 |
| 无金样 | `shell-chrome` / dual-audience 各 1 份 expected cco-split JSON fixture |
| 假边 / 过串 | 加强 soft_accept + 计划「依赖：无」解析（双受众 S2 同向） |
| body 工人腔 | 双受众 S0 + convert 时 strip scaffold（展示优先，拆分也禁输出 worker 句） |
| 默认 fast | §3 已纠桌面默认 |

---

## 7. 与双受众 / 壳层计划的关系（防抢跑）

| 层 | 文档 | 解决什么 | 顺序 |
|----|------|----------|------|
| **展示** | 双受众 S0–S3 | 同一种拆分结果，甲乙怎么读 | **先 / 并行进行中** |
| **默认参数** | 本文 work_style | 不同人默认拆多细、多并行、话多密 | 展示 S0 后薄插 |
| **拆分智能** | 本文 §6 + split_agent | 多窗口可执行 DAG + body | 与 S2 同向；默认 ai 已先 |
| **壳层减法** | shell-chrome | 顶栏/CTA/移除…产品改动本身 | 可被上述 DAG 执行 |

```text
S0 人话（双受众）     ← 没有它，再准的 DAG 也劝退
  + 默认 ai（本文 §3） ← 没有它，DAG 根本不经模型
  + scope+body 质量    ← 没有它，多窗口必撞车
  + work_style 四旋钮  ← 锦上添花，不是前提
```

---

## 8. 建议落地顺序（薄 · 可测）

| 序 | 做什么 | 依赖 | 状态 |
|----|--------|------|------|
| **Q0** | 桌面默认 `plan_mode=ai`；fast 去「推荐」 | 无 | **✅ 本轮代码**（index.html · jobPoll.js） |
| **Q1** | 拆分结果第一行常显来源（智能/本地） | 双受众/S1 可同 PR | **✅**（`splitFillMeta.splitSourceLabel` 顶栏） |
| **Q2** | Split Agent：强制 scope_paths + body 模板 + 禁 worker 腔 + 并行规则进 prompt | Q0 | **✅**（`split_agent/prompt.rs` + parse 接 scope_paths） |
| **Q3** | soft_accept：scope 重叠不并行；信计划「依赖：无」 | Q2 · 双受众 S2 | **✅**（heuristic 依赖列 + soft_accept `serialize_scope_overlaps`） |
| **Q4** | shell-chrome（或双受众）一份 split fixture 金样 + 回归 | Q2 | **✅**（`tests/fixtures/cco_split/*.json` + parse 测） |
| **Q5** | work_style 四选一（可跳过）→ local prefs + 设置 + 并发/模板种子 | Q1+S0 | **✅ 2026-07-22**（`web/js/shared/workStyle.js` · welcome · settings · jobPoll 并发种子；**不**改默认 plan_mode=ai） |
| **Q6** | 计划结构自动压过自称 | Q5 · S2 | **✅**（heuristic 信「依赖」列 / 无列则 batch；profile 不压过计划结构；项目级覆盖可后补） |

**明确不做：** 开机领域问卷、行业中台、按习惯旁路 confirm、默认回 fast、平行第二套阶段表。

---

## 9. 拆分质量度量（可事后对照）

1. **来源**：`plan_mode=ai` 且 adapter 非 heuristic（失败兜底要标红）  
2. **可并行率**：无依赖且 scope 不重叠的任务对 / 总任务对（人工抽检）  
3. **假边率**：depends 在 body 无法解释的比例  
4. **scope 覆盖率**：有路径或明确「无代码」的任务占比  
5. **开跑前可读**：甲 10 秒能答「几步 / 能否开始」（双受众成功标准）  
6. **执行冲突**：同 run 内因同文件互相覆盖的次数（目标 → 0）  
7. **重拆次数**：人因「完全不对」点重新规划的次数  

---

## 10. 与用户三条诉求的对齐

| 诉求 | 本文结论 |
|------|----------|
| 默认不能 fast | **✅** 默认 ai；fast 显式 |
| 拆成多窗口可并发、有序提示词 | **目标 DAG + scope + body 模板 + worktree**；Agent prompt/schema 升级 |
| 画像 / 领域 | **工作习惯 C**；不问行业；不挡路；**不**把 PM 默认打回 fast |
| 另一窗在做双受众 C | 展示听双受众文档；本文不抢 S0 勾选 |

---

## 11. 落地实施真源（勾选不在本文）

**实施勾选请看：** [`multi-window-split-landing-2026-07-22.md`](./multi-window-split-landing-2026-07-22.md)  
（W1 scope+body 工单 · W2 并行不撞 · W3 来源常显 · W4 习惯可选；Q0 默认 ai 已 ✅）

本文保留决策与金样 DAG 叙述；**禁止**与落地文档双轨勾选。不另起领域中台；不与双受众双轨勾选。
