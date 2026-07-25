# 巡检关账闭环（Ensure）根治计划

> **状态：实现已落 · E0–E5 ✅ · E6 金样/打包 ✅ · wros 五条铁律人工实测 ☐**  
> 日期：2026-07-24  
> 触发：wros `chat-20260724-0629` 多轮卡在末尾 inspect（功能已绿 · 台账/地图未关 · 误点「再跑一次」空转）  
> 角色：**契约变更 + host 闭环**——把「终端步骤」从「只读开 ISSUES」升级为「对照计划 → 有界关账 → 再验 → 可自动回补直至终结」  
> **不**替代 Mode B `confirm_start`；**不**让 inspect 无界改业务代码凑 PASS；**不**回灌 D0–D4 / P2-17 已勾项  

关联真源：

| 读这个 | 关系 |
|--------|------|
| 本文 | **本问题唯一实施勾选落点** |
| [`split-product-rules.md`](./split-product-rules.md) | 落地后**同步短规则**（终端职责 / plan_ref 主人） |
| [`plan-execute-inspect-rework-2026-07-19.md`](./plan-execute-inspect-rework-2026-07-19.md) | P-loop 规则参考；**阶段勾选勿继承**；Q3「inspect 不改 L1」由本文 **M3 有界关账** 修订 |
| [`multi-cli-collaboration-2026-07-18.md`](./multi-cli-collaboration-2026-07-18.md) | role/scope/VERDICT 既有能力 |
| 外部草稿 `巡检卡点根治计划_c689af7e` | 方向参考（closeout + auto_rework）；**触发条件与职责拆分以本文为准** |

[PROTOCOL]: 变更时更新文首状态与 §5 勾选；落地后同步 `docs/CLAUDE.md` · `split-product-rules.md` · 相关 L2；**禁止**平行第二套「巡检总览」阶段表

---

## 0. 一句话

**终端步骤必须保证「计划勾选 ↔ 磁盘事实」对齐并可终结：先审计，再只修「地图/台账类」缺口，再审计；业务缺口自动回补；标准漂移与信息不足才停人。**

```text
计划成功标准（勾选真源）
    ↓ host 抽出 plan_checklist（confirm/materialize）
实现波（每条 plan_ref 有主人 + 证据）
    ↓
E1 审计  →  gap 分类 A/B/C/D
    ↓
E2 有界关账（仅 B + 约定 residual）
    ↓
E3 再审计
    ↓
全 PASS → Done
仅 A → 自动 rework（≤2）→ 回 E1
C/D 或轮次耗尽 → 停下 + 人话说明
```

---

## 1. 问题定性（为什么修了好几次还复发）

### 1.1 用户感知

每次拆解跑到最后一步都红：

- 卡面：`inspect VERDICT=FAIL … Open risks ISSUES[t7-p0-gates]`
- 主按钮像「再跑一次」→ 重考官 → 再 FAIL
- 功能其实已交付；人不知道该点「回补」还是重跑

### 1.2 本轮铁证（wros · run `20260724T085442Z-347c`）

| 项 | 事实 |
|----|------|
| adapter | `cco-split/heuristic` |
| `require_inspect` | **false** |
| 实现任务 role | **None**（t1–t4） |
| 终端任务 | `t7-p0-gates` · role=`inspect` · 标题含「**并回写台账**」 |
| smoke / 单测 | **全绿**（VERDICT 实测表） |
| blocking | **B6** 台账 §6/§9/README 仍「未开工」 |
| map | **M1** acceptance README 断链 |
| residual | R1 未 commit 等（不挡 PASS） |
| inspect 契约 | 系统提示 + 剥 Edit：**业务树只读**，只写 `.cco-out/inspect/**` |

**结论：不是某次实现写挂；是「关账无主人 + 考官只读 + 主机不自动修」的结构环。**

### 1.3 多层根因（必须同时断）

| 层 | 断点 | 证据 | 只修本层为何不够 |
|----|------|------|------------------|
| **L0 产品契约** | 用户要「查出并推动完成」；代码要「只读开单」 | `INSPECT_SYSTEM_PROMPT` vs 用户诉求 | 补丁永远和目标拧着 |
| **L1 拆分职责** | 末尾任务 = 门禁 **+** 回写台账 | 标题/prompt 揉在 inspect | 有 closeout 也会双头 |
| **L2 角色 hardening** | inspect 剥写入、禁改业务 | `materialize_inspect_task` | 越诚实越 FAIL |
| **L3 主人缺失** | 台账/commit 无独立任务 | DAG 无 closeout | 永远留给「下一轮」 |
| **L4 触发漏诊** | 旧稿绑 `require_inspect && Implement` | 本图两者皆假/空 | **closeout 根本不注入** |
| **L5 对齐真源软** | 勾选对照靠模型读 md | 无 host checklist 驱动调度 | 漏项靠运气 |
| **L6 闭环死停** | FAIL 后 rework 要人点 | `start_rework_from_run` 存在但不自动 | 红框永久 |
| **L7 UI 误导** | 失败卡主 CTA「再跑一次」 | `logBoardCard.js` | 人空转考官 |
| **L8 历史补丁片面** | 权限 / 分级 / rework API 各修一环 | 多轮仍卡 | 环未闭合 |

### 1.4 缺口分类（对齐失败时补什么）

| 类 | 含义 | 例 | 谁修 | 能否自动 |
|----|------|----|------|----------|
| **A 证据缺口** | 功能/验收未达标 | smoke 红、API 未挂 | rework implement | 可自动 rework |
| **B 地图缺口** | 证据已在，台账/勾选/索引未同步 | B6、M1、GEB 指针 | **有界关账** | **应自动** |
| **C 标准漂移** | 实现把成功标准改弱 | 必做变 optional | **人** | **禁止自动** |
| **D 信息不足** | 计划写不清无法判定 | 空话成功标准 | 回改计划 | 停人 |

图 1 主因 = **B**。对 B 再跑 inspect 或重做实现都无效。

### 1.5 与旧契约的冲突（必须显式修订）

[`plan-execute-inspect-rework`](./plan-execute-inspect-rework-2026-07-19.md) 已落地 P-loop，但留下：

- Q3：inspect **不**改 L1/L2 → 文档滞后只开 ISSUES  
- N5：禁止检验员兼全部施工  
- rework **不**默认自动  

这些在「防假成功」上正确，在「计划必须终结」上不足。  
本文 **不推翻**「禁止无界改业务凑 PASS」，**修订**为：

> 终端 **Ensure** = E1 只读审计 + E2 **有界**关账（仅 B）+ E3 再审计；业务代码仍不进 E2。

---

## 2. 产品契约（冻结）

### 2.1 终端目标（用户语言）

巡检/收尾的主作用：

1. **对照计划**查出遗漏与偏向（不是只看 exit 0）  
2. **推动补齐**可自动部分（地图/台账）  
3. **再验**后给出终态，使整轮计划可终结  
4. 业务缺口自动进入回补波；标准问题才问人  

### 2.2 选定模型：**M3 审计 → 有界关账 → 再审计**

| 模型 | 是否采用 | 原因 |
|------|----------|------|
| M1 纯审计（现状） | ❌ | 不能终结 |
| M2 考官全能修 | ❌ | 假成功 / 偏向 |
| **M3 Ensure** | ✅ | 可终结 + 防放水 |

### 2.3 有界关账白名单（E2 仅可写）

**允许（有证据才勾 ✅）：**

- 计划/台账勾选行、§ 状态表、README 进度句  
- map 断链：文档索引、acceptance README 指针  
- `.cco-out/progress/**`：`plan_ref → 证据`  
- 约定 residual：`git add` 相关文档 + commit（信息含 plan_ref / run 摘要）  
- 路径偏好：`docs/**`、`**/*.md`、`README*`、`.cco-out/**`、`tests/**/README*`、`CLAUDE.md`

**禁止：**

- 改业务源码（`src/**`、引擎 crate 等）凑绿  
- 无 smoke/证据勾 ✅  
- 把 blocking 改 residual、把必做改 optional  
- 改 acceptance 标准本身、删失败测试  

### 2.4 终态机

```text
E1 审计
  → 无 gap                    → Done(PASS)
  → 仅 B（+可自动 residual）  → E2 关账 → E3
  → 含 A                      → auto rework（≤ REWORK_MAX）→ 再 E1
  → 含 C 或 D                 → Paused + 人话（改计划 / 接受残留）
  → 轮次耗尽                  → Paused + 未清 plan_ref 列表
```

### 2.5 配置（默认开，可关）

| key | 默认 | 含义 |
|-----|------|------|
| `default.auto_closeout` | `true` | 物化注入关账任务 / E2 |
| `default.auto_rework` | `true` | 终态后 A/B 满足条件自动 `start_rework` |
| `default.auto_rework_docs_only` | `true` | 若 true：仅全部 blocking 为 docs-closeout 才自动；含业务 path 则停人（**首版建议 true**，更安全） |

说明：首版可用 `auto_rework_docs_only=true` 先打通 B 类死循环；A 类自动 rework 为 §5 同波或紧随后波，须单独金样。

### 2.6 非目标

- 不让 UI/`start_run` 旁路 Mode B  
- 不做第二套完整拆分台  
- 不在 JS 复制门禁策略  
- 不把 TUI 做成第二控制台  
- 不在本轮做 A5-5 crate 拆分  
- **不**把「再跑一次」删掉（降为次要：仅疑巡检本身坏时用）

---

## 3. 解决放哪（完整落点图）

> 法则：规则在 domain；用例在 app；IO 在 runtime/services；UI 只渲染 DTO。  
> **禁止**往 `services/runs.rs` 厚文件堆策略；新逻辑抽文件或进 domain/app。

### 3.1 总览

| 能力 | 层 | 主路径（新建或改） |
|------|----|--------------------|
| 缺口分类 docs-closeout | domain | **新建** `src/domain/inspect/classify.rs` |
| 计划勾选清单抽取 | domain | **新建** `src/domain/plan/checklist.rs`（或 `domain/chat` 旁路复用 parse） |
| TaskRole::Closeout | domain | `src/domain/plan/types.rs` |
| 注入 closeout + 剥离 inspect 关账职责 | domain | `src/domain/plan/materialize.rs` |
| Ensure 语义 / 系统提示 | domain | `types.rs` 常量 + closeout prompt 模板 |
| 配置开关 | config | `src/config/mod.rs` |
| 物化挂载 | app | `src/app/run/materialize.rs`（+ 现有 route 收口） |
| 自动 rework | app 用例 + 薄 IO | **新建** `src/app/run/ensure_loop.rs`；**禁止**策略堆进 `services/runs.rs` |
| 调度终态钩子 | runtime | `scheduler/finish.rs` 或 run 收尾 → 调 app ensure_loop |
| 前台 CLI 收尾 | app | `src/app/run/foreground.rs` |
| rework prompt 补 commit | runtime | `src/runtime/handoff/rework.rs` |
| 拆分：门禁≠关账 | plan | `planner/heuristic.rs` · `split_agent/prompt.rs` · llm 表约束 |
| 桌面 DTO | services/live 或 app | `InspectLoopView` 增 `auto_rework_run_id` / `ensure_phase` |
| UI CTA | web | `features/run/logBoardCard.js` · `features/result/*` |
| 短规则 | docs | `split-product-rules.md` 增「终端 Ensure」一节 |
| 金样 | tests | `tests/scheduler_fake.rs` 等 |

### 3.2 Domain（纯规则 · 无 IO）

#### 3.2.1 `domain/inspect/classify.rs`（新）

```text
is_docs_closeout_issue(issue: &ParsedIssue) -> bool
all_blocking_are_docs_closeout(issues: &[ParsedIssue]) -> bool
classify_kind(issue) -> Evidence | MapCloseout | Drift | Underspecified   // 可先 MapCloseout+Evidence 两档
```

判定（首版可启发式，单测锁样本）：

- `severity == Map` → docs-closeout 真  
- `severity == Blocking` 且 path 命中文档白名单 **且** symptom/fix_wp/raw 含收尾词（`docs`/`closeout`/`readme`/`回写`/`台账`/`勾选`/`index`/`索引`/`commit`/§）  
- 反例：path∈`src/**` 或「引擎未实现」类 → 假  
- 空列表 → `all_blocking…` = false  

**单测必含** wros 本轮 `ISSUES.md` 中 B6/M1 原文 → 真；混合业务 blocking → 假。

#### 3.2.2 `domain/plan/checklist.rs`（新 · host 对齐表）

confirm/materialize 时从计划 md + 任务 acceptance/`plan_ref` 抽出：

```text
PlanChecklistItem {
  plan_ref,          // 稳定 id：P0-1 / §9-W2 / 标题哈希回落
  text,              // 成功标准原文
  owner_task_id?,    // 物化后填
  evidence_hint?,    // verify_cmd / done_when
  kind,              // feature | ledger | map | other
}
```

规则：

- 每个**必做**勾选 ≥1 主人；ledger/map 类若无主人 → 物化时归 `sys-closeout`  
- 清单落盘：`run_dir/plan.checklist.json`（schema 版本字段）  
- E1/E3 prompt **必须**粘贴该清单（R-rework-2 同级硬规则）  
- 报告「对照计划」优先读此文件，减少模型自由发挥  

#### 3.2.3 `TaskRole::Closeout`

- serde `snake_case`：`closeout`  
- 旧 plan 无此值：兼容  
- `parse` / `as_str` / `parse_role_input` 同步  
- validate：closeout 不得作为唯一业务叶替代 inspect 终端（inspect 仍可作 E1/E3）

#### 3.2.4 `materialize.rs`：`inject_closeout_task`

**触发（修订 · 防漏诊）：** 满足任一即考虑注入：

1. 存在 `role == Inspect` 的任务，**或**  
2. 存在 kind/标题启发式门禁尾（`门禁|验收|巡检|inspect|gates|VERDICT`），**或**  
3. `require_inspect == true`  

且：

- `config.auto_closeout`  
- 尚无 `role=Closeout` / id `sys-closeout`  
- 存在至少一个非 inspect/非 system-post 业务任务（role 可为 None）  

**不要**要求 `role=Implement`。

**DAG：**

```text
[业务任务…] → sys-closeout → [inspect / E3]
```

- `sys-closeout.depends_on` = 全部业务任务 id（或业务叶）  
- 原 inspect 的 `depends_on` **增加** `sys-closeout`（保留原边）  
- 幂等：重复 materialize 不双注入  

**closeout 任务字段：**

| 字段 | 值 |
|------|-----|
| id | `sys-closeout` |
| role | `Closeout` |
| title | 人话：「回写台账与验收索引（有证据才勾）」 |
| scope.paths | docs 白名单 + `.cco-out/progress/**` |
| scope.forbid | 业务源码硬否（如 `inkos-rs/crates/**/src/**` 过宽则用通用 `**/src/**` 策略需测 wros） |
| prompt | 固定模板：跑/读已有 acceptance 证据 → 绿则回写 plan_checklist 中 ledger/map 项 + 可选 commit → 不绿则只写 progress 禁止勾 ✅ |
| provider/mode | plan 默认；走现有 permission/effort 软填 |

**剥离 inspect 关账职责（同函数内）：**

- 若 title/prompt 含「回写台账|勾选|§9|gap-audit|commit」类 → 改写为「只验收对照清单；关账由 sys-closeout」  
- 确保 inspect outputs 含 VERDICT/ISSUES；scope 可写仅 `.cco-out/inspect/**`（E1/E3）  
- **禁止**再给 inspect 塞「并回写台账」标题  

### 3.3 Config

`src/config/mod.rs`：

```toml
# ~/.cco/config.toml [default]
auto_closeout = true
auto_rework = true
auto_rework_docs_only = true
```

settings 桌面高级区可后续暴露；首版 CLI/config 即可。

### 3.4 Application

#### 3.4.1 物化挂载

与 `apply_permission_mode` 同级，在：

- `app/run/materialize.rs::materialize_run_with_route`  
- 任何 `start_run_from_plan*` 最终 materialize 点（**一条路径**，避免双写分叉）

顺序建议：

```text
materialize_selected_tasks
→ materialize_role_defaults
→ inject_closeout_task（新）
→ apply soft-fill permission/effort
→ validate
→ 写 plan.checklist.json
```

#### 3.4.2 `app/run/ensure_loop.rs`（新 · 自动回补）

```text
maybe_auto_rework(config, run_id) -> Option<ReworkStartResponse>
```

条件（全满足）：

1. run 终态 Failed/Paused  
2. `is_inspect_gate_error`  
3. blocking 可解析；若 `auto_rework_docs_only` 则 `all_blocking_are_docs_closeout`  
4. `count_rework_rounds < REWORK_MAX_ROUNDS`  
5. 无 `ACCEPTED_RESIDUAL`  
6. `auto_rework == true`  

动作：

- 调现有 `start_rework_from_run`（或抽 port 避免 services↔app 环；若暂经 services facade 须标注 deprecated 过渡）  
- handoff timeline：`auto_rework_wave · round=N · trigger=docs-closeout|ensure`  
- 返回新 run_id 供 UI  

挂载：

- 桌面：scheduler `run().await` 结束、`write_reports` 之后  
- CLI：`foreground.rs` 收尾  

**架构约束：** 策略判断在 domain；编排在 app；`services/runs.rs` 只保留薄委托（L1 硬规则：禁止继续堆）。

### 3.5 Runtime / Handoff

| 文件 | 改动 |
|------|------|
| `runtime/handoff/rework.rs` | rework prompt 增加：「修复完成后按需 `git add` 文档与 progress 并 commit（信息含 ISSUE id）」；map 白名单与 classify 白名单对齐 |
| `runtime/handoff/rework.rs` `InspectLoopView` | `auto_rework_run_id: Option<String>`；`ensure_phase: Option<String>`（audit/closeout/reinspect） |
| `runtime/scheduler/finish.rs` 或等价收尾 | 钩子 → app ensure_loop（不解析 VERDICT 正文） |
| inspect 系统提示 | E1/E3 保持只读；**Closeout 角色**另起 prompt，不复用「禁止一切写入」 |

### 3.6 拆分侧（从源头少揉职责）

| 文件 | 改动 |
|------|------|
| `plan/split_agent/prompt.rs` | 硬规则：验收/巡检任务 **禁止** 写「并回写台账/commit」；关账单独任务或交 host 注入 |
| `plan/planner/llm.rs` | 表字段说明：inspect ≠ closeout |
| `plan/planner/heuristic.rs` | work-order 末尾：inspect 只对照；progress/台账回写在落地波或 closeout |
| soft_accept | 若末任务同时像 inspect+ledger → 拆或打标供 materialize 剥离 |

### 3.7 前端（只消费 DTO）

| 文件 | 改动 |
|------|------|
| `web/js/features/run/logBoardCard.js` | inspect 门禁失败：主 CTA =「回补并再巡检（第 N/2 轮）」；「再跑一次」次要 + title 说明 |
| `web/js/features/result/inspectCopy.js` / `ResultView.js` | 展示自动回补已发起 + 跳转新 run；人话避免引擎词第一句 |
| `web/js/features/result/ResultViewModel.js` | 已有 rework；接 `auto_rework_run_id` 提示 |

**禁止**前端判断 docs-closeout；只读 `can_rework` / 新字段。

### 3.8 文档 / GEB

| 文件 | 动作 |
|------|------|
| **本文** | 实施真源与勾选 |
| `docs/CLAUDE.md` | 活跃落地索引加入本文 |
| `docs/split-product-rules.md` | 增 §「终端 Ensure / 关账」短规则 |
| `docs/plan-execute-inspect-rework-…` | 文首注：Q3 由 Ensure E2 有界修订（**不**复活阶段勾选） |
| `src/domain/CLAUDE.md` · `src/app/CLAUDE.md` · `src/runtime/CLAUDE.md` | 成员清单 + 硬规则一句 |
| `src/domain/plan/types.rs` `INSPECT_SYSTEM_PROMPT` | 可保留只读；Closeout 另常量 |

---

## 4. 端到端目标路径（用户可见）

### 4.1 幸福路径（本次 wros 类）

```text
confirm
  → 物化：t1..t4 + sys-closeout + t7-inspect（职责已剥离）
  → 写 plan.checklist.json（P0 功能项 + ledger 项）
  → t1..t4 实现（smoke 绿，台账仍旧）
  → sys-closeout：见绿 → 回写 §9/README/gap-audit + 修 M1 索引 + commit
  → t7 审计：对照 checklist → PASS
  → run Done · 无红框
```

### 4.2 closeout 漏做 / 仍 FAIL（仅 B）

```text
inspect FAIL(B6/M1)
  → host 自动 rework-r1（docs scope）
  → reinspect PASS
  → 结果台：「已自动回补 run xxx」
```

### 4.3 含业务 blocking（A）

```text
首版 auto_rework_docs_only：停下 +「回补并再巡检」主 CTA
同波若开启 A 自动：rework 可写业务 path → 再验
```

### 4.4 标准漂移（C）

```text
ISSUES 标 drift / 人话：「实现与计划不一致，请改计划或接受残留」
禁止自动勾 ✅
```

---

## 5. 实施波次与勾选（只认这里）

### E0 · 契约与分类（先锁语义） ✅

- [x] 本文 §2 评审确认（M3 / 白名单 / 配置默认）  
- [x] `classify.rs` + B6/M1 真实样本单测  
- [x] `split-product-rules.md` 草稿一节（可与 E1 同 PR）  

### E1 · 物化关账主人 ✅

- [x] `TaskRole::Closeout`  
- [x] `inject_closeout_task` + 触发条件（**不**绑 require_inspect/Implement）  
- [x] 剥离 inspect 关账文案  
- [x] config `auto_closeout`  
- [x] materialize 单测：依赖边、幂等、开关关、role=None 图  

### E2 · host 勾选清单 ✅

- [x] `plan.checklist.json` 抽取与落盘  
- [x] closeout/inspect prompt 注入清单  
- [x] ledger 无主人 → 归 closeout  

### E3 · 自动回补闭环 ✅

- [x] `app/run/ensure_loop.rs`  
- [x] 桌面 + CLI 挂载  
- [x] `auto_rework` / `auto_rework_docs_only`  
- [x] rework prompt 补 commit  
- [x] InspectLoopView 字段  
- [x] fake provider 端到端：docs-only FAIL → 自动新 run（`tests/ensure_close_loop.rs`）  

### E4 · UI 反误导 ✅

- [x] 失败卡主 CTA = 回补  
- [x] 「再跑一次」降级  
- [x] 自动回补提示 + 跳转  

### E5 · 拆分源头 ✅

- [x] split_agent / llm / heuristic 禁止 inspect 兼关账  
- [x] 金样：标题不再出现「巡检并回写台账」（启发式 inspect 标题已锁「专门巡检对照计划」）  

### E6 · 验收与打包 ✅（wros 人工五条铁律仍 ☐）

- [x] `cargo test --lib` 相关 Ensure/红线绿（a0/mode_b/handoff/scheduler_fake 集成绿；chat/preview 2 条既有 flaky 与本波无关）  
- [x] 更新受影响 golden（mode_b / a0 / scheduler_fake — **无需改期望图**；closeout 仅 materialize 注入）  
- [x] `tests/ensure_close_loop.rs`：closeout 注入 · docs-only 自动 rework · 业务 blocking 停人 · 开关关  
- [x] `scripts/package-app.sh`  
- [ ] **wros 实测**（§6）五条铁律（需真实项目/计划；自动化金样已覆盖结构环）  

---

## 6. 验收铁律（没过 = 未解决）

用 **同一类** wros 计划（门禁+台账成功标准），禁止只靠单元测宣称完成：

| # | 场景 | 期望 |
|---|------|------|
| V1 | 无人点击完整跑 | implement → closeout → inspect **PASS** |
| V2 | 故意跳过 closeout 写入 | 自动 rework 一轮后 PASS 或明确 B 已清 |
| V3 | 失败卡 UI | 主按钮是回补不是再跑考官 |
| V4 | 故意留业务缺口 | docs_only 模式下停人；或 A 自动 rework 后有业务 diff |
| V5 | 模型企图无证据勾台账 | closeout 纪律禁止；inspect 仍可 FAIL |

**回归：** 权限假 done、optional 不停、Mode B 唯一开跑、inspect 不改业务源码凑 PASS。

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| closeout 乱改业务 | forbid scope + prompt + 无 Edit 业务工具；单测 path |
| 无证据勾 ✅ | prompt 硬规则 + inspect E3 再抓；可选 host 校验「勾选行变更 ⇒ 对应 evidence 存在」 |
| 与 system_post inspect 双尾 | id/依赖去重；`sys-closeout` 与 `sys-post-*` 文档化顺序 |
| services 循环依赖 | ensure_loop 放 app；services 只 facade |
| 体积超限 | 新文件；禁止堆 `runs.rs` / 厚 scheduler |
| 金样大面积红 | 先测 inject 幂等再改期望图 |
| 用户关自动 | config false 时回退「可点回补」，CTA 仍正确 |

---

## 8. 假设与不变量

- `REWORK_MAX_ROUNDS = 2` 不变  
- Mode B confirm 仍是唯一业务开跑入口；rework/ensure 是 **P-loop 延续**，不是第二入口  
- CLI 与桌面共用 app ensure_loop  
- soft-fill 不覆盖任务显式 route  
- 人话第一句不出现 run_id/VERDICT（结果台既有规则）  

---

## 9. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-24 | 初稿：多层根因 + M3 Ensure + 完整落点 + E0–E6；吸收 c689af7e 并修正触发漏诊 |
| 2026-07-24 | 实现：E0–E5 代码+单测+短规则/L2；E6 留 package + wros 实测；`finish_plan_job` critic 提前 save 防 refresh 丢字段 |
| 2026-07-24 | E6：`tests/ensure_close_loop.rs` 四金样绿；timeline 勿写 `rework_wave` 误计轮次；package-app 打包；wros 人工 V1–V5 仍 ☐ |
