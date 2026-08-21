# src/doctor/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: run_doctor · DoctorReport/CheckLine（**help_url** 官网下载）· **browser_automation** 行（`browser_mcp::doctor_browser_line` · 默认关提示 · 启用未就绪不挡整体）· print_report · **汇总**：仅默认 provider **binary 缺失** 或 **硬 auth**（auth_invalid/insufficient_funds/endpoint_broken）拉红；非默认 CLI 缺失=行级软提示 `ok=true`；CLI 登录无 API Key（`not_supported`）**不**挡 overall
provider_probe.rs: 各通道 Key 探活 · `ProbeResult` · **is_blocking_auth_fail**（订阅登录/缺 Key 非阻塞）

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
