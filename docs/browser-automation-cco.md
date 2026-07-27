# cco 浏览器自动化（网页验收 · 抓取回填 · 表单冒烟）

> **能力契约真源**（本能力唯一勾选落点）。  
> 产品定位仍见 [`PRODUCT.md`](../PRODUCT.md)：任务控制台，**不是** IDE。  
> 工程分层见 [`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)。

---

## 1. 要什么 / 不要什么

| 要 | 不要 |
|----|------|
| Worker 用浏览器工具：截图、打开预览、核对主路径 | 嵌 Michael IDE / 调闭源 `automation-server` |
| 抓取公开页数据写回 **scope 内** 项目文件 | cco 进程内自研完整 CDP 引擎（默认） |
| 表单/主路径冒烟（人话步骤） | 新 `TaskRole=browser`；UI 旁路 Mode B 开跑 |
| 证据进 `.cco-out/browser/<task_id>/` | 主路径第一句甩 MCP/CDP/引擎名 |
| 默认 **关**；任务 **optional** 默不勾 | 静默 auto-start 可选浏览器步 |

---

## 2. 引擎

| | 默认 | 后备 |
|--|------|------|
| 实现 | **Kitewright** MCP（Rust · chromiumoxide · CDP） | `@playwright/mcp` |
| 选型理由 | 与常见本机 AI IDE 的 Rust/CDP 路线同族；体积轻、可审计 | 更成熟、工具略多 |
| 浏览器 | 本机 Chrome / Chromium / `chrome-headless-shell` | 同左（Playwright 自带亦可） |

cco **不内嵌**浏览器内核；只给 Claude worker 注入 **task 级** `--mcp-config`（print 仍可用 `--bare`，显式 mcp-config 不受 bare 自动发现关闭影响）。

配置（`~/.cco/config.toml`）：

```toml
[browser]
enabled = false
engine = "kitewright"          # kitewright | playwright_mcp
command = "npx"                # 或本机 kite
args = ["-y", "@kitewright/mcp"]
out_dir = ".cco-out/browser"
require_preview = true
strict_mcp = true              # --strict-mcp-config
```

环境覆盖（可选）：`CCO_BROWSER_ENABLED=1`、`CCO_BROWSER_ENGINE=…`。

---

## 3. 任务形状（无新 Role）

沿用 `scout|implement|integrate|inspect|closeout`；用 **tags + outputs + prompt**：

| 能力 | tags | 典型 outputs | 拆分台 risk |
|------|------|--------------|-------------|
| 页面截图验收 | `browser`, `ui-verify` | `.cco-out/browser/<id>/shot.png`, `report.md` | 跑命令 / 改本地 |
| 抓取回填 | `browser`, `scrape` | 业务 path + `.cco-out/browser/<id>/raw.md` | **会外发** |
| 表单冒烟 | `browser`, `ui-smoke` | `.cco-out/browser/<id>/smoke.md`（+ 可选 shot） | 跑命令 |

- `done_when`：人话（主 CTA 可见、标题含 X…）。  
- `verify_cmd`：仅可选 **端口探活**（shell）；**禁止**用 shell 假装会点页面。  
- `optional: true`：确认屏默不勾。  
- 抓取必须写清 **源 URL + 写入相对路径**；`scope.paths` 覆盖写入目标。

---

## 4. 运行时

```text
tags ∋ browser 且 config.browser.enabled
  → 写 task_dir/mcp-browser.json
  → env: CCO_PREVIEW_URL · CCO_BROWSER_OUT
  → claude: --mcp-config … [--strict-mcp-config]
  → append_system_prompt: BROWSER 片段（证据目录 · 只动允许 URL）
  → outputs 门禁检查 shot/report 等
```

Preview：优先 `preview_status` 的 URL（`.cco/preview`）；无 URL 且 `require_preview` 时 ui-verify 应在 prompt/报告中说明无法验收（不静默 PASS）。

Inspect：可读 `.cco-out/browser/**` 作证据；**仍不改业务源码**。

---

## 5. 安全

- 默认 enabled=false。  
- Task 级 MCP + 建议 strict，避免串用户全局 MCP。  
- 截图/raw 默认只写下 out_dir 与 scope。  
- 外站 scrape → 人话「会外发」。  
- 不把密码写进 plan；不宣称企业级 SSRF 黑名单（可后续 allowlist）。

---

## 6. 波次勾选

| 波 | 内容 | 状态 |
|----|------|------|
| **W0** | 本文 + prompts/L2 交叉 · 产物路径约定 | ✅ |
| **W1** | config · doctor · mcp 注入 · preview env · BROWSER system 片段 · 示例 optional 验收任务 · 单测 | ✅ |
| **W2** | scrape 模板 · risk 会外发 · scope 强制 | ✅（risk tag + 文档；模板随 prompts） |
| **W3** | ui-smoke 示例 · live `browser_evidence` · 结果台「网页验收证据」缩略/摘录 | ✅ |
| **W3+** | 设置页「网页自动化」开关 · ui-verify 无预览 soft-fail · scrape 缺 scope validate | ✅ |
| **W3++** | 设置引擎/预览门闩 · 结果台点图放大/打开文件 · 非 Claude soft env 提示 | ✅ |

### W3+ / W3++ 行为细则

| 项 | 行为 |
|----|------|
| 设置 | 高级 → **网页自动化** · **网页引擎**（kitewright / playwright_mcp）· **验收须有预览** |
| ui-verify 无预览 | `enabled` 且 `require_preview` 且 tags 含 `browser`+`ui-verify` 且无 `CCO_PREVIEW_URL` → **任务启动失败**（人话错误，不 spawn worker） |
| scrape 缺 scope | tags 含 `browser`+`scrape` 且 `scope.paths` 空 → **plan validate 失败**（须写写入白名单） |
| ui-smoke | 不强制 preview 门闩（可关 require_preview 后仅靠 prompt） |
| 结果台 | 截图可点放大；「打开文件」走 `open_path` |
| 非 Claude | 仍写 env + browser 纪律 prompt；**不**注入 `--mcp-config`（需本机 CLI 自配 MCP 或改用 claude） |

---

## 7. 非目标

- Michael IDE / 闭源 automation-server  
- Domain 依赖 chromiumoxide  
- 任意页面 `eval` 工具暴露  
- 把 Playwright 设为**默认**引擎（仅 `engine=` 后备）
