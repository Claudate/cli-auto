# P2-7 · SDK provider（非 CLI worker）— 设计与最小切片

> 日期：2026-07-21 · 状态：**S0 ✅**（inline）· **S1 ✅**（Anthropic Messages HTTP）· **S2 ✅**（Messages tool loop · cwd-scoped）  
> 归属：总账 **P2-7** 单项（勿整包 M5：不做自动 PR / Windows 已 t59 / Mermaid 已 t41）  
> 红线：只经 `ports::WorkerPort` · **不**旁路 confirm · **默认关** · 不改 run_dir/session/plan job

---

## 1. 问题

今日三家 worker 都是 **CLI 进程适配**：

| Provider | 机制 |
|----------|------|
| `claude` | spawn `claude` CLI（print / bg agent） |
| `codex` | spawn `codex` CLI |
| `fake` | 内联 stub **或** 可选假 bin（测试用） |

M5 / multi-cli **N4** 曾列为非目标：「用 SDK 进程内双嵌替代 CLI」。  
P2-7 出池单项目标：**先证明** 存在一条 **非 CLI 适配路径**，与 scheduler 同构，而不是重写协议。

---

## 2. 目标与非目标

### 目标（本切片）

1. 新增 `SdkProvider` 实现 `ports::WorkerPort`（name = `"sdk"`）。  
2. **不** `Command::new` 拉起 agent CLI；任务在 **进程内 backend** 完成。  
3. 经 `ProviderRegistry` 装配；**`providers.sdk.enabled` 默认 `false`** → 默认产品行为 **零 diff**。  
4. 任务显式 `provider: sdk`（或 soft-fill 到 sdk）时，scheduler 可 start/poll/stop/collect。  
5. 单测覆盖 start→poll→collect；registry 默认不含 sdk。

### 非目标（仍池内 / 后序）

| 项 | 说明 |
|----|------|
| 完整 Agent SDK / 真 PTY / 任意 shell tool | S2 仅 Messages API + **cwd 内** read/list/write；**无** Bash、无 CLI 嵌套 |
| 替代默认 `claude` | 默认 provider 仍 `claude` |
| 自动 PR · Windows · Mermaid | 已有或仍池内；本切片无关 |
| Failover 配对 sdk | `production_failover_target` 不纳入 sdk（与 fake 同） |
| 旁路 confirm / 新开跑入口 | **禁止** |
| 桌面/CLI 向导暴露 sdk | 产品决策，非本切片 |

---

## 3. 架构

```
Plan task.provider = "sdk"
        │
        ▼
ProviderRegistry.get("sdk")  ──仅当 config.providers.sdk.enabled──
        │
        ▼
SdkProvider : WorkerPort
        │
        ├── start  → SdkBackend::execute (async, in-process)
        │              write task_dir/{prompt.md,stdout.json,meta.json,.done}
        ├── poll   → .done 文件（与 fake 同契约，scheduler 无特殊分支）
        ├── stop   → 写 .done=130（协作式；inline 无真 kill）
        └── collect→ TaskResult（解析 stdout NDJSON result 行）
```

| 层 | 职责 |
|----|------|
| `domain/worker` | `ProviderId::Sdk` 可选解析；route soft-fill **不**因 sdk 改语义 |
| `ports::WorkerPort` | **唯一** worker 总线（不新总线） |
| `runtime/provider/sdk` | 适配器 + `SdkBackend` |
| `ProviderRegistry` | opt-in 注册 |
| Scheduler / app::split::confirm | **无改**；经 port 启停 |

### 3.1 Backend 分相

| 相 | 后端 | 何时 |
|----|------|------|
| **S0 ✅** | `InlineSdkBackend` | `bin=inline`（默认）或非 messages/tools 别名 |
| **S1 ✅** | `AnthropicMessagesBackend` | `bin=messages\|http\|anthropic\|api` 或 `CCO_SDK_BACKEND=messages`；需 API key |
| **S2 ✅** | `AnthropicToolLoopBackend` | `bin=tools\|tool_loop\|tool-loop\|agent` 或 `CCO_SDK_BACKEND=tools`；需 API key；cwd-scoped tools |

S1/S2 落点：同一 `SdkProvider`，registry 经 `build_sdk_provider` 注入 `Arc<dyn SdkBackend>`；**禁止** scheduler 分支 backend 名。

---

## 4. 配置

```toml
# 默认不写 = 不注册。显式开启：
[providers.sdk]
enabled = false   # 必须显式 true 才进 registry
bin = "inline"    # S0；S1 "messages"；S2 "tools"
extra_args = []   # 首项可作 model 覆盖（或 CCO_SDK_MODEL）
```

Env（S1/S2）：

- `CCO_SDK_API_KEY` / `ANTHROPIC_API_KEY`（preflight 必需其一）
- `CCO_SDK_MODEL`（默认 `claude-sonnet-4-5`）
- `CCO_SDK_BASE_URL`（默认 `https://api.anthropic.com`；可指 mock）
- `CCO_SDK_BACKEND=messages|tools|inline`（强制选后端，覆盖 bin）
- `CCO_SDK_MAX_TOKENS` / `CCO_SDK_TIMEOUT_SECS`
- `CCO_SDK_MAX_TOOL_ROUNDS`（S2 默认 8）

---

## 5. 磁盘契约（不变）

与 claude/fake **同 task_dir 形状**（A0 run-dir）：

- `prompt.md`
- `stdout.json`（NDJSON stream-json 形，末行可含 `CCO_DONE`）
- `meta.json`（`provider`/`mode`/`exit_code`/`backend`/`inline_sdk`；S1 `backend=messages` · S2 `backend=tools` · `inline_sdk=false`）
- `.done`（exit code 文本；start 必须清残留）

S2 tools（仅 `work_dir` 内相对路径；禁止 `..` 逃逸与绝对路径）：`read_file` · `list_dir` · `write_file`。

**禁止** 改 `run.json` / plan job 路径 / handoff schema。

---

## 6. 验收

| 检查 | 期望 |
|------|------|
| `cargo test --lib -p cco` | 含 sdk 单测绿 |
| `cargo test -p cco --test a0_behavior_golden --test mode_b_golden` | 绿（零行为 diff） |
| `bash scripts/check-arch.sh` | 无新违规 |
| 默认 config | registry **无** `sdk` |
| `providers.sdk.enabled=true` + task `provider: sdk` | start→Done + `CCO_DONE` |
| 开跑入口 | 仍仅 confirm |

---

## 7. 文件清单

| 路径 | 作用 |
|------|------|
| `src/runtime/provider/sdk.rs` | `SdkProvider` + `InlineSdkBackend` + `SdkBackend` trait（含 preflight） |
| `src/runtime/provider/sdk_http.rs` | **S1** `AnthropicMessagesBackend` · `MessagesHttpClient` · mock 单测 |
| `src/runtime/provider/sdk_tool_loop/` | **S2** `AnthropicToolLoopBackend` · cwd tools · multi-turn mock 单测 |
| `src/runtime/provider/mod.rs` | `mod sdk`/`sdk_http`/`sdk_tool_loop` · registry opt-in · `build_sdk_provider` |
| `src/domain/worker/types.rs` | `ProviderId::Sdk` |
| `src/config/mod.rs` | default/load/template：`providers.sdk` **enabled=false** · bin 注释 |
| 本文件 | 设计真源（单项） |
| L2：`runtime/provider/CLAUDE.md` · `config/CLAUDE.md` · `docs/CLAUDE.md` 索引 | 地图=地形 |

---

## 8. 风险

| 风险 | 对策 |
|------|------|
| 被当成默认第二 Claude | 默认 `enabled=false`；产品文案不提 |
| fake 与 sdk 重复 | fake = 测 CLI 形；sdk = **非 CLI 路径证明** + 后序真 API 挂点 |
| 厚文件 | 新文件 `sdk.rs`，不堆 `fake.rs` / `claude/` |
| 依赖膨胀 | S0 零新 crate；S1 加 `reqwest`（json only · 已落） |

---

## 9. 勾选

- [x] 设计文档  
- [x] `SdkProvider` : WorkerPort（inline backend）  
- [x] registry opt-in · config 默认关  
- [x] `ProviderId::Sdk`  
- [x] S0 单测  
- [x] **S1** Anthropic Messages HTTP backend（`sdk_http` · mock client 单测 · `bin=messages`）  
- [x] **S2** Messages tool loop（`sdk_tool_loop` · cwd read/list/write · `bin=tools` · mock 多轮）  
- [ ] 桌面/CLI 向导暴露 sdk（产品决策，非本切片）

[PROTOCOL]: 改边界先改本文与 L2；合入后总账 gap 记 t6x 一行（勿整包勾 P2-7）
