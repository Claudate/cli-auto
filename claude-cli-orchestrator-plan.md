# Claude CLI Orchestrator · 独立编排器完整计划

> 状态：**M0–M4 已落地**（doctor/run/resume/status/stop/report/logs/term/tui + providers + 桌面壳 + Mode B 主线）；**M5 + 可选增强 → 总账 D5 池（P2-5/6/7，不排期）**  
> 日期：2026-07-17（状态校正 2026-07-18；D5 池 t15）  
> 修订：锁定 **Rust**；多 Provider 扩展；**多页面 TUI + 终端多开**；已实现 claude/codex/fake  
> 定位：**与 wros / 任意业务仓库解耦** 的通用工具  
> 目标：选项目目录 → 读计划 → 主控拆任务 → 启停多个 CLI worker → 巡查完成 → 进入下一步；运行期可用 **TUI 多页 + 多终端** 观察/介入  
> 未完善总账：[`docs/gap-and-landing-plan-2026-07-18.md`](./docs/gap-and-landing-plan-2026-07-18.md) §1.3 / §4 D5（**勿再把 M0–M4 当缺口**；M5 不排期则不碰）

[PROTOCOL]: 变更时更新本文件版本与里程碑勾选。

---

## 0. 一句话

做一个**独立 Rust CLI 工具**（暂名 `cco` / `claude-orch`）：  
用户只给它一个**项目根路径** + 一份**计划文件**，它就按计划把工作拆成多个 agent CLI 会话，负责启动、巡查、关闭、推进阶段——**不嵌入任何业务仓库逻辑**。  
运行时可无界面（纯 headless），也可进入 **TUI 多页面**，并对每个 worker **多开终端**（内嵌 PTY 或系统终端窗）。

---

## 1. 产品边界

### 1.1 做什么

| 能力 | 说明 |
|------|------|
| 选项目 | 第一步强制指定 `project_root`（绝对路径） |
| 读计划 | 从 `{project_root}/docs/...` 读取计划（默认约定，可覆盖） |
| 解析任务图 | 解析阶段 / 并行组 / 依赖 / 每任务提示词 |
| Provider 启停 | 通过 `WorkerProvider` 启停 worker（v1：Claude CLI） |
| 状态机 | pending → running → done / failed / stopped |
| 巡查 | 轮询 exit / provider poll / 日志 / acceptance |
| 阶段推进 | 依赖满足后启动下一并行组 |
| 报告 | 本地 JSON + Markdown 运行报告 |
| **TUI 多页** | Dashboard / Graph / Task / Logs / Terminals / Config |
| **终端多开** | 每任务可开内嵌 PTY 或外部终端；网格布局；attach/detach |

### 1.2 不做什么

- 不替代 `git` / CI；只可选调用用户在计划里写的验收命令
- 不做云端托管（v1 仅本机）
- 不在业务仓库内写死路径；**编排器自身是独立目录/独立仓库**
- 不解析特定业务仓库（wros、inkos 等）的领域逻辑
- v1 不强制「主控也是 Claude」；host 是 `cco` 二进制

### 1.3 与业务项目的关系

```
┌─────────────────────────────┐     只读/写约定产物
│  cco（独立安装 / 独立仓库）  │ ──────────────────────► 任意 project_root
│  状态、配置在用户家目录或   │     读 docs 计划
│  项目下 .cco/（可选）       │     在 project 里 spawn worker cwd=project
└─────────────────────────────┘
```

- **运行时 cwd**：每个 worker 的工作目录 = `project_root`（或 plan 指定的 worktree）
- **编排状态**：默认写在 `~/.cco/runs/<run_id>/`，可选镜像到 `{project_root}/.cco/runs/`
- **零耦合**：从业务仓库删掉本计划文件，也不影响已独立发布的 `cco`

---

## 2. 用户主流程（UX）

```text
1. cco init                         # 可选：写全局配置 ~/.cco/config.toml
2. cco run --project /path/to/repo --plan docs/serial-plans/xxx.md
   或
   cco run --tui                    # 交互：先问项目路径，再列计划，进入 TUI
   或
   cco tui                          # 直接进入 TUI（可从 status 页 resume）
   或桌面 App：选项目 → 选计划 → 分配计划

3. 编排器（Mode B 默认）：
   a. 校验 project_root 存在、默认 provider 可用、auth 可用
   b. 读计划文档
      · 结构化（cco-plan/v1 / serial-prompts）→ 自动 skip-plan，load_plan → PlanIR
      · 散文/未知 → plan job（AI/heuristic）→ plan.proposed.json
   c. 展示任务图 / 波次
      · CLI：打印 DAG；需 --yes 或交互确认
      · 桌面：默认 auto-start（UI 调 confirm_start）；高级可「规划后暂停确认」
   d. confirm_start / Scheduler 按 depends_on + max_parallel 启 worker
   e. 巡查；完成则 stop/回收；失败按策略重试或暂停
   f. 全部完成 → 写报告 → exit 0/1
```

### 2.0 Mode B 规划 → 确认 → 执行（P0-3）

产品真源：[`docs/product-mode-b-ai-planner.md`](./docs/product-mode-b-ai-planner.md) §4.1（D1 决议）。

```text
┌──────────── Plan Job ────────────┐    ┌──────── Exec Run ────────┐
│ pending → planning → planned     │ →  │ running → … → completed  │
│              ↘ failed_plan       │    │              ↘ failed    │
│ planned 可：replan / edit / start│    │                          │
└──────────────────────────────────┘    └──────────────────────────┘
         │                                         ▲
         │  confirm_start（唯一业务 worker 入口）    │
         └─────────────────────────────────────────┘
```

| 入口 | 规划 | 确认 | 执行 |
|------|------|------|------|
| 桌面「分配计划」 | `start_plan_job`（ai/parse/fake） | 默认 **auto-start**（自动 `confirm_start`）；高级暂停 | Scheduler |
| `cco plan` | 只规划，写 `plan.proposed.json` | 不启动 worker | — |
| `cco run` 结构化 | **自动 skip-plan** | 打印 DAG + `--yes`/交互 | Scheduler |
| `cco run` 散文 | plan job（`--plan-mode`） | 打印 DAG + `--yes`/交互 | Scheduler |
| `cco run --skip-plan` | 强制 parse | 同上 | Scheduler |

**硬规则**：业务 worker **只**经 `confirm_start`（桌面）或 CLI 确认后进 Scheduler；禁止旁路 spawn。

### 2.1 第一步：设置项目文件夹（硬门禁）

任何 `run` 在解析计划前必须拿到：

```toml
# 运行参数（CLI 优先于 config）
project_root = "/absolute/path/to/target-repo"
```

校验清单：

1. 路径存在且为目录  
2. 可读  
3. （可选）是 git 仓库 — 警告非阻断，除非 plan 要求  
4. 计划文件相对 `project_root` 可解析  
5. 若 plan 要求 worktree：`git worktree` 可用  

**禁止**默认使用「当前 shell cwd 碰巧是业务仓库」而不显式确认——交互模式必须 echo 一次路径让用户确认；非交互必须 `--project`。

### 2.2 第二步：读计划

默认搜索顺序（可配置）：

1. `--plan <path>`（相对 project_root 或绝对路径）  
2. `{project_root}/docs/serial-plans/*.md` 列表供选择  
3. `{project_root}/docs/**/*plan*.md` 回退  
4. `{project_root}/.cco/plan.md` 项目级默认计划  

### 2.3 运行形态

| 形态 | 入口 | 用途 |
|------|------|------|
| Headless | `cco run --yes` | CI / 脚本 / 后台 tmux |
| TUI | `cco run --tui` / `cco tui` | 本机观察、终端多开、手动 stop/resume |
| 查询 | `cco status` / `report` / `logs` | 不进 TUI 的快速查看 |

两种形态**共用同一 scheduler / state / provider**；TUI 只是事件订阅 + 渲染 + 输入层。

### 2.4 桌面 App（Tauri）UX

桌面 App 入口：`open dist/CCO.app`（或 `cargo build -p cco-desktop --release`）。  
改版规格与阶段任务：[`docs/archive/desktop-ux-redesign-plan.md`](./docs/archive/desktop-ux-redesign-plan.md)。

```
我的项目（侧栏）
  └→ 点项目 → 工作区
       ├→ 未运行：计划列表 + 人话预览 + [开始运行]（高级选项折叠）
       ├→ 运行中：进度顶栏 + 左任务列表 + 右主日志（主从）
       └→ 完成/失败：摘要 + 再跑 / 换计划
侧栏底部：环境检查 / 设置 / 帮助
```

**主路径**

1. 添加项目文件夹  
2. 选/建计划（默认预选 `default_plan` / `last_plan`）  
3. 开始运行（启动前 doctor 门禁；缺 CLI 时人话警告）  
4. 监视：自动聚焦 running/failed；日志 ≥14px；`log_max_bytes` 默认 96KB  

**计划区**

- 单一计划列表（标题 + 任务数摘要）；路径小字  
- 预览只显示标题与步骤人话，不堆 schema / run_id  
- Provider / 执行方式放进「高级选项」  
- 运行中折叠为一行「计划 xxx · 更换」  

**监视**

- 默认主从（左任务 / 右大日志）；单任务可隐藏左栏  
- 失败摘要条 + 「仅看失败」；复制日志、字号 12/14/16  
- 文案中文：运行中 / 失败 / 已完成 / 开始运行 / 我的项目  

**设置**：状态刷新间隔、日志字号、默认引擎与执行方式（人话说明）。

---

## 3. 计划文件约定（Plan Schema）

业务仓库只需要放**数据**；编排器只认 schema，不认业务语义。

### 3.1 推荐：YAML frontmatter + Markdown 任务块

```markdown
---
schema: cco-plan/v1
name: example-wave
project_hint: optional-name-only
default_provider: claude     # 已有 claude / codex / fake；后期可 gemini / ...
default_mode: print          # print | bg | auto
max_parallel: 3
worktree: true
on_failure: pause            # pause | continue | retry
retry_max: 1
# Claude 等后端私有默认，见 providers 桶
providers:
  claude:
    permission_mode: dontAsk
    allowed_tools: ["Read", "Edit", "Bash", "Glob", "Grep"]
    max_turns: 40
    max_budget_usd: 8
---

# Example Wave

## Graph

| id | title | group | depends_on | mode | provider |
|----|-------|-------|------------|------|----------|
| t1 | setup docs | G1 | | print | claude |
| t2 | feature A | G1 | | bg | claude |
| t3 | feature B | G1 | | bg | claude |
| t4 | integrate | G2 | t2,t3 | print | claude |

## Tasks

### t1 · setup docs

```
你在项目根目录执行……
验收：……
完成后在 stdout 最后一行输出：CCO_DONE ok
```

### t2 · feature A
...
```

### 3.2 机器友好替代：纯 JSON/YAML

`docs/plans/example.wave.yaml`：

```yaml
schema: cco-plan/v1
name: example-wave
defaults:
  provider: claude
  mode: print
  worktree: true
  timeout_secs: 1200
  providers:
    claude:
      max_turns: 40
      max_budget_usd: 8
      permission_mode: dontAsk
      allowed_tools: [Read, Edit, Bash, Glob, Grep]
groups:
  - id: G1
    parallel: true
    tasks: [t1, t2, t3]
  - id: G2
    parallel: false
    depends_on: [G1]
    tasks: [t4]
tasks:
  - id: t1
    title: setup docs
    prompt_file: prompts/t1.md   # 相对 plan 文件目录
  - id: t2
    title: feature A
    mode: bg
    prompt: |
      你是 worker t2……
  - id: t3
    ...
  - id: t4
    depends_on: [t2, t3]
    prompt: |
      整合 t2/t3 结果……
```

### 3.3 PlanIR（解析后的规范模型，host 只认这个）

```yaml
# plan.resolved.json 概念
task:
  id: t2
  title: "feature A"
  depends_on: [t1]
  group: G1
  provider: claude              # 缺省 = defaults.provider
  mode: print | bg | auto       # auto = provider 按 capabilities 选择
  prompt: "..."
  acceptance: "cargo test -p foo"  # host 侧，与 provider 无关
  timeout_secs: 900
  worktree: true
  # 后端私有：host 不解释，整包交给 Provider::validate + start
  provider_opts:
    max_turns: 40
    max_budget_usd: 8
    permission_mode: dontAsk
    allowed_tools: [Read, Edit, Bash]
    model: null
```

原则：

- **通用字段**：id / deps / prompt / mode / timeout / worktree / acceptance / provider  
- **私有字段**：一律 `provider_opts`（或 plan 里 `providers.<name>` 合并进来）  
- DAG / 并行 / resume / report / TUI **从不**硬读 Claude 专用字段  

### 3.4 兼容现有「多窗提示词」Markdown（适配器）

| 适配器 | 输入 | 说明 |
|--------|------|------|
| `cco-plan/v1` | 本 schema | 一等公民 |
| `serial-prompts/v0` | 多窗提示词 MD | 启发式解析表格 + `## ID · title` 块 |
| `raw-single` | 单文件整篇 prompt | 单任务 fallback |

`serial-prompts/v0` 规则（可迭代）：

1. 找「并行组 / 依赖」表 → 建 graph  
2. 找 `## <id>` 标题下的第一个长 code fence → 作为 prompt  
3. 找不到 graph 则整篇串行单任务  
4. provider 默认 `claude`（可在 frontmatter 覆盖）  

这样**不改业务仓库**也能跑；长期鼓励业务方迁到 `cco-plan/v1`。

---

## 4. 架构

### 4.0 实现语言（已锁定）

| 项 | 选择 |
|----|------|
| 语言 | **Rust**（edition 2021+） |
| 异步 | `tokio` |
| CLI | `clap`（derive） |
| 序列化 | `serde` + `serde_json` + `serde_yaml` + `toml` |
| 错误 | `thiserror` / `anyhow` |
| TUI | `ratatui` + `crossterm` |
| 内嵌终端 | `portable-pty` + VT 解析（如 `vt100` / `vte`） |
| 日志 | `tracing` + 可选 `tracing-subscriber` |
| 测试 | 内置 test + `assert_cmd` / `predicates`；fake provider |

**M0 可用极薄 shell 冒烟，但正式代码从 Day 1 起就是 Rust crate，不再走 TS/长期 Bash 产品路径。**

### 4.1 组件

```text
cco (single binary)
├── cli/                    # clap：init / run / tui / status / stop / resume / doctor / ...
├── config/                 # ~/.cco/config.toml + env + providers.*
├── plan/                   # load + adapters + validate DAG → PlanIR
├── graph/                  # topological stages, parallel sets
├── runtime/
│   ├── scheduler.rs        # 状态机、并行、timeout、resume（与后端无关）
│   ├── worktree.rs         # optional git worktree per task
│   ├── acceptance.rs       # plan acceptance 命令
│   ├── timeout.rs
│   ├── log_events.rs       # stream-json → LogEvent（监视 A 路径 P0）
│   └── provider/
│       ├── mod.rs          # WorkerProvider trait + registry
│       ├── claude.rs       # print + bg + agents poll（首个实现）
│       ├── codex.rs        # Codex CLI（已实现）
│       └── fake.rs         # 测试用
├── state/                  # run.json, events.jsonl, task states
├── report/                 # markdown + json summary
├── doctor/                 # 按 enabled providers 预检
├── terminal/               # 终端多开抽象
│   ├── manager.rs          # 打开/关闭/聚焦多个 session
│   ├── embedded.rs         # PTY 内嵌（TUI 用）
│   ├── external.rs         # 系统终端窗（iTerm/kitty/wezterm/Terminal.app/...）
│   └── layout.rs           # 网格 / 标签布局策略
└── tui/                    # 多页面 UI（可选启用）
    ├── app.rs              # 事件环：订阅 runtime events
    ├── pages/
    │   ├── dashboard.rs
    │   ├── graph.rs
    │   ├── task_detail.rs
    │   ├── logs.rs
    │   ├── terminals.rs    # 多终端页
    │   └── config_view.rs
    ├── widgets/
    └── input.rs            # 快捷键
```

### 4.2 多 Provider 扩展模型（防腐烂契约）

```text
┌─────────────────────────────────────────────────────────┐
│  Host：cli · config · plan · graph · scheduler · state  │
│        report · doctor · terminal · tui                 │
└─────────────────────────────────────────────────────────┘
              │                    │
       PlanAdapter           WorkerProvider
              │                    │
   cco-plan/v1               ClaudeProvider   (已有)
   serial-prompts/v0         CodexProvider    (已有)
   raw-single                FakeProvider     (测试)
                             GeminiProvider   (M5 backlog)
```

#### 4.2.1 WorkerProvider trait（概念签名）

```rust
// 概念 API，实现时可微调命名
#[async_trait]
trait WorkerProvider: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    async fn preflight(&self) -> Result<()>;
    fn validate_task(&self, task: &TaskIR) -> Result<()>;
    async fn start(&self, task: &TaskIR, ctx: &StartCtx) -> Result<WorkerHandle>;
    async fn poll(&self, handle: &WorkerHandle) -> Result<WorkerStatus>;
    async fn stop(&self, handle: &WorkerHandle) -> Result<()>;
    async fn collect(&self, handle: &WorkerHandle) -> Result<TaskResult>;
}

struct Capabilities {
    print: bool,
    background: bool,
    stop: bool,
    cost: bool,
    session_resume: bool,
    /// 是否建议把 worker 绑到可交互 PTY（便于终端多开 attach）
    interactive_pty: bool,
}

struct TaskResult {
    status: TaskStatus,          // done | failed | stopped | timeout
    exit_code: Option<i32>,
    stdout_path: Option<PathBuf>,
    session_id: Option<String>,
    agent_id: Option<String>,
    cost_usd: Option<f64>,
    raw: serde_json::Value,
}
```

契约：

1. Host **不**在 scheduler 内拼具体 CLI flag（只在对应 provider 文件内）  
2. PlanIR 必有 `provider: string`，缺省 `config.default_provider`  
3. `provider_opts` 对 host 不透明；由 `validate_task` 校验  
4. Scheduler 只依赖 `TaskResult` + `Capabilities`  
5. 新增 CLI = 新 provider 模块 + config 段 + doctor + example plan，**不改** graph/state/report/tui 核心  
6. 测试：FakeProvider 覆盖 scheduler；Claude 有 contract 测试  

#### 4.2.2 完成判定（host 统一）

优先级：

1. Provider 终态（exit / agent state / job status）  
2. 通用完成标记：`CCO_DONE`（兼容）或 `ORCH_DONE`  
3. `acceptance` 命令（在 worktree/project_root 执行，exit 0）  

### 4.3 进程模型

```text
cco run [--tui]
  │
  ├─ scheduler
  │    ├─ stage G1 (max_parallel=3)
  │    │    ├─ provider.start(t1)  → handle
  │    │    ├─ provider.start(t2)
  │    │    └─ provider.start(t3)
  │    └─ stage G2 ...
  │
  ├─ terminal manager（可选）
  │    ├─ embedded PTY panes  ← TUI Terminals 页
  │    └─ external windows    ← 用户「弹出到系统终端」
  │
  └─ write report
```

### 4.4 状态目录

```text
~/.cco/
  config.toml
  runs/
    20260717T153000Z-a1b2/
      run.json
      plan.resolved.json
      events.jsonl
      tasks/
        t1/
          status.json
          stdout.json
          prompt.md
          meta.json           # session_id / agent_id / cost / exit / provider
          pty.log             # 若启用终端捕获
        t2/
          ...
      report.md
      report.json
```

可选：`--mirror-state` 同步到 `{project_root}/.cco/runs/<id>/`。

---

## 5. Claude Provider 控制规格（首个后端）

> 本章是 **ClaudeProvider** 的实现规格，不是 host 通用逻辑。

### 5.1 Print 模式（默认短任务）

```bash
claude -p --bare \
  --output-format json \
  --max-turns "$MAX_TURNS" \
  --max-budget-usd "$MAX_BUDGET" \
  --permission-mode "$PERM" \
  --allowedTools "$TOOLS" \
  --model "${MODEL:-}" \
  "$PROMPT"
```

- `cwd` = `project_root` 或 worktree path  
- 环境：`ANTHROPIC_API_KEY` 必须可用（`--bare` 不读 keychain）  
- 成功：exit 0 且 JSON 可解析  
- 失败：非 0、JSON 缺 result、超时  

### 5.2 Background 模式（长任务 / 可取消）

```bash
claude --bg --name "cco-$RUN_ID-$TASK_ID" "$PROMPT"
claude agents --json --all
claude logs <id>
claude stop <id>
claude rm <id>          # 可选清理
```

完成判定（provider 内映射到 TaskResult）：

1. `agents --json` 中 state ∈ `done|failed|stopped`  
2. 或日志出现 `CCO_DONE` / `ORCH_DONE`  
3. 或 wall-clock timeout → `claude stop` → timeout  

### 5.3 模式选择策略

| 条件 | 模式 |
|------|------|
| plan 显式 `mode` | 听从 plan |
| `mode: auto` 且 caps.background | 长任务倾向 bg |
| 预计短、无需中途取消 | `print` |
| 需超时强杀、可 attach 调试 | `bg` 或 interactive PTY |
| 同阶段并行写同一仓库 | 强制 worktree + 任一模式 |

### 5.4 主 Claude 会话嵌套 spawn？

**v1 不做。**  
编排器是独立 host；不在「某一个交互 Claude 窗」里用 Bash 套娃。  
若用户想在 Claude Code 里触发：提供 slash/skill 只是 `cco run ...` 的薄封装。

---

## 6. 编排状态机

### 6.1 Run 级

```text
init → validated → running → (paused) → completed | failed | aborted
```

### 6.2 Task 级

```text
pending
  → queued
  → starting
  → running
  → done
  → failed
  → stopped      # 用户或编排器主动 stop
  → skipped      # 依赖失败 + on_failure=continue
```

### 6.3 阶段调度

1. 计算 ready set：`depends_on` 全 done  
2. 按 `max_parallel`（及可选 per-provider 上限）启动  
3. 事件循环（默认 5s tick，可配置；TUI 下与 UI tick 合并）：  
   - `provider.poll`  
   - 检查 timeout  
   - 写 events.jsonl  
   - 向 TUI 广播 `RuntimeEvent`  
4. ready 空且有 running → 继续等  
5. ready 空且无 running → 若有 failed 且 `on_failure=pause` → paused；否则 completed/failed  

### 6.4 RuntimeEvent（TUI / 日志共用）

```rust
enum RuntimeEvent {
    RunStart { run_id, project, plan },
    TaskStart { task_id, provider, mode },
    TaskOutput { task_id, chunk },      // 可选流式
    TaskEnd { task_id, status, cost_usd },
    StageComplete { group },
    RunEnd { status },
    TerminalOpened { task_id, kind },   // embedded | external
    TerminalClosed { task_id },
}
```

---

## 7. TUI 多页面 + 终端多开

### 7.1 设计目标

1. **多页面（Pages）**：同一 TUI 内切换视图，不打断 scheduler  
2. **终端多开**：每个 running/历史 task 可绑定 0..N 个终端视图  
3. **双通道终端**：  
   - **内嵌 PTY**（默认）：在 TUI 内分屏看多个 worker  
   - **外部终端窗**：一键弹到 iTerm / kitty / WezTerm / Ghostty / macOS Terminal 等  
4. Headless 与 TUI **共享 state**；关 TUI 不杀 run（可选 `--detach-ui`）

### 7.2 页面结构

| 页面 | 快捷键（建议） | 内容 |
|------|----------------|------|
| **Dashboard** | `1` | run 状态、并行数、费用、失败数、最近事件 |
| **Graph** | `2` | DAG / 并行组；节点状态色；焦点任务 |
| **Task** | `3` | 单任务详情：prompt 快照、meta、acceptance、provider_opts |
| **Logs** | `4` | events.jsonl + 任务 stdout 滚动 |
| **Terminals** | `5` | **多终端网格**（核心页） |
| **Config** | `6` | 当前 project/plan/provider 只读视图 + 帮助 |

全局键（建议）：

| 键 | 动作 |
|----|------|
| `q` | 请求退出 TUI（可选是否 stop run） |
| `Tab` / `Shift-Tab` | 页面切换 |
| `n` / `p` | 下一/上一任务 |
| `s` | stop 焦点任务 |
| `r` | resume run（paused 时） |
| `o` | 为焦点任务 **打开内嵌终端** |
| `O` | 为焦点任务 **弹出外部终端** |
| `x` | 关闭焦点终端 session |
| `Enter` | Graph/Dashboard → 跳 Task |
| `?` | 帮助层 |

### 7.3 终端多开模型

```text
TerminalManager
  sessions: HashMap<SessionId, TerminalSession>
  focus: Option<SessionId>
  layout: Grid | Tabs | FocusMain

TerminalSession
  id: SessionId
  task_id: TaskId
  kind: Embedded | External
  // Embedded:
  pty: portable_pty::Master
  parser: vt100::Parser
  // External:
  launcher: ExternalLauncher  // 探测到的终端 app
  child_hint: Option<pid/window_id>
```

#### 7.3.1 内嵌多开（TUI Terminals 页）

- 布局：  
  - `1` 任务：全屏  
  - `2`：左右或上下  
  - `3–4`：2×2 网格  
  - `>4`：网格 + 焦点放大（vim-tmux 风格：`z` zoom）  
- 每个 pane 标题：`task_id · provider · status · cost`  
- 输入：焦点 pane 可把键盘写入 PTY（**只在「交互 attach」模式**；纯日志 tail 模式只读）  
- Claude print 模式：不一定有交互 PTY → 显示 **stdout 滚动缓冲**（伪终端视图）  
- Claude bg 模式：优先 `claude logs -f` 或读状态目录日志进 pane  

#### 7.3.2 外部终端多开

配置：

```toml
[terminal]
# embedded | external | ask
default_kind = "embedded"
# auto | iterm | kitty | wezterm | ghostty | terminal_app | tmux | custom
external_launcher = "auto"
# custom 时：
# external_command = "kitty -e {shell} -c '{cmd}'"
# 占位：{cmd} {cwd} {task_id} {run_id}
max_embedded = 6
max_external = 8
```

启动外部窗时执行的逻辑（概念）：

1. 解析 launcher（macOS 探测 `TERM_PROGRAM` / 应用是否存在）  
2. `cmd` 示例：  
   - attach 已有 bg agent：`claude logs -f <id>` 或 `cco logs --task t2 --follow`  
   - 或进入 worktree shell：`cd {worktree} && exec $SHELL`  
3. 记录 session，Dashboard 显示「已外开」徽章  
4. 外部窗关闭：best-effort 检测；不自动 stop worker（除非用户显式 s）  

#### 7.3.3 与 worker 生命周期

| 事件 | 终端行为 |
|------|----------|
| task_start | 可选 `auto_open_terminal = true` 时自动开 pane |
| task_end | pane 保留只读，标题标 done/failed；可配置 auto_close |
| run_end | 保留至用户退 TUI；report 可链到各 pty.log |
| stop task | 先 provider.stop，再可选关 terminal |

### 7.4 TUI 与 scheduler 的线程模型

```text
main (tokio)
  ├─ scheduler task          // 写 state + emit RuntimeEvent
  ├─ terminal I/O tasks      // PTY read → 更新 session buffer
  └─ tui task                // ratatui draw loop；订阅 broadcast/mpsc
```

- 状态真源在 `state/` 与内存 `RunState`  
- TUI **只读投影** + 发送 `UserCommand`（stop/open terminal/resume）到 scheduler  
- 避免在 draw 线程里直接 `kill` 子进程  

### 7.5 无 TUI 时的「多开」

Headless 仍支持：

```bash
cco term open --run <id> --task t2 --external
cco term list
cco term close --session <sid>
```

方便用户在 tmux 里自己拼观察窗，而不进入 ratatui。

---

## 8. CLI 命令面（v1）

```text
cco doctor
  检查 enabled providers、PATH、API key、git、磁盘、终端 launcher

cco init
  写入 ~/.cco/config.toml 模板

cco plans --project <path>
  列出可识别计划文件

cco parse --project <path> --plan <path>
  只解析打印任务图（dry）

cco run --project <path> --plan <path> [options]
  --yes
  --tui                  进入多页面 TUI
  --mode print|bg|auto
  --provider <name>      覆盖默认 provider
  --max-parallel N
  --adapter cco-plan/v1|serial-prompts/v0|raw-single
  --mirror-state
  --from-task <id>
  --only <id,id>
  --dry-run
  --auto-open-terminal   任务 start 时自动开终端（embedded 或按 config）

cco tui [run_id]
  附着到已有 run 或启动交互向导

cco status [run_id]
cco stop [run_id] [--task id]
cco resume [run_id]
cco report [run_id]
cco logs [run_id] [--task id] [--follow]

cco term open  --task <id> [--embedded|--external]
cco term list  [run_id]
cco term close --session <sid>
```

### 8.1 全局配置 `~/.cco/config.toml` 示例

```toml
[default]
max_parallel = 2
poll_interval_secs = 5        # scheduler 轮询间隔（秒），桌面与 CLI 均生效；范围 1–60
default_mode = "print"
default_provider = "claude"
worktree = true
mirror_state = false

[providers.claude]
enabled = true
bin = "claude"
extra_args = []
# max_turns / permission 等也可作全局默认，再被 plan 覆盖

# [providers.codex]
# enabled = false
# bin = "codex"

[terminal]
default_kind = "embedded"
external_launcher = "auto"
max_embedded = 6
max_external = 8
auto_open_on_start = false
auto_close_on_done = false

[tui]
tick_ms = 200
default_page = "dashboard"

# 允许的项目列表（桌面 App 管理，可设 default_plan）
[[projects]]
path = "/path/to/my-repo"
name = "my-repo"               # 可选显示名
default_plan = "docs/plans/my-plan.md"   # 可选：该项目默认计划（md 文档优先）
last_plan = "docs/plans/my-plan.md"      # 自动记录上次使用的计划

[[projects]]
path = "/path/to/other"
```

---

## 9. Worktree 与并行安全

当 `worktree: true` 且任务会改代码：

```text
project_root/
  .cco-worktrees/           # 或 git 默认 worktrees 路径
    cco-$run-$task/
```

规则：

1. 每并行任务独立 branch：`cco/<run_id>/<task_id>`  
2. 完成后 **不自动合 main**（v1）；report 列出分支与「建议 merge 顺序」  
3. plan 可声明 `merge_order: [t2, t3, t1]`  
4. 串行任务可共用 `project_root` 主 worktree  
5. 外部/内嵌终端的 `cwd` = 该任务 worktree  

v2 可加可选 `auto_merge`（危险，默认关）。

---

## 10. 权限、安全、费用

| 风险 | 缓解 |
|------|------|
| 无人值守乱改文件 | 默认最小 `allowed_tools`（Claude）；其它 provider 各自安全默认 |
| 密钥 | `--bare` + 环境变量；doctor 检查；不写进 report |
| 费用爆炸 | provider cost + run 级总预算 + 并行上限；无 cost 的后端 `null` |
| 死循环 agent | `max_turns` / wall timeout |
| 危险权限 | 不在 v1 默认 bypass；仅 `--i-know` 打开 |
| 路径逃逸 | 相对路径 resolve 在 project_root 下；拒绝 `..` 逃出 |
| PTY 任意输入 | 默认日志只读；交互写入需显式 attach |
| provider_opts 注入 | Provider `validate_task` allowlist；禁止任意 shell 拼接 |

---

## 11. 目录与发布形态（独立工程）

```text
claude-cli-orchestrator/          # 独立 git 根
  README.md
  Cargo.toml
  Cargo.lock
  src/
    main.rs
    lib.rs
    cli/
    config/
    plan/
    graph/
    runtime/
      provider/
    state/
    report/
    doctor/
    terminal/
    tui/
  examples/
    plans/
      demo.cco.yaml
      serial-prompts-sample.md
  tests/
    fixtures/
      fake-claude.sh
      plans/
  docs/
    plan-schema.md
    adapters.md
    tui.md
    providers.md
  scripts/
    smoke.sh
```

**本文件** `claude-cli-orchestrator-plan.md` 为设计真源；实现代码不放进业务 monorepo。

安装：

```bash
cargo install --path .
# 或
cp target/release/cco ~/.local/bin/
```

### 11.1 Cargo 依赖方向（v1）

| crate | 用途 |
|-------|------|
| clap | CLI |
| tokio | async runtime |
| serde / serde_json / serde_yaml / toml | 配置与 plan |
| thiserror / anyhow | 错误 |
| tracing | 日志 |
| ratatui / crossterm | TUI 多页 |
| portable-pty | 内嵌终端 |
| vt100 或 vte | PTY 渲染 |
| which | 探测 bin |
| chrono / uuid | run_id |
| notify（可选） | 日志文件 watch |

---

## 12. 里程碑

### M0 · Rust 骨架 + 单任务跑通（1–2 天）

- [x] 独立 crate：`cco` binary  
- [x] `clap`：`doctor` / `run` / `parse`  
- [x] `ClaudeProvider` 最小：`claude -p --bare --output-format json`  
- [x] `raw-single` 适配器  
- [x] 状态写 `~/.cco/runs/<id>/`  
- [x] `FakeProvider` + 单测  
- [x] smoke：fixture plan + fake bin  

**验收：**  
`cco run --project /tmp/demo --plan plan.md --yes` 能起 worker、回收、exit code 正确。

### M1 · MVP 编排（约 1 周）

- [x] 冻结 `WorkerProvider` + `PlanIR` + `TaskResult`  
- [x] `cco-plan/v1` YAML + DAG + `max_parallel`  
- [x] `serial-prompts/v0` 基础  
- [x] `status` / `stop` / `report` / events.jsonl  
- [x] 失败 pause  
- [x] FakeProvider 证明 scheduler 不依赖 Claude 细节  

**验收：**  
真实多任务 plan 在 fixture 跑完；`stop` 干净；换 FakeProvider 全绿。

### M2 · 后台、worktree、终端多开后端（约 1 周）

- [x] Claude bg + agents 轮询  
- [x] worktree 隔离  
- [x] timeout / acceptance（resume 部分：from-task / only）  
- [x] **TerminalManager**：embedded 会话登记 + external launcher（macOS Terminal/iTerm + kitty/wezterm/…）  
- [x] `cco term open|list|close`  
- [x] 任务日志路径绑定 session  

**验收：**  
同阶段 2 任务并行；可为每个任务外开终端看日志；report 含 cost/session。

### M3 · 多页面 TUI

- [x] `cco run --tui` / `cco tui`  
- [x] 页面：Dashboard / Graph / Task / Logs / **Terminals** / Help  
- [x] 快捷键：切页、stop、开/关终端  
- [x] UI 与 scheduler 解耦（轮询 run 目录）  
- [x] 关 TUI 不杀 run（scheduler 可继续至结束）  
- [x] Terminals 多窗格 log 网格 + zoom（伪 PTY 只读 tail）→ 总账 **P2-5 ✅ t40**；真交互 write 仍外部终端；portable-pty 未引入

### M4 · 体验与硬化

- [x] 交互选 project / plan  
- [x] 更强 serial-prompts 解析 + 黄金 fixture  
- [x] 单元/集成测试完善（bg / worktree / resume / budget / serial）  
- [x] 文档与 example  
- [x] （可选）Claude Code skill：`/cco-run` → 总账 **P2-6 ✅ t37**（`.claude/skills/cco-run/SKILL.md` 薄封装）  
- [x] per-provider 并行上限、run 级预算  
- [x] `cco resume` 从暂停/失败继续  

### M5 · 扩展 backlog → 总账 **D5 / P2-7**（t15 池；按需单独立项，勿整包）

- ~~第二真实 CLI Provider（Codex）~~ → **已有** `src/runtime/provider/codex.rs`（已出池；勿再写「尚无第二 provider」）  
- 更多真实 Provider（Gemini 等，继续验证扩展契约） → **P2-7**  
- Agent SDK 作为 `ClaudeSdkProvider` → **P2-7**  
- 计划可视化导出（Mermaid） → **P2-7 部分 t41**：`format_mermaid` + `cco parse --mermaid` ✅；其余 M5 仍池内  
- 自动开 PR（gh） → **P2-7**  
- 远程 worker — 明确不在 v1  
- Windows 外部终端 launcher → **P2-7** 

---

## 13. 测试策略

| 层 | 内容 |
|----|------|
| 单元 | plan 解析、DAG 环检测、路径沙箱、layout 算法 |
| 契约 | FakeProvider 测 scheduler 状态机 |
| Provider | stub `fake-claude` 脚本测 ClaudeProvider |
| 终端 | PTY mock / 外部 launcher dry-run |
| TUI | 尽量测状态投影与按键命令，不强依赖真实 tty（可用 ratatui test harness） |
| 集成 | 真 `claude -p` 单任务（`#[ignore]`） |
| 适配器 | serial-plans 风格 MD 黄金文件 |

```bash
# tests/fixtures/fake-claude
#!/bin/sh
echo '{"type":"result","result":"ok","session_id":"s1","total_cost_usd":0.01}'
exit 0
```

```bash
CCO_CLAUDE_BIN=./tests/fixtures/fake-claude cco run ...
```

---

## 14. 与「主 Claude + 定时巡查」设想的映射

| 原设想 | 本计划落地 |
|--------|------------|
| 主 Claude 做计划 | **人**写 plan，或另开 Claude **生成** plan；执行期 host 是 `cco` |
| 开多个窗口 | **终端多开**（内嵌网格 + 外部窗）+ 多 worker 进程 |
| 定时巡查 | scheduler 事件循环（非 `/loop`），`poll_interval_secs` 可配，桌面有设置页 |
| 定时巡查可配 | `poll_interval_secs`（1–60s）贯穿 CLI 与桌面；设置页 UI 可改 |
| 结束后关窗 | provider.stop + TerminalManager.close |
| 下一步 | DAG 调度器 |
| 看页面 | **ratatui 多页面 TUI** |

不推荐让交互 Claude 用 session `/loop` 当生产调度器。

---

## 15. 首周实施清单（Rust）

### Day 1

1. 新建独立仓库 `claude-cli-orchestrator`（Cargo binary `cco`）  
2. `doctor`：检测 claude、API key、状态目录  
3. `ClaudeProvider::print` + `run` 单任务 `raw-single`  

### Day 2

4. 多任务顺序执行 + `run.json` / `events.jsonl`  
5. `WorkerProvider` trait + `FakeProvider` 单测  
6. `parse` 打印 DAG  

### Day 3–4

7. `cco-plan/v1` YAML + `max_parallel`  
8. `serial-prompts/v0` 基础  
9. `stop` / 失败 pause / `report.md`  

### Day 5

10. `TerminalManager` 最小：external launcher + `cco term open`  
11. README + example plan  
12. 真实小仓库端到端演示  

### Week 2（TUI）

13. ratatui 壳 + Dashboard/Logs  
14. Terminals 页 + embedded PTY 多开  
15. Graph/Task/快捷键打磨  

---

## 16. 示例：对任意项目跑起来

```bash
export ANTHROPIC_API_KEY=sk-...

cd /path/to/any-project
mkdir -p docs/plans
cat > docs/plans/hello.cco.yaml <<'EOF'
schema: cco-plan/v1
name: hello
defaults:
  provider: claude
  mode: print
  providers:
    claude:
      max_turns: 20
      max_budget_usd: 2
      permission_mode: dontAsk
      allowed_tools: [Read, Glob, Grep]
tasks:
  - id: inventory
    title: list project shape
    prompt: |
      只读探索本仓库顶层结构与 README。
      用中文写一份不超过 30 行的结构摘要。
      最后一行输出：CCO_DONE ok
EOF

cco doctor
cco run --project /path/to/any-project --plan docs/plans/hello.cco.yaml --yes
cco run --project /path/to/any-project --plan docs/plans/hello.cco.yaml --tui
cco report
```

---

## 17. 风险与决策记录

| 决策 | 选择 | 原因 |
|------|------|------|
| 独立 vs 嵌业务仓 | **独立工具** | 可复用到任何 repo |
| 语言 | **Rust 锁定** | 单二进制、状态机/TUI/PTY 生态成熟、与本机 CLI 契合 |
| 主控实现 | **host 进程 cco** | 关终端可继续（headless）；状态机可靠 |
| 多 CLI | **WorkerProvider 插件** | 后期加后端不改 scheduler |
| 默认执行模式 | **print** | 边界清晰；bg / PTY 作升级 |
| 计划格式 | **cco-plan/v1 + 适配器** | 规范 + 兼容旧 MD |
| UI | **ratatui 多页面**，非 Web | 本机 CLI 场景；零服务依赖 |
| 终端多开 | **内嵌 PTY + 外部 launcher** | 观察密集用内嵌；深度交互用外开 |
| 并行写代码 | **默认 worktree** | 避免冲突 |
| 自动 merge | **v1 不做** | 合入策略属人类/CI |
| 产品名 `cco` | 保留 | 历史/简短；语义上 = CLI orchestrator，不绑死 Claude |

---

## 18. 完成定义（Definition of Done · MVP）

1. 与业务仓库无编译/运行时依赖；**Rust 单二进制**  
2. `cco run --project P --plan F` 为主路径；缺 project 则交互必问  
3. 能解析至少一种结构化 plan + 一种 serial-prompts 适配  
4. `WorkerProvider` 接口冻结；Claude 为默认实现；FakeProvider 测试通过  
5. 能顺序/有限并行启动 worker 并正确回收  
6. 状态与报告落盘可 `status`/`report` 回看  
7. `stop` 不留僵尸进程/bg agent  
8. **终端多开**：至少支持 external 打开 + list/close；TUI 路径在 M3 完成网格  
9. README 使陌生人 15 分钟内在新机器跑通 hello 示例  

---

## 19. 下一步（2026-07-18 校正）

> M0–M4 与桌面/Mode B 主线 **已落地**（见总账 §1.3）。下列「建仓」步骤 **作废**，勿再当待办。

按优先级（残差）：

1. ~~产品规则收口~~ → **D1 已完成**（P0-1/2/3 · P1-7；见 §2.0 与 Mode B §4.1）  
2. ~~监视与桌面接线~~ → **D2 已完成**（P1-1..P1-3）  
3. ~~边界金样与重打包验证~~ → **D3 已完成**（P0-4·P1-4..P1-6）  
4. ~~大文件纵切~~ → **D4 已完成**（t14）  
5. M5 / 可选增强 → 总账 **D5 池（P2-5/6/7 等）**；**不排期则不碰**；按需单独立项  

完整缺口与顺序真源：[`docs/gap-and-landing-plan-2026-07-18.md`](./docs/gap-and-landing-plan-2026-07-18.md) §4 D5。

---

## 附录 A · 事件类型（events.jsonl）

```json
{"ts":"...","type":"run_start","run_id":"...","project":"...","plan":"..."}
{"ts":"...","type":"task_start","task_id":"t1","provider":"claude","mode":"print","pid":123}
{"ts":"...","type":"task_end","task_id":"t1","status":"done","cost_usd":0.12}
{"ts":"...","type":"task_end","task_id":"t2","status":"failed","error":"timeout"}
{"ts":"...","type":"terminal_open","task_id":"t2","kind":"external","session_id":"..."}
{"ts":"...","type":"stage_complete","group":"G1"}
{"ts":"...","type":"run_end","status":"completed"}
```

## 附录 B · run.json 最小字段

```json
{
  "schema": "cco-run/v1",
  "run_id": "20260717T153000Z-a1b2",
  "project_root": "/path/to/repo",
  "plan_path": "docs/plans/hello.cco.yaml",
  "adapter": "cco-plan/v1",
  "started_at": "...",
  "finished_at": null,
  "status": "running",
  "tasks": {
    "t1": {
      "status": "done",
      "provider": "claude",
      "session_id": "...",
      "cost_usd": 0.1
    },
    "t2": {
      "status": "running",
      "provider": "claude",
      "agent_id": "abc",
      "terminals": ["sid-1"]
    }
  }
}
```

## 附录 C · 术语

| 词 | 含义 |
|----|------|
| project_root | 被编排的目标业务仓库根 |
| plan / PlanIR | 任务图 + 提示词；解析后的规范模型 |
| run | 一次编排执行实例 |
| worker | 一个 provider 拉起的进程或 bg agent |
| host / cco | 编排器本身 |
| adapter | 计划文件格式解析器 |
| provider | WorkerProvider 实现（claude / codex / fake；其它见 M5） |
| page | TUI 内一个全局面板 |
| terminal session | 内嵌 PTY 或外部终端窗，通常绑定某 task |

## 附录 D · TUI 线框（Terminals 页）

```text
┌─ cco · run 20260717… · running · $0.42 ─────────── 1:Dash 2:Graph 3:Task 4:Logs 5:Term ─
│ task   provider  status   term
│ t1     claude    done     —
│ t2     claude    running  embedded×1 external×1
│ t3     claude    running  embedded×1
├──────────────────┬──────────────────────────────────┐
│ t2 · claude · bg │ t3 · claude · bg                 │
│ … log stream …   │ … log stream …                   │
│                  │                                  │
│                  │                                  │
├──────────────────┴──────────────────────────────────┤
│ focus: t2  [o]embed [O]external [x]close [s]stop [z]zoom │
└─────────────────────────────────────────────────────┘
```

---

**文档结束。** 实现时以本文件为真源；若落独立仓库，将本文件复制为该仓库 `docs/architecture-plan.md` 并在此标注新位置。
