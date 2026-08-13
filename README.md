# Leaf — 轻叶 · 项目任务控制台

**Leaf（轻叶）** — 独立 **Rust** 桌面/CLI 编排器：读计划 → DAG 调度 → 通过 `WorkerProvider` 启停 agent CLI（首期 **Claude**，可扩展）。「翻叶子」暗合计划流转。

产品方向（给谁用、主循环、轻量化）：[`PRODUCT.md`](./PRODUCT.md)  
工程设计真源：[`docs/architecture-redesign-2026-07-20.md`](./docs/architecture-redesign-2026-07-20.md)（架构 · A0–A5 ✅）· 索引 [`docs/CLAUDE.md`](./docs/CLAUDE.md)  
历史编排设计（M0–M4 考古）：[`docs/archive/claude-cli-orchestrator-plan.md`](./docs/archive/claude-cli-orchestrator-plan.md)

## 状态

**M0–M4 + macOS 桌面 App（Tauri）**

- CLI：`doctor` / `run` / `resume` / `status` / `stop` / `report` / `logs` / `term` / `tui`
- **桌面软件**：`dist/Leaf.app`（Tauri 2 原生窗口，可双击）
- Plan：`cco-plan/v1` · `serial-prompts/v0` · `raw-single`
- Providers：`claude`（print + bg）· `codex`（exec）· `fake`
- 桌面监视：可读事件流（解析 stream-json）/ 原始日志切换
- worktree / acceptance / run 预算 / TUI

### 桌面 App（推荐）

三步上手：

1. 打开 App → 点 **＋** 添加本机项目文件夹  
2. 在项目内选一份计划文档（`.md`）→ **开始运行**  
3. 左侧看任务进度，右侧读完整日志；失败会自动聚焦  

```bash
# 已打包好（在仓库根目录）：
open dist/Leaf.app

# 或从源码构建（web/ 为前端资源，改完需重新打包才进 .app）：
cargo build -p cco-desktop --release
# 再运行 scripts/package-app.sh（或见 dist/）
```

压缩包：`dist/CCO-macos-arm64.zip`  
UX 改版计划：[`docs/archive/desktop-ux-redesign-plan.md`](./docs/archive/desktop-ux-redesign-plan.md)

## 构建

```bash
cargo build --release
# 二进制：target/release/cco
```

## 快速开始（假 provider，无需 API）

```bash
# 使用内置 fake（默认当 bin 找不到时 inline 成功）
export CCO_STATE_ROOT=/tmp/cco-demo-state
export CCO_DEFAULT_PROVIDER=fake

cargo run -- doctor
cargo run -- init --force

# 任意项目目录
DEMO=/tmp/cco-demo-proj
mkdir -p "$DEMO/docs/plans"
cp examples/plans/hello.cco.yaml "$DEMO/docs/plans/"

cargo run -- parse --project "$DEMO" --plan docs/plans/hello.cco.yaml
cargo run -- run --project "$DEMO" --plan docs/plans/hello.cco.yaml --yes --provider fake
cargo run -- status
cargo run -- report
```

## 真 Claude

```bash
export ANTHROPIC_API_KEY=sk-...
# 可选：CCO_CLAUDE_BIN=/path/to/claude

cco doctor --project /path/to/repo
cco run --project /path/to/repo --plan docs/plans/hello.cco.yaml --yes --provider claude
```

用 stub 测 Claude 路径：

```bash
chmod +x tests/fixtures/fake-claude
export CCO_CLAUDE_BIN="$PWD/tests/fixtures/fake-claude"
cco run --project "$DEMO" --plan docs/plans/hello.cco.yaml --yes --provider claude
```

## 计划示例

见 [`examples/plans/`](./examples/plans/)。

```yaml
schema: cco-plan/v1
name: hello
defaults:
  provider: claude   # 或 fake
  mode: print
  providers:
    claude:
      max_turns: 20
      max_budget_usd: 2
      permission_mode: dontAsk
      allowed_tools: [Read, Glob, Grep]
tasks:
  - id: inventory
    title: explore
    prompt: |
      ...
      CCO_DONE ok
```

## 架构要点

```
plan adapters → PlanIR → scheduler → WorkerProvider → TaskResult
                              ↑
                     state / report / CLI
```

新增 CLI 后端 = 新 `WorkerProvider` 实现 + config 段，不改 scheduler。

## 命令

```text
cco doctor [--project PATH]
cco init [--force]
cco plans [--project PATH]                 # 可交互
cco parse [--project PATH] [--plan PATH] [--adapter ...]
cco run [--project PATH] [--plan PATH] [--yes] [--provider] [--mode] [--only a,b]
        [--dry-run] [--auto-open-terminal] [--terminal-kind embedded|external]
        [--max-budget USD] [--tui]
cco resume [run_id] [--yes] [--tui] [--max-budget USD]
cco status [run_id]
cco stop [run_id] [--task id]
cco report [run_id]
cco logs [run_id] [--task id] [--follow]
cco term open [--task id] [--kind embedded|external] [--shell]
cco term list [run_id]
cco term close --session <id>
cco tui [run_id]
```

### Resume / 预算

```bash
# 失败 pause 后继续未完成任务
cco resume --yes

# run 级总预算（超出后 pause，不再启动新任务）
cco run --project "$DEMO" --plan docs/plans/hello.cco.yaml --yes --provider fake --max-budget 0.005
```

### TUI

```bash
cco run --project "$DEMO" --plan docs/plans/hello.cco.yaml --yes --provider fake --tui
# 或附着已有 run
cco tui
```

键位：`1-6` 切页 · `j/k` 选任务 · `o`/`O` 开终端 · `s` 停任务 · `q` 退出（不杀 run，若 scheduler 仍在跑会等其结束）

### 终端多开示例

```bash
# run 完成后（或进行中）为任务开日志窗
cco term open --task inventory --kind external
cco term open --task inventory --kind embedded   # 只登记 session，供 TUI 使用
cco term open --task inventory --shell           # 在 work_dir 开交互 shell
cco term list
cco term close --session <id>
```

## 测试

```bash
cargo test
```

## 许可证

MIT
