# B2 · Headless 静默执行模式

> 类型：**实施真源**（本文为 B2 唯一勾选落点）
> 日期：2026-08-14
> 来源：harness-inspired-roadmap-2026-08-14.md §B2 + §U-B2
> 约束：架构规则 4（地图与地形同构）· 规则 12（CLI/桌面同一 app 路径）· 规则 23（主路径第一句不出现技术枚举/run_id）· 规则 24（高级能力默认关）

---

## 一、问题

Leaf 目前 `cco run` 是**前台交互式**：规划→TTY `proceed?` 人确认→前台阻塞循环→打印人话 summary + status + report 路径。
对 CI / 脚本 / 定时场景这套不够用：

- 没有「无人值守」入口——CI 要人按回车才能开跑；
- 没有**机器可解析**的完成输出——CI 只能 grep 文本，不能 `jq`；
- stderr 与 stdout 混着人话摘要，无法 `2>/dev/null` 干净丢弃运行日志。

Harness 的 headless 语义值得借鉴：一条命令、JSON 到 stdout、日志到 stderr、exit code 可直接 `if`。

---

## 二、设计

### CLI 表面（规则 12：仍走同一 Application API，非第二套调度）

`cco run` 增加两个 flag：

- `--headless`：进入静默执行路径——**不**起 TUI、**不**走 `proceed?` 交互确认（等价 `--yes` 但语义面向 CI）；
- `--output json`：完成时把结构化结果打印到 **stdout**（仅 `json` 值受支持；缺省=现有人话输出）。

**关键约束**：`--headless` **不**绕开 Mode B 开跑。它仍调 `app::split::confirm_materialize`（散文）/ `app::run::materialize_run`（ParseOnly）——与桌面 confirm、与现有 `cco run` **同一 Application 用例**（规则 10/12）。headless 只改三件事：

1. 跳过 `interactive::confirm("proceed?")`（强制 yes，等价 `--yes`，但文案面向 CI）；
2. 不渲染 TUI / 不 auto_open_terminal；
3. 完成时把结果序列化成 JSON 打到 stdout，**替代**现有人话 summary 那几行。

`--headless` 与 `--tui` 互斥（headless 优先，TUI 被忽略并 warn）。

### 开跑链路（复用，无新策略）

```
plan_then_load_ir (散文) / load_plan (structured)
  → confirm_materialize / materialize_run_with_route   （同一 app 用例 · 规则 12）
  → prepare_scheduler(ForegroundOpts{ auto_open_terminal:false, terminal_kind:Embedded })
  → preflight_plan
  → sched.run().await                                     （同一调度循环）
  → finish_with_reports                                   （同一收尾 + Ensure）
  → emit headless JSON                                    （B2 新增 · 仅打印）
```

**不**新增 `app::headless::*` 用例、**不**复制 soft-fill / optional drop / auto_commit 策略。B2 是**纯 Presentation** 层的输出格式适配。

### JSON 输出契约（§U-B2 · JSON 即 UX）

完成时 stdout 输出（stderr 仍走 log_events / 人话进度，可 `2>/dev/null` 丢弃）：

```json
{
  "summary": "完成 8/10 个任务",
  "status": "completed",
  "tasks": [
    { "id": "t1", "title": "搜集竞品资料", "status": "done", "duration_s": 42 }
  ],
  "failed_tasks": [],
  "cost_usd": 0.12,
  "exit_code": 0
}
```

字段语义：

| 字段 | 来源 | 说明 |
|------|------|------|
| `summary` | `report::report_summary_line`（剥 markdown `**`） | 人话一句话，CI 日志里直接可读（规则 23：第一句是人话，非 run_id/VERDICT） |
| `status` | `RunStatus` 映射 `completed\|failed\|partial` | `Completed→completed`；`Failed/Aborted→failed`；`Paused→partial`（部分完成可续） |
| `tasks[]` | `RunState.tasks` + `plan.resolved` title | 每项 `id/title/status/duration_s`；status 用 `done\|failed\|skipped\|stopped\|pending`（CI 友好小写） |
| `failed_tasks[]` | `tasks` 过滤非 done 终态 | 空数组而非 null（方便 `jq .failed_tasks[]`） |
| `cost_usd` | `report` 已算的 `total_cost_usd` | 无花费时 `null`（不伪造 0） |
| `exit_code` | `finish_with_reports` 返回 | 0=全完成；1=部分失败；2=Paused；与进程 exit code 一致 |

**第一行是 `summary`（人话），不是 `run_id`/`VERDICT`**（规则 23）。`run_id` 不进 headless JSON——CI 要 run_id 读 `report.json`。

### 人话 vs JSON 分流

- `--headless`（无 `--output json`）：stderr 走 log_events，stdout 走**人话 summary**（`report_summary_line`）——面向「人在 CI 日志里扫一眼」。
- `--headless --output json`：stdout 走上述 JSON——面向 `jq` / 脚本解析。
- 都不发 `run_id:` / `run_dir:` / `report:` 这些人话行到 stdout（改 stderr，保持 stdout 干净）。

### H1 输出构建位置

JSON 不进 `app::run`（那是业务用例层）。新增 `report::headless_result(&RunState) -> HeadlessResult`（观察面，同 `report_summary_line` / `summarize_providers` 同级），由 CLI handler 调用并 `serde_json::to_string` 打印。**纯读取 RunState + plan.resolved**，无策略、无 IO 写。

---

## 三、不做的部分（本轮）

| 条目 | 理由 |
|------|------|
| `--output text`/`yaml` 等多格式 | 本轮只 `json`；多格式后置 |
| 流式 JSON（逐任务推送） | 本轮只在**完成时**打一次完整 JSON；流式属 B1 事件总线 |
| 进度条 / spinner | headless 无 TTY 交互；stderr 留 log_events 即可 |
| 桌面/Tauri headless | 桌面本就非 CI 场景；headless = CLI only |
| 新增 `app::headless` 用例 | 违反规则 12（同一 app API）；B2 纯 Presentation |
| 超时/取消 flag | 现有 `--max-budget` + `stop` 已覆盖；本轮不引入 `--timeout` |

---

## 四、验收标准

1. `cargo build` 通过，`scripts/check-arch.sh` 无新 violation（规则 15/16 体积）；
2. `cco run --headless --plan …` 不提示 `proceed?`、不起 TUI、完成后人话 summary 到 stdout；
3. `cco run --headless --output json --plan …` 完成时 stdout 为单一 JSON 对象，`jq .status` / `jq .failed_tasks[]` 可用；
4. stderr 仍含 log_events（`cco run --headless --output json 2>/dev/null` → stdout 纯 JSON）；
5. exit code：全完成=0，部分失败=1，Paused=2（与现有 `finish_with_reports` 一致）；
6. 开跑仍经 `confirm_materialize` / `materialize_run`（规则 12 同一 app 路径，无第二套调度）；
7. `--headless` 与 `--tui` 同传：headless 优先，stderr warn TUI 被忽略；
8. JSON 第一字段为 `summary`（人话），不出现 `run_id`/`VERDICT` 作首字段（规则 23）。

---

## 五、勾选（改代码时在此更新）

- H1 `report::headless_result` 观察 DTO ✅
- H2 CLI `--headless` / `--output` flag（cli/mod.rs）✅
- H3 `commands/run.rs` headless 分流 + JSON 打印（复用 app 用例）✅
- H4 `--headless` 跳过 confirm / 不起 TUI / stderr 分流 ✅

---

> [PROTOCOL]: 改代码时先更新此文件勾选；完成后更新 docs/CLAUDE.md「还在做」区；
> 门禁：`scripts/check-arch.sh`；规则 12 同一 app 路径；规则 23 JSON 首字段人话。
