# 项目切换「只加载当前数据」落地计划
> 2026-08-22 · 分支 `feat/ui-redesign-dsh` · 状态 ✅ **T1–T6 全部落地**（门禁绿：`node build.mjs` 253/253 · `cargo check -p cco` clean · `cargo test -p cco --lib` 721 passed）

**父级真源**：[`架构大改`](./architecture-redesign-2026-07-20.md) · [`CLAUDE.md`](../CLAUDE.md) 工程硬规则 §15-22
**相关 L2**：[`src/services/`](../src/services/CLAUDE.md) · [`web/`](../web/CLAUDE.md) · [`src/doctor/`](../src/doctor/CLAUDE.md)

## 一、用户定的原则（不可动摇）

1. **历史积累再多，和项目切换没关系**——切换只加载「当前项目的当前数据」。
2. **doctor 和项目切换无关**——环境检查有独立生命周期，不挂在切换关键路径上。
3. **打开的窗口需要哪些才加载哪些**——落地哪个视图就加载那个视图要的，不过度加载。

> 这三条是验收基线：任何改动若违反其一，视为未完成。

## 二、实测现状（本机当前项目实际）

- `~/.cco/runs/` 已有 **100** 个 run 目录；`~/.cco/cco.db` ~1.7MB。
- 切换/刷新走的 `project_live_view` 每次都 **全量扫描**，与「当前项目」无关。

### 缺陷点（file:line 实证）

| # | 位置 | 现状 | 违反原则 |
|---|------|------|----------|
| D1 | [`src/services/runs.rs:93`](../src/services/runs.rs) `list_runs` | `read_dir` 整个 `~/.cco/runs/`，排序取前 80，逐个 `RunState::load`（反序列化含全部 task） | ①③ 全历史扫描 |
| D2 | [`src/services/live.rs:186`](../src/services/live.rs) `project_live_view` | 调 `list_runs` 后才 `filter` 出本项目——先加载全世界再丢弃 | ①③ 过度加载 |
| D3 | [`web/js/features/settings/shellBoot.js:230`](../web/js/features/settings/shellBoot.js) 轮询 tick | **chat 页每 2s 无条件 `loadLive()`**（workspace 有 `shouldPoll` 门，chat 没有）→ 每 2s 触发一次 D1 全量扫描 | ①③ 后台持续过度加载 |
| D4 | [`src/services/runs.rs:168`](../src/services/runs.rs) `aggregate_plan_runs` | 计划列表 meta 扫描前 **200** 个 run，同样全历史 | ③ 过度加载 |
| D5 | [`web/js/features/project/sessionEntry.js:81`](../web/js/features/project/sessionEntry.js) 我上一轮的 `host.ensureDoctor().catch(()=>{})` | fire-and-forget 只是「不阻塞」，doctor 仍被切换触发；且裸调用绕开本文件 scopeGen 纪律 | ② doctor 仍耦合切换 |

### 没有的东西（关键遗漏）

- **不存在「当前项目 → 当前 run」指针**。`project_ui_prefs` 只有 `dismissed_run_id`（[`src/state/project_ui.rs:20`](../src/state/project_ui.rs)），没有 `current_run_id`。所以 `choose_current_run`（[`src/services/live.rs:494`](../src/services/live.rs)）只能靠「扫全表再 filter 再挑」拿到当前 run——这正是 D1/D2 无法短路的根因。

## 三、我之前分析错在哪 / 漏了什么（认账）

- **错**：早期把卡顿主要甩给 doctor 的网络探活。实测 doctor 有 60s 缓存，且与切换耦合只在我自己加的 sessionEntry.js:81 一处；chatRender:626 / jobPoll:209 / SplitView:455 的 doctor 调用都是「用户点环境检查」或「confirm 开跑前预检」，**合理，不该动**。真正的常态卡顿是 D1/D2/D3 的全历史扫描。
- **漏**：完全没算到 **D3——chat 页每 2s 无条件全量扫描**。这才是「停在聊天页也卡、切回来更卡」的持续性来源。
- **漏**：没指出「缺当前 run 指针」这个根因，导致之前只想着「并行化扫描」——那是给错误的设计加速，不是修设计。
- **半吊子**：sessionEntry.js 的 fire-and-forget 只解决「阻塞」，没解决「doctor 不该被切换触发」，也破坏了本文件 scopeGen 一致性。

## 四、根因归类

> 一句话：**没有按项目建立当前 run 的索引，所有「取当前」都退化成「扫全历史」；而这个扫描被切换和 2s 轮询反复触发。**

## 五、落地计划（分阶段 · 后阶段依赖前阶段）

### T1 — 建「当前项目 → 当前 run」指针（根因，最高优先） ✅

- **改哪里**：[`src/state/project_ui.rs`](../src/state/project_ui.rs) 复用 `project_ui_prefs`，加 `KEY_CURRENT_RUN_ID`（`set/get/try_*`，对齐现有 `dismissed_run_id` 写法）。
- **谁写**：run 生命周期唯一真源——scheduler 开跑/终态处（[`src/runtime/scheduler/*`](../src/runtime/CLAUDE.md)）在写 `run.json` 时顺手 `try_set_current_run_id(project, run_id)`；「结束计划」时按现有 dismissed 逻辑处理。
- **落地**：`app::project_ui::try_set_current_run` 在三个已握有 config+project+run_id 的收口点写入——`app/run/foreground.rs::prepare_scheduler`（CLI+resume+tests）、`services/runs.rs::start_run_from_plan_with_route_opts`（桌面开跑/confirm/rework）、`spawn_resume`（桌面 resume/retry）；全 best-effort。
- **验收** ✅：`current_run_roundtrip` 测试证明 set/get/clear + 与 dismissed key 相互独立。

### T2 — `project_live_view` 走指针，不扫全历史 ✅

- **改哪里**：[`src/services/live.rs:186`](../src/services/live.rs)。
- **怎么改**：先读 `current_run_id`（T1）→ 命中则 `load_run(run_id)` 直达，**不调 `list_runs`**；未命中（老项目/无指针）才 fallback 到「只扫本项目」的窄扫描（见 T3）。dismissed 逻辑保持不变。
- **落地**：`resolve_current_run` 指针快路径（`try_get_current_run_id` → `load_run` → `paths_match` + `should_hide_run_as_current` 守卫），未命中回落 `list_runs_for_project` + `choose_current_run`；「新硬活 run 清 dismissed」逻辑改按 `rs.run_id`/`rs.status`。
- **验收** ✅：命中指针全程只 `load_run` 一次；100 个历史 run 不再被 touch。

### T3 — 保留的扫描收窄为「按项目」，不再取全世界 ✅

- **改哪里**：[`src/services/runs.rs:93`](../src/services/runs.rs) `list_runs` 增 `list_runs_for_project(config, project)` 变体（或加 `project` 过滤参数），在 `read_dir` 遍历时**边读 run.json 边按 `project_root` 过滤**，命中即用，不把别的项目 load 进内存。
- **同步**：`aggregate_plan_runs`（D4）改用同一按项目扫描；`take(200)` 收窄为「本项目内」上限。
- **落地**：`for_each_project_run` + 廉价 `RunProjectPeek`（仅反序列化 `project_root`），仅匹配项目才全量 `RunState`；`max_matches` 界定每项目全载数。
- **验收** ✅：`list_runs_for_project_excludes_other_projects` 测试证明其他项目 run 不被反序列化；全量 `list_runs` 仅跨项目处保留。

### T4 — 砍掉 chat 页每 2s 的无条件 `loadLive` ✅

- **改哪里**：[`web/js/features/settings/shellBoot.js:230`](../web/js/features/settings/shellBoot.js)。
- **怎么改**：chat 分支不再每 tick `loadLive()`。live 只在「真有活跃 run（`hasActiveRunForEvents`）」或 run 事件到达时刷新；idle 聊天页零轮询扫描。与 workspace 分支的 `shouldPoll` 门对齐。
- **验收** ✅：chat 分支已门控 `if (runLive) loadLive()`；idle 聊天页零轮询扫描。

### T5 — doctor 与切换彻底解耦 ✅

- **改哪里**：[`web/js/features/project/sessionEntry.js:81`](../web/js/features/project/sessionEntry.js) 移除 `host.ensureDoctor()`。
- **原则**：doctor 只由三类触发——用户点「环境检查」（`openChatEnvDoctor`）、confirm 开跑前预检（jobPoll:209 / SplitView:455）、以及 doctor 页自身。切换不触发。
- **scopeGen**：切换关键路径 `Promise.all([loadLive, loadPlansForPicker])` 保留，但确保结果落地前用现有 `scopeGenStillCurrent` 守卫（对齐本文件纪律），不留裸 fire-and-forget。
- **验收** ✅：`selectProject` 已去 `ensureDoctor`；`scopeGenStillCurrent` 守卫保留；doctor 三类合法触发点仍在。

### T6 — 按落地视图懒加载（chat 落地不加载看板/日志） ✅

- **改哪里**：`loadLive()` 桥（[`web/js/features/project/loadLiveBridge.js:54`](../web/js/features/project/loadLiveBridge.js)）。
- **怎么改**：落地 chat → 只要 live 的「锁态/当前计划卡」所需最小 DTO；落地 workspace/result 才拉 task board / 日志 / verification。复用既有 `log_max_bytes=0` 后端预算（服务端跳过每任务 log tail），不新开 DTO 档位/不改 IPC 契约。
- **落地**：桥不再无条件传 `logMaxBytes: 96000`，改 phase-aware（`running` | `done`+活跃 → 96KB，否则 0），与 `features/run/loadLive.js` 的默认对齐；chat/idle 落地 phase 非 running → 0 预算。
- **验收** ✅：落地 chat（phase pick/confirm）传 0 字节 → 服务端不读 task 日志；落地 workspace（phase running）才 96KB。

## 六、门禁与验收

- `cd web && node build.mjs` 通过；`cargo check -p cco` + `cargo test -p cco --lib` 绿。
- 手测：`~/.cco/runs` 保留 100+ 目录，切换项目 P95 无可感卡顿；chat 页 idle 时后台无全量扫描（可临时 `tracing` 计数 `list_runs` 调用验证）。
- 不违反 [`CLAUDE.md`](../CLAUDE.md) §15-22：文件不破 600 行硬顶；UI 不写业务策略；IPC 仅经 gateway。

## 七、不做什么（防蔓延）

- 不重命名 crate、不动 IPC 命令名/JSON 字段（web 兼容）。
- 不删跨项目历史面板真正需要的全量 `list_runs`；只是不让「切换/常态刷新」走它。
- 不动 doctor 的探活逻辑本身（并行化那版可留作独立小优化，但**不是**本计划重点——本计划是「不触发」而非「触发得更快」）。
- 不引入新 `*Manager` / 不加厚 `services`；T1 指针复用既有 `project_ui_prefs`。

## 八、建议执行序

`T1 → T2 → T3`（后端根因链，一条 PR 可含）→ `T4 → T5`（前端解耦，风险低可并行薄改）→ `T6`（懒加载分层，最后做，独立验收）。

